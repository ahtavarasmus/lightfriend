use crate::{
    handlers::auth_middleware::AuthUser, pg_models::NewPgBridge, utils::matrix_auth, AppState,
};
use anyhow::{anyhow, Result};
use axum::{extract::State, http::StatusCode, response::Json as AxumJson};
use matrix_sdk::{
    config::SyncSettings as MatrixSyncSettings,
    ruma::{
        api::client::room::create_room::v3::Request as CreateRoomRequest,
        events::room::message::{MessageType, RoomMessageEventContent, SyncRoomMessageEvent},
        events::AnySyncTimelineEvent,
        OwnedRoomId, OwnedUserId,
    },
    Client as MatrixClient,
};
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{sleep, timeout, Duration};

use std::path::Path;
use tokio::fs;

const TELEGRAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(45);

/// Truncate a string to `max_chars` characters for log output, replacing newlines
/// with spaces so the log stays on a single line. This is used to dump bot message
/// bodies into logs without blowing up log size.
fn truncate_for_log(s: &str, max_chars: usize) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    if cleaned.chars().count() <= max_chars {
        cleaned
    } else {
        let truncated: String = cleaned.chars().take(max_chars).collect();
        format!(
            "{}...<+{} chars>",
            truncated,
            cleaned.chars().count() - max_chars
        )
    }
}

/// Flatten any std::error::Error source chain into a single grep-friendly line,
/// joining causes with " -> ". Works for reqwest::Error, matrix_sdk::Error, etc.
fn format_std_err_chain(e: &(dyn std::error::Error + 'static)) -> String {
    let mut parts: Vec<String> = vec![e.to_string()];
    let mut current: Option<&(dyn std::error::Error + 'static)> = e.source();
    while let Some(cause) = current {
        parts.push(cause.to_string());
        current = cause.source();
    }
    parts.join(" -> ")
}

/// Flatten an anyhow::Error chain into a single line via its `.chain()` walker.
fn format_anyhow_chain(e: &anyhow::Error) -> String {
    e.chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// Log the proxy / TLS environment variables that reqwest and matrix-sdk
/// auto-detect. This is the first thing to check when the enclave behaves
/// differently from a bare VPS, because the enclave usually routes outbound
/// traffic through vsock -> host proxy and custom TLS config.
fn log_network_env(user_id: i32) {
    // Proxy env vars (reqwest/hyper auto-detect these)
    let proxy_vars = [
        "HTTP_PROXY",
        "http_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "ALL_PROXY",
        "all_proxy",
        "NO_PROXY",
        "no_proxy",
    ];
    for key in proxy_vars.iter() {
        match std::env::var(key) {
            Ok(v) => tracing::info!("[TG-CONNECT user={}] PHASE=net_env {}={}", user_id, key, v),
            Err(_) => tracing::info!(
                "[TG-CONNECT user={}] PHASE=net_env {}=<unset>",
                user_id,
                key
            ),
        }
    }

    // TLS cert env vars (various crates honor these differently)
    let tls_vars = [
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "REQUESTS_CA_BUNDLE",
        "CURL_CA_BUNDLE",
    ];
    for key in tls_vars.iter() {
        match std::env::var(key) {
            Ok(v) => tracing::info!("[TG-CONNECT user={}] PHASE=net_env {}={}", user_id, key, v),
            Err(_) => tracing::info!(
                "[TG-CONNECT user={}] PHASE=net_env {}=<unset>",
                user_id,
                key
            ),
        }
    }
}

/// Read /etc/resolv.conf and log the first 500 chars. On the VPS this
/// is usually the cloud provider's resolver; inside the enclave it's
/// whatever was baked into the image (or nothing at all if DNS is broken).
async fn log_resolv_conf(user_id: i32) {
    match tokio::fs::read_to_string("/etc/resolv.conf").await {
        Ok(contents) => {
            tracing::info!(
                "[TG-CONNECT user={}] PHASE=net_env /etc/resolv.conf={:?}",
                user_id,
                truncate_for_log(&contents, 500)
            );
        }
        Err(e) => {
            tracing::warn!(
                "[TG-CONNECT user={}] PHASE=net_env /etc/resolv.conf unreadable chain=[{}]",
                user_id,
                format_std_err_chain(&e)
            );
        }
    }
}

/// Resolve the homeserver hostname to IP addresses using tokio's resolver.
/// This isolates DNS from TCP/TLS/HTTP. If this fails but HTTP_PROXY is set,
/// that's normal (proxy handles DNS); if this fails and no proxy is set,
/// that's the root cause.
async fn log_dns_lookup(user_id: i32, host: &str, port: u16) {
    let t0 = Instant::now();
    let target = format!("{}:{}", host, port);
    tracing::info!(
        "[TG-CONNECT user={}] PHASE=dns_lookup resolving {}",
        user_id,
        target
    );
    let result = tokio::net::lookup_host(target.clone()).await;
    match result {
        Ok(addrs) => {
            let ips: Vec<String> = addrs.map(|a| a.to_string()).collect();
            if ips.is_empty() {
                tracing::error!(
                    "[TG-CONNECT user={}] PHASE=dns_lookup EMPTY no addresses for {} elapsed_ms={}",
                    user_id,
                    target,
                    t0.elapsed().as_millis()
                );
            } else {
                tracing::info!(
                    "[TG-CONNECT user={}] PHASE=dns_lookup OK {} -> [{}] elapsed_ms={}",
                    user_id,
                    target,
                    ips.join(", "),
                    t0.elapsed().as_millis()
                );
            }
        }
        Err(e) => {
            tracing::error!(
                "[TG-CONNECT user={}] PHASE=dns_lookup FAILED {} elapsed_ms={} chain=[{}]",
                user_id,
                target,
                t0.elapsed().as_millis(),
                format_std_err_chain(&e)
            );
        }
    }
}

/// Raw TCP connect probe - no TLS, no HTTP. If this fails, it's either
/// tap0 is dead, the vsock proxy on the host isn't running, or DNS gave
/// us an unreachable IP. Isolates transport-layer issues from TLS/HTTP.
async fn log_tcp_connect(user_id: i32, host: &str, port: u16) {
    let t0 = Instant::now();
    let target = format!("{}:{}", host, port);
    tracing::info!(
        "[TG-CONNECT user={}] PHASE=tcp_connect probing {}",
        user_id,
        target
    );
    match tokio::time::timeout(
        Duration::from_secs(3),
        tokio::net::TcpStream::connect(target.as_str()),
    )
    .await
    {
        Ok(Ok(stream)) => {
            let peer = stream
                .peer_addr()
                .map(|a| a.to_string())
                .unwrap_or_else(|e| format!("<peer_addr err: {}>", e));
            tracing::info!(
                "[TG-CONNECT user={}] PHASE=tcp_connect OK peer={} elapsed_ms={}",
                user_id,
                peer,
                t0.elapsed().as_millis()
            );
            drop(stream);
        }
        Ok(Err(e)) => {
            tracing::error!(
                "[TG-CONNECT user={}] PHASE=tcp_connect FAILED {} elapsed_ms={} chain=[{}]",
                user_id,
                target,
                t0.elapsed().as_millis(),
                format_std_err_chain(&e)
            );
        }
        Err(_) => {
            tracing::error!(
                "[TG-CONNECT user={}] PHASE=tcp_connect TIMEOUT {} after 3s elapsed_ms={}",
                user_id,
                target,
                t0.elapsed().as_millis()
            );
        }
    }
}

/// Pre-flight network connectivity check. Issues a plain HTTPS GET to
/// `{homeserver}/_matrix/client/versions` with a short timeout so we can
/// tell apart "tap0/network is broken" from "Matrix protocol misbehaving".
/// This is the first place in the connect flow where we touch the network,
/// so any network-level failure (DNS, TCP connect, TLS, HTTP) shows up here
/// with a descriptive single-line error chain in the logs.
async fn preflight_homeserver_reachability(user_id: i32, homeserver: &str) {
    let t0 = Instant::now();
    let versions_url = format!(
        "{}/_matrix/client/versions",
        homeserver.trim_end_matches('/')
    );
    tracing::info!(
        "[TG-CONNECT user={}] PHASE=preflight GET {}",
        user_id,
        versions_url
    );
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(
                "[TG-CONNECT user={}] PHASE=preflight FAILED to build reqwest client chain=[{}]",
                user_id,
                format_std_err_chain(&e)
            );
            return;
        }
    };
    match client.get(&versions_url).send().await {
        Ok(resp) => {
            let status = resp.status();
            let body_snippet = match resp.text().await {
                Ok(t) => truncate_for_log(&t, 200),
                Err(e) => format!("<body read failed: {}>", format_std_err_chain(&e)),
            };
            if status.is_success() {
                tracing::info!(
                    "[TG-CONNECT user={}] PHASE=preflight OK status={} elapsed_ms={} body={:?}",
                    user_id,
                    status,
                    t0.elapsed().as_millis(),
                    body_snippet
                );
            } else {
                tracing::warn!(
                    "[TG-CONNECT user={}] PHASE=preflight HTTP non-2xx status={} elapsed_ms={} body={:?} (homeserver REACHABLE but returned error status)",
                    user_id,
                    status,
                    t0.elapsed().as_millis(),
                    body_snippet
                );
            }
        }
        Err(e) => {
            // Walk the chain so we can see whether it's DNS, TCP connect,
            // TLS, or request-body level. This is the log line that will
            // clearly say "tap0 is broken" if the network is down.
            tracing::error!(
                "[TG-CONNECT user={}] PHASE=preflight NETWORK FAILURE elapsed_ms={} is_timeout={} is_connect={} is_request={} is_body={} chain=[{}]",
                user_id,
                t0.elapsed().as_millis(),
                e.is_timeout(),
                e.is_connect(),
                e.is_request(),
                e.is_body(),
                format_std_err_chain(&e)
            );
        }
    }
}

// Helper function to detect the one-time key conflict error
fn is_one_time_key_conflict(error: &anyhow::Error) -> bool {
    if let Some(http_err) = error.downcast_ref::<matrix_sdk::HttpError>() {
        let error_str = http_err.to_string();
        return error_str.contains("One time key") && error_str.contains("already exists");
    }
    false
}

/// Extract connected account (username or phone) from bridge status message
/// For Telegram, it might be username like @username or phone number
fn extract_connected_account(message: &str) -> Option<String> {
    // First try to find a phone number pattern
    if let Ok(re) = regex::Regex::new(r"\+\d{6,15}") {
        if let Some(m) = re.find(message) {
            return Some(m.as_str().to_string());
        }
    }
    // Then try to find @username pattern
    if let Ok(re) = regex::Regex::new(r"@[\w]+") {
        if let Some(m) = re.find(message) {
            return Some(m.as_str().to_string());
        }
    }
    None
}

// Helper function to get the store path
fn get_store_path(username: &str) -> Result<String> {
    let persistent_store_path = std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
        .map_err(|_| anyhow!("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"))?;
    Ok(format!("{}/{}", persistent_store_path, username))
}

// Wrapper function with retry logic
async fn connect_telegram_with_retry(
    client: &mut Arc<MatrixClient>,
    bridge_bot: &str,
    user_id: i32,
    state: &Arc<AppState>,
) -> Result<(OwnedRoomId, String)> {
    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY: Duration = Duration::from_secs(2);

    let username = client
        .user_id()
        .ok_or_else(|| anyhow!("User ID not available"))?
        .localpart()
        .to_string();

    tracing::info!(
        "[TG-CONNECT user={}] PHASE=retry_loop starting (max={}) matrix_username={}",
        user_id,
        MAX_RETRIES,
        username
    );

    for retry_count in 0..MAX_RETRIES {
        let attempt_start = Instant::now();
        tracing::info!(
            "[TG-CONNECT user={}] PHASE=retry_loop attempt={}/{}",
            user_id,
            retry_count + 1,
            MAX_RETRIES
        );
        match connect_telegram(client, bridge_bot, user_id).await {
            Ok(result) => {
                tracing::info!(
                    "[TG-CONNECT user={}] PHASE=retry_loop attempt={} SUCCEEDED elapsed_ms={}",
                    user_id,
                    retry_count + 1,
                    attempt_start.elapsed().as_millis()
                );
                return Ok(result);
            }
            Err(e) => {
                tracing::warn!(
                    "[TG-CONNECT user={}] PHASE=retry_loop attempt={} FAILED elapsed_ms={} err={:?}",
                    user_id,
                    retry_count + 1,
                    attempt_start.elapsed().as_millis(),
                    e
                );
                if retry_count < MAX_RETRIES - 1 && is_one_time_key_conflict(&e) {
                    tracing::warn!(
                        "[TG-CONNECT user={}] PHASE=retry_loop one-time key conflict detected (attempt {}/{}), resetting client store",
                        user_id,
                        retry_count + 1,
                        MAX_RETRIES
                    );

                    // Clear the store
                    let store_path = get_store_path(&username)?;
                    if Path::new(&store_path).exists() {
                        fs::remove_dir_all(&store_path).await?;
                        sleep(Duration::from_millis(500)).await; // Small delay before recreation
                        fs::create_dir_all(&store_path).await?;
                        tracing::info!(
                            "[TG-CONNECT user={}] PHASE=retry_loop cleared store directory: {}",
                            user_id,
                            store_path
                        );
                    }

                    // Add delay before retry
                    sleep(RETRY_DELAY).await;

                    // Reinitialize client
                    match matrix_auth::get_cached_client(user_id, state).await {
                        Ok(new_client) => {
                            *client = new_client; // Update the client reference
                            tracing::info!(
                                "[TG-CONNECT user={}] PHASE=retry_loop client reinitialized, retrying",
                                user_id
                            );
                            continue;
                        }
                        Err(init_err) => {
                            tracing::error!(
                                "[TG-CONNECT user={}] PHASE=retry_loop FAILED to reinitialize client: {:?}",
                                user_id,
                                init_err
                            );
                            return Err(init_err);
                        }
                    }
                } else if is_one_time_key_conflict(&e) {
                    return Err(anyhow!(
                        "Failed after {} attempts to resolve one-time key conflict: {}",
                        MAX_RETRIES,
                        e
                    ));
                } else {
                    return Err(e);
                }
            }
        }
    }

    Err(anyhow!("Exceeded maximum retry attempts ({})", MAX_RETRIES))
}

#[derive(Serialize)]
pub struct TelegramConnectionResponse {
    login_url: String,
}

async fn connect_telegram(
    client: &MatrixClient,
    bridge_bot: &str,
    user_id: i32,
) -> Result<(OwnedRoomId, String)> {
    let fn_start = Instant::now();
    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=init entered connect_telegram bot={}",
        user_id,
        bridge_bot
    );

    let bot_user_id = OwnedUserId::try_from(bridge_bot).map_err(|e| {
        tracing::error!(
            "[TG-CONNECT user={}] SUBPHASE=init invalid bridge_bot={:?} err={:?}",
            user_id,
            bridge_bot,
            e
        );
        anyhow!("Invalid bridge_bot user id: {}", e)
    })?;

    // --- create room ---
    let t_room = Instant::now();
    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=create_room calling client.create_room()...",
        user_id
    );
    let request = CreateRoomRequest::new();
    let response = client.create_room(request).await.map_err(|e| {
        tracing::error!(
            "[TG-CONNECT user={}] SUBPHASE=create_room FAILED elapsed_ms={} chain=[{}]",
            user_id,
            t_room.elapsed().as_millis(),
            format_std_err_chain(&e)
        );
        anyhow::Error::from(e)
    })?;
    let room_id = response.room_id();
    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=create_room OK room_id={} elapsed_ms={}",
        user_id,
        room_id,
        t_room.elapsed().as_millis()
    );

    let room = client.get_room(room_id).ok_or_else(|| {
        tracing::error!(
            "[TG-CONNECT user={}] SUBPHASE=create_room room lookup returned None for room_id={}",
            user_id,
            room_id
        );
        anyhow!("Room not found")
    })?;

    // --- invite bot ---
    let t_invite = Instant::now();
    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=invite_bot inviting {}",
        user_id,
        bot_user_id
    );
    room.invite_user_by_id(&bot_user_id).await.map_err(|e| {
        tracing::error!(
            "[TG-CONNECT user={}] SUBPHASE=invite_bot FAILED elapsed_ms={} chain=[{}]",
            user_id,
            t_invite.elapsed().as_millis(),
            format_std_err_chain(&e)
        );
        anyhow::Error::from(e)
    })?;
    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=invite_bot OK elapsed_ms={}",
        user_id,
        t_invite.elapsed().as_millis()
    );

    // --- first sync so the invite is processed ---
    // Use a room filter so sync only processes the telegram bot room, not all
    // rooms. Without this, sync_once processes hundreds of old messages across
    // every room, saturating the Tokio runtime and making the backend
    // unresponsive to health checks (which triggers the watchdog restart).
    let tg_room_filter = {
        use matrix_sdk::ruma::api::client::filter::{FilterDefinition, RoomFilter};
        use matrix_sdk::ruma::api::client::sync::sync_events::v3::Filter;
        let mut room_filter = RoomFilter::default();
        room_filter.rooms = Some(vec![room_id.into()]);
        let mut filter_def = FilterDefinition::default();
        filter_def.room = room_filter;
        Filter::FilterDefinition(filter_def)
    };

    let t_sync1 = Instant::now();
    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=sync1 calling sync_once(timeout=5s) to process invitation (filtered to room {})...",
        user_id,
        room_id
    );
    client
        .sync_once(
            MatrixSyncSettings::default()
                .timeout(Duration::from_secs(5))
                .filter(tg_room_filter.clone()),
        )
        .await
        .map_err(|e| {
            tracing::error!(
                "[TG-CONNECT user={}] SUBPHASE=sync1 FAILED elapsed_ms={} chain=[{}]",
                user_id,
                t_sync1.elapsed().as_millis(),
                format_std_err_chain(&e)
            );
            anyhow::Error::from(e)
        })?;
    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=sync1 OK elapsed_ms={}",
        user_id,
        t_sync1.elapsed().as_millis()
    );

    // --- wait for bot to join (15 x 500ms = 7.5s) ---
    let t_join = Instant::now();
    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=bot_join waiting for bot to join (max 15x500ms = 7.5s)...",
        user_id
    );
    let mut bot_joined = false;
    for attempt in 1..=15 {
        let members = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    "[TG-CONNECT user={}] SUBPHASE=bot_join attempt={}/15 members() FAILED chain=[{}]",
                    user_id,
                    attempt,
                    format_std_err_chain(&e)
                );
                sleep(Duration::from_millis(500)).await;
                continue;
            }
        };
        tracing::debug!(
            "[TG-CONNECT user={}] SUBPHASE=bot_join attempt={}/15 joined_count={} looking_for={}",
            user_id,
            attempt,
            members.len(),
            bot_user_id
        );
        if members.iter().any(|m| m.user_id() == bot_user_id) {
            bot_joined = true;
            tracing::info!(
                "[TG-CONNECT user={}] SUBPHASE=bot_join OK attempt={}/15 elapsed_ms={}",
                user_id,
                attempt,
                t_join.elapsed().as_millis()
            );
            break;
        }
        sleep(Duration::from_millis(500)).await;
    }

    if !bot_joined {
        // Detailed membership dump so we can see who IS in the room
        let all_members = room
            .members(matrix_sdk::RoomMemberships::empty())
            .await
            .map_err(|e| {
                tracing::error!(
                    "[TG-CONNECT user={}] SUBPHASE=bot_join FAILED to fetch full members list chain=[{}]",
                    user_id,
                    format_std_err_chain(&e)
                );
                anyhow::Error::from(e)
            })?;
        let member_summary: Vec<String> = all_members
            .iter()
            .map(|m| format!("{}={:?}", m.user_id(), m.membership()))
            .collect();
        tracing::error!(
            "[TG-CONNECT user={}] SUBPHASE=bot_join FAILED bot={} never joined after 7.5s elapsed_ms={} all_members=[{}]",
            user_id,
            bot_user_id,
            t_join.elapsed().as_millis(),
            member_summary.join(", ")
        );
        return Err(anyhow!("Bot {} failed to join room", bot_user_id));
    }

    // --- clean up stale login session (!tg logout / !tg cancel) ---
    let t_cleanup = Instant::now();
    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=cleanup sending !tg logout to clear stale session",
        user_id
    );
    if let Err(e) = room
        .send(RoomMessageEventContent::text_plain("!tg logout"))
        .await
    {
        tracing::warn!(
            "[TG-CONNECT user={}] SUBPHASE=cleanup !tg logout send FAILED (continuing) chain=[{}]",
            user_id,
            format_std_err_chain(&e)
        );
    }
    sleep(Duration::from_secs(1)).await;

    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=cleanup sending !tg cancel",
        user_id
    );
    if let Err(e) = room
        .send(RoomMessageEventContent::text_plain("!tg cancel"))
        .await
    {
        tracing::warn!(
            "[TG-CONNECT user={}] SUBPHASE=cleanup !tg cancel send FAILED (continuing) chain=[{}]",
            user_id,
            format_std_err_chain(&e)
        );
    }
    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=cleanup DONE elapsed_ms={}",
        user_id,
        t_cleanup.elapsed().as_millis()
    );

    // --- send !tg login ---
    let t_login_send = Instant::now();
    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=send_login sending !tg login",
        user_id
    );
    room.send(RoomMessageEventContent::text_plain("!tg login"))
        .await
        .map_err(|e| {
            tracing::error!(
                "[TG-CONNECT user={}] SUBPHASE=send_login FAILED elapsed_ms={} chain=[{}]",
                user_id,
                t_login_send.elapsed().as_millis(),
                format_std_err_chain(&e)
            );
            anyhow::Error::from(e)
        })?;
    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=send_login OK elapsed_ms={} now polling up to 60x for URL response (max ~30s)",
        user_id,
        t_login_send.elapsed().as_millis()
    );

    // --- poll for login URL ---
    let t_poll = Instant::now();
    let mut login_url: Option<String> = None;
    let mut bot_message_count = 0usize;
    let mut last_bot_body: Option<String> = None;
    // Track polls where sync_once returned Ok() but we saw no bot messages
    // at all. If this stays high, either the network is flaky (requests
    // returning quickly with no payload) or the bridge bot just hasn't
    // spoken yet. Either way we want the diagnostic in logs.
    let mut quiet_polls: u32 = 0;

    let sync_settings = MatrixSyncSettings::default()
        .timeout(Duration::from_millis(1500))
        .filter(tg_room_filter);

    'poll: for attempt in 1..=60 {
        if attempt == 1 || attempt <= 3 || attempt % 10 == 0 {
            tracing::info!(
                "[TG-CONNECT user={}] SUBPHASE=url_poll attempt={}/60 poll_elapsed_ms={} bot_msgs_seen={}",
                user_id,
                attempt,
                t_poll.elapsed().as_millis(),
                bot_message_count
            );
        }

        // Time sync_once so we can tell apart "proxy killed the long-poll"
        // (returns Ok in << 1500ms) from "normal sync" (~1500ms).
        let t_sync = Instant::now();
        let sync_result = client.sync_once(sync_settings.clone()).await;
        let sync_ms = t_sync.elapsed().as_millis();
        if let Err(e) = sync_result {
            tracing::warn!(
                "[TG-CONNECT user={}] SUBPHASE=url_poll attempt={}/60 sync_once FAILED sync_ms={} chain=[{}]",
                user_id,
                attempt,
                sync_ms,
                format_std_err_chain(&e)
            );
            sleep(Duration::from_millis(500)).await;
            continue;
        }
        if attempt == 1 || attempt <= 3 || attempt % 10 == 0 {
            // Budget is 1500ms. Anything <100ms on success is suspicious
            // (suggests the proxy is returning empty responses immediately).
            let suspicious = if sync_ms < 100 {
                " SUSPICIOUS_FAST"
            } else {
                ""
            };
            tracing::info!(
                "[TG-CONNECT user={}] SUBPHASE=url_poll attempt={}/60 sync_once OK sync_ms={} budget_ms=1500{}",
                user_id,
                attempt,
                sync_ms,
                suspicious
            );
        }

        if let Some(room) = client.get_room(room_id) {
            let mut options =
                matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            options.limit = matrix_sdk::ruma::UInt::new(5).unwrap();
            let messages = match room.messages(options).await {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(
                        "[TG-CONNECT user={}] SUBPHASE=url_poll attempt={}/60 messages() FAILED chain=[{}]",
                        user_id,
                        attempt,
                        format_std_err_chain(&e)
                    );
                    sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

            // Diagnostic: how many events in the room at all, and how many
            // from the bot specifically. Helps distinguish "network is fine,
            // bot is silent" from "room.messages() returned nothing".
            let total_events = messages.chunk.len();
            let bot_events_this_poll = messages
                .chunk
                .iter()
                .filter_map(|m| m.raw().deserialize().ok())
                .filter(|ev| ev.sender() == bot_user_id)
                .count();
            if bot_events_this_poll == 0 {
                quiet_polls += 1;
                // Log at intervals so we don't spam
                if quiet_polls == 1 || quiet_polls.is_multiple_of(10) {
                    tracing::info!(
                        "[TG-CONNECT user={}] SUBPHASE=url_poll attempt={}/60 QUIET quiet_streak={} total_events_in_window={} bot_events_in_window=0",
                        user_id,
                        attempt,
                        quiet_polls,
                        total_events
                    );
                }
            } else {
                quiet_polls = 0;
            }

            for msg in messages.chunk.iter() {
                let raw_event = msg.raw();
                let event = match raw_event.deserialize() {
                    Ok(ev) => ev,
                    Err(_) => continue,
                };
                if event.sender() != bot_user_id {
                    continue;
                }
                let sync_event = match event.clone() {
                    AnySyncTimelineEvent::MessageLike(
                        matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(se),
                    ) => se,
                    _ => continue,
                };
                let event_content: RoomMessageEventContent = match sync_event {
                    SyncRoomMessageEvent::Original(original_event) => original_event.content,
                    SyncRoomMessageEvent::Redacted(_) => continue,
                };
                let message_body = match event_content.msgtype {
                    MessageType::Notice(text_content) => text_content.body,
                    MessageType::Text(text_content) => text_content.body,
                    other => {
                        tracing::debug!(
                            "[TG-CONNECT user={}] SUBPHASE=url_poll attempt={}/60 skipping bot msgtype={:?}",
                            user_id,
                            attempt,
                            std::mem::discriminant(&other)
                        );
                        continue;
                    }
                };

                bot_message_count += 1;
                let body_for_log = truncate_for_log(&message_body, 500);
                tracing::info!(
                    "[TG-CONNECT user={}] SUBPHASE=url_poll attempt={}/60 bot_msg#{} body={:?}",
                    user_id,
                    attempt,
                    bot_message_count,
                    body_for_log
                );
                last_bot_body = Some(message_body.clone());

                if let Some(url) = extract_login_url(&message_body) {
                    tracing::info!(
                        "[TG-CONNECT user={}] SUBPHASE=url_poll OK found login_url on attempt={}/60 url_len={} poll_elapsed_ms={}",
                        user_id,
                        attempt,
                        url.len(),
                        t_poll.elapsed().as_millis()
                    );
                    login_url = Some(url);
                    break 'poll;
                } else {
                    tracing::debug!(
                        "[TG-CONNECT user={}] SUBPHASE=url_poll attempt={}/60 bot msg had no URL pattern",
                        user_id,
                        attempt
                    );
                }
            }
        } else {
            tracing::warn!(
                "[TG-CONNECT user={}] SUBPHASE=url_poll attempt={}/60 client.get_room({}) returned None",
                user_id,
                attempt,
                room_id
            );
        }

        sleep(Duration::from_millis(500)).await;
    }

    let login_url = match login_url {
        Some(u) => u,
        None => {
            tracing::error!(
                "[TG-CONNECT user={}] SUBPHASE=url_poll TIMEOUT no login URL after 60 attempts poll_elapsed_ms={} bot_msgs_seen={} last_bot_body={:?}",
                user_id,
                t_poll.elapsed().as_millis(),
                bot_message_count,
                last_bot_body
                    .as_deref()
                    .map(|b| truncate_for_log(b, 800))
                    .unwrap_or_else(|| "<none>".to_string())
            );
            return Err(anyhow!(
                "Telegram login url not received within 30 seconds ({} bot messages seen). Please try again.",
                bot_message_count
            ));
        }
    };

    tracing::info!(
        "[TG-CONNECT user={}] SUBPHASE=done connect_telegram OK total_elapsed_ms={}",
        user_id,
        fn_start.elapsed().as_millis()
    );
    Ok((room_id.into(), login_url))
}

// Helper function to extract login url more efficiently
fn extract_login_url(message: &str) -> Option<String> {
    // Remove backticks and other formatting that might interfere
    let clean_message = message.replace('`', "").replace("*", "");

    // Match any plain https URL first. The bot response format varies by bridge version.
    let plain_url_re = regex::Regex::new(r#"https?://[^\s<>\")\]]+"#).ok()?;
    if let Some(found) = plain_url_re.find(&clean_message) {
        return Some(
            found
                .as_str()
                .trim_end_matches(['.', ',', ')', ']'])
                .to_string(),
        );
    }

    // Fallback for Markdown [text](url) format if the closing paren was excluded above.
    let markdown_re = regex::Regex::new(r"\((https?://[^\)]+)\)").ok()?;
    if let Some(captures) = markdown_re.captures(&clean_message) {
        return Some(captures[1].to_string());
    }

    None
}

pub async fn start_telegram_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<TelegramConnectionResponse>, (StatusCode, AxumJson<serde_json::Value>)> {
    let user_id = auth_user.user_id;
    let flow_start = Instant::now();
    tracing::info!(
        "[TG-CONNECT user={}] ==== start_telegram_connection invoked ====",
        user_id
    );

    // Clean up any stale telegram bridge records from previous failed attempts
    let cleanup_start = Instant::now();
    match state.user_repository.delete_bridge(user_id, "telegram") {
        Ok(_) => tracing::info!(
            "[TG-CONNECT user={}] PHASE=cleanup stale bridge cleared elapsed_ms={}",
            user_id,
            cleanup_start.elapsed().as_millis()
        ),
        Err(e) => tracing::warn!(
            "[TG-CONNECT user={}] PHASE=cleanup failed to clean stale telegram bridge record elapsed_ms={} err={}",
            user_id,
            cleanup_start.elapsed().as_millis(),
            e
        ),
    }

    // Check env vars early and log
    let bridge_bot = match std::env::var("TELEGRAM_BRIDGE_BOT") {
        Ok(v) => {
            tracing::info!(
                "[TG-CONNECT user={}] PHASE=env TELEGRAM_BRIDGE_BOT={}",
                user_id,
                v
            );
            v
        }
        Err(e) => {
            tracing::error!(
                "[TG-CONNECT user={}] PHASE=env FATAL TELEGRAM_BRIDGE_BOT not set: {}",
                user_id,
                e
            );
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Server misconfigured: TELEGRAM_BRIDGE_BOT not set"})),
            ));
        }
    };
    let homeserver_url = match std::env::var("MATRIX_HOMESERVER") {
        Ok(v) => {
            tracing::info!(
                "[TG-CONNECT user={}] PHASE=env MATRIX_HOMESERVER={}",
                user_id,
                v
            );
            Some(v)
        }
        Err(e) => {
            tracing::error!(
                "[TG-CONNECT user={}] PHASE=env MATRIX_HOMESERVER not set: {}",
                user_id,
                e
            );
            None
        }
    };

    // --- network pre-flight ---
    // The enclave's network path (tap0 -> vsock -> host proxy) is completely
    // different from a bare VPS. So before we even touch matrix-sdk, we probe
    // each OSI layer separately:
    //   1. PHASE=net_env - proxy / TLS env vars
    //   2. PHASE=net_env - /etc/resolv.conf contents
    //   3. PHASE=dns_lookup - DNS resolution of homeserver host
    //   4. PHASE=tcp_connect - raw TCP to homeserver host:port
    //   5. PHASE=preflight - plain HTTPS GET to /_matrix/client/versions
    // Each produces a distinct log so we can grep and know exactly which
    // layer broke after the enclave migration.
    log_network_env(user_id);
    log_resolv_conf(user_id).await;

    if let Some(ref hs) = homeserver_url {
        match url::Url::parse(hs) {
            Ok(parsed) => {
                let host = parsed.host_str().unwrap_or("").to_string();
                let port = parsed.port_or_known_default().unwrap_or(443);
                if host.is_empty() {
                    tracing::error!(
                        "[TG-CONNECT user={}] PHASE=preflight MATRIX_HOMESERVER has no host: {}",
                        user_id,
                        hs
                    );
                } else {
                    log_dns_lookup(user_id, &host, port).await;
                    log_tcp_connect(user_id, &host, port).await;
                }
            }
            Err(e) => {
                tracing::error!(
                    "[TG-CONNECT user={}] PHASE=preflight MATRIX_HOMESERVER parse FAILED url={:?} chain=[{}]",
                    user_id,
                    hs,
                    format_std_err_chain(&e)
                );
            }
        }
        preflight_homeserver_reachability(user_id, hs).await;
    } else {
        tracing::warn!(
            "[TG-CONNECT user={}] PHASE=preflight SKIPPED (MATRIX_HOMESERVER not set)",
            user_id
        );
    }

    tracing::info!(
        "[TG-CONNECT user={}] PHASE=get_client acquiring Matrix client (note: NOT inside the {}s connect timeout)...",
        user_id,
        TELEGRAM_CONNECT_TIMEOUT.as_secs()
    );
    // Get or create Matrix client using the centralized function.
    // NOTE: this is OUTSIDE the 45s TELEGRAM_CONNECT_TIMEOUT, so if matrix_auth hangs
    // here we'll see it in the timing log below and can tell it apart from a
    // connect_telegram timeout.
    let client_start = Instant::now();
    let client = matrix_auth::get_cached_client(user_id, &state)
        .await
        .map_err(|e| {
            tracing::error!(
                "[TG-CONNECT user={}] PHASE=get_client FAILED elapsed_ms={} chain=[{}]",
                user_id,
                client_start.elapsed().as_millis(),
                format_anyhow_chain(&e)
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to initialize Matrix client: {}", e)})),
            )
        })?;
    let client_elapsed_ms = client_start.elapsed().as_millis();
    tracing::info!(
        "[TG-CONNECT user={}] PHASE=get_client OK matrix_user={} elapsed_ms={}",
        user_id,
        client
            .user_id()
            .map(|u| u.to_string())
            .unwrap_or_else(|| "<unknown>".to_string()),
        client_elapsed_ms
    );

    tracing::info!(
        "[TG-CONNECT user={}] PHASE=connect_telegram starting (budget={}s, flow_elapsed_ms={})...",
        user_id,
        TELEGRAM_CONNECT_TIMEOUT.as_secs(),
        flow_start.elapsed().as_millis()
    );
    // Connect to Telegram bridge
    let connect_start = Instant::now();
    let mut client_clone = Arc::clone(&client);
    let (room_id, login_url) = match timeout(
        TELEGRAM_CONNECT_TIMEOUT,
        connect_telegram_with_retry(&mut client_clone, &bridge_bot, user_id, &state),
    )
    .await
    {
        Ok(Ok(result)) => {
            tracing::info!(
                "[TG-CONNECT user={}] PHASE=connect_telegram OK elapsed_ms={}",
                user_id,
                connect_start.elapsed().as_millis()
            );
            result
        }
        Ok(Err(e)) => {
            tracing::error!(
                "[TG-CONNECT user={}] PHASE=connect_telegram FAILED elapsed_ms={} chain=[{}]",
                user_id,
                connect_start.elapsed().as_millis(),
                format_anyhow_chain(&e)
            );
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to connect to Telegram bridge: {}", e)})),
            ));
        }
        Err(_) => {
            tracing::error!(
                "[TG-CONNECT user={}] PHASE=connect_telegram TIMEOUT after {}s (flow_total_elapsed_ms={}, get_client_elapsed_ms={}). Check preceding [TG-CONNECT] PHASE logs to see where inside connect_telegram we got stuck.",
                user_id,
                TELEGRAM_CONNECT_TIMEOUT.as_secs(),
                flow_start.elapsed().as_millis(),
                client_elapsed_ms
            );
            return Err((
                StatusCode::GATEWAY_TIMEOUT,
                AxumJson(json!({
                    "error": "Telegram connection timed out while waiting for the bridge. Please try again."
                })),
            ));
        }
    };

    tracing::info!(
        "[TG-CONNECT user={}] PHASE=login_url OK room_id={} login_url_len={} flow_elapsed_ms={}",
        user_id,
        room_id,
        login_url.len(),
        flow_start.elapsed().as_millis()
    );

    // Create bridge record
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let new_bridge = NewPgBridge {
        user_id,
        bridge_type: "telegram".to_string(),
        status: "connecting".to_string(),
        room_id: Some(room_id.to_string()),
        data: None,
        created_at: Some(current_time),
    };

    // Store bridge information
    let db_write_start = Instant::now();
    state
        .user_repository
        .create_bridge(new_bridge)
        .map_err(|e| {
            tracing::error!(
                "[TG-CONNECT user={}] PHASE=db_write FAILED elapsed_ms={} err={:?}",
                user_id,
                db_write_start.elapsed().as_millis(),
                e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to store bridge information"})),
            )
        })?;
    tracing::info!(
        "[TG-CONNECT user={}] PHASE=db_write OK bridge row inserted elapsed_ms={}",
        user_id,
        db_write_start.elapsed().as_millis()
    );

    // Spawn a task to monitor the connection status
    let state_clone = state.clone();
    let room_id_clone = room_id.clone();
    let bridge_bot_clone = bridge_bot.to_string();
    let client_clone = client.clone();

    tracing::info!(
        "[TG-CONNECT user={}] PHASE=spawn_monitor starting monitor task",
        user_id
    );
    tokio::spawn(async move {
        match monitor_telegram_connection(
            &client_clone,
            &room_id_clone,
            &bridge_bot_clone,
            user_id,
            state_clone,
        )
        .await
        {
            Ok(_) => {
                tracing::info!(
                    "[TG-MONITOR user={}] connection monitoring completed successfully",
                    user_id
                );
            }
            Err(e) => {
                tracing::error!(
                    "[TG-MONITOR user={}] connection monitoring FAILED err={:?}",
                    user_id,
                    e
                );
            }
        }
    });

    tracing::info!(
        "[TG-CONNECT user={}] ==== returning login_url to frontend, flow_total_elapsed_ms={} ====",
        user_id,
        flow_start.elapsed().as_millis()
    );
    Ok(AxumJson(TelegramConnectionResponse { login_url }))
}

pub async fn get_telegram_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::debug!("📊 Checking Telegram status for user {}", auth_user.user_id);
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "telegram")
        .map_err(|e| {
            tracing::error!("Failed to get Telegram bridge status: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Telegram status"})),
            )
        })?;

    match bridge {
        Some(bridge) => Ok(AxumJson(json!({
            "connected": bridge.status == "connected",
            "status": bridge.status,
            "created_at": bridge.created_at.unwrap_or(0),
            "connected_account": bridge.data,
        }))),
        None => Ok(AxumJson(json!({
            "connected": false,
            "status": "not_connected",
            "created_at": 0,
            "connected_account": null,
        }))),
    }
}

async fn monitor_telegram_connection(
    client: &MatrixClient,
    room_id: &OwnedRoomId,
    bridge_bot: &str,
    user_id: i32,
    state: Arc<AppState>,
) -> Result<(), anyhow::Error> {
    let monitor_start = Instant::now();
    tracing::info!(
        "[TG-MONITOR user={}] starting monitoring in room={} bot={} max_duration=10min",
        user_id,
        room_id,
        bridge_bot
    );
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;

    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(10));

    // Actively probe login status with "!tg ping" every PING_INTERVAL attempts.
    // The web login may send the "Logged in" notification to a different room
    // than the one we're monitoring, so passive waiting alone is unreliable.
    const PING_INTERVAL: u32 = 4;
    // Wait a bit before the first ping so the user has time to complete the web flow.
    const FIRST_PING_AT: u32 = 6;

    let mut total_bot_msgs_seen = 0usize;
    let mut last_bot_body: Option<String> = None;
    let mut quiet_polls: u32 = 0;

    for attempt in 1..=120 {
        // Send "!tg ping" to actively check login status
        if attempt >= FIRST_PING_AT && attempt % PING_INTERVAL == 0 {
            if let Some(room) = client.get_room(room_id) {
                tracing::info!(
                    "[TG-MONITOR user={}] attempt={}/120 sending !tg ping to check login status",
                    user_id,
                    attempt
                );
                if let Err(e) = room
                    .send(RoomMessageEventContent::text_plain("!tg ping"))
                    .await
                {
                    tracing::warn!(
                        "[TG-MONITOR user={}] attempt={}/120 !tg ping send FAILED chain=[{}]",
                        user_id,
                        attempt,
                        format_std_err_chain(&e)
                    );
                }
                // Give the bot a moment to respond
                sleep(Duration::from_secs(2)).await;
            }
        }

        if attempt <= 3 || attempt % 10 == 0 {
            tracing::info!(
                "[TG-MONITOR user={}] attempt={}/120 elapsed_ms={} bot_msgs_seen={}",
                user_id,
                attempt,
                monitor_start.elapsed().as_millis(),
                total_bot_msgs_seen
            );
        }

        // Sync Matrix events - tolerate transient errors.
        // Time sync_once so a proxy that kills long-poll connections is
        // visible: budget is 10s, suspiciously-fast is < 500ms.
        let t_sync = Instant::now();
        let sync_result = client.sync_once(sync_settings.clone()).await;
        let sync_ms = t_sync.elapsed().as_millis();
        if let Err(e) = sync_result {
            tracing::warn!(
                "[TG-MONITOR user={}] attempt={}/120 sync_once FAILED sync_ms={} chain=[{}]",
                user_id,
                attempt,
                sync_ms,
                format_std_err_chain(&e)
            );
            sleep(Duration::from_secs(5)).await;
            continue;
        }
        if attempt <= 3 || attempt % 10 == 0 {
            let suspicious = if sync_ms < 500 {
                " SUSPICIOUS_FAST"
            } else {
                ""
            };
            tracing::info!(
                "[TG-MONITOR user={}] attempt={}/120 sync_once OK sync_ms={} budget_ms=10000{}",
                user_id,
                attempt,
                sync_ms,
                suspicious
            );
        }

        if let Some(room) = client.get_room(room_id) {
            let mut options =
                matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            options.limit = matrix_sdk::ruma::UInt::new(50).unwrap();
            let messages = match room.messages(options).await {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(
                        "[TG-MONITOR user={}] attempt={}/120 messages() FAILED chain=[{}]",
                        user_id,
                        attempt,
                        format_std_err_chain(&e)
                    );
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            // Diagnostic: count events in window and bot-events specifically.
            let total_events = messages.chunk.len();
            let bot_events_this_poll = messages
                .chunk
                .iter()
                .filter_map(|m| m.raw().deserialize().ok())
                .filter(|ev| ev.sender() == bot_user_id)
                .count();
            if bot_events_this_poll == 0 {
                quiet_polls += 1;
                if quiet_polls == 1 || quiet_polls.is_multiple_of(12) {
                    tracing::info!(
                        "[TG-MONITOR user={}] attempt={}/120 QUIET quiet_streak={} total_events_in_window={} bot_events_in_window=0 elapsed_ms={}",
                        user_id,
                        attempt,
                        quiet_polls,
                        total_events,
                        monitor_start.elapsed().as_millis()
                    );
                }
            } else {
                quiet_polls = 0;
            }

            for msg in messages.chunk {
                let raw_event = msg.raw();
                if let Ok(event) = raw_event.deserialize() {
                    if event.sender() == bot_user_id {
                        if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(
                                sync_event,
                            ),
                        ) = event
                        {
                            let event_content = match sync_event {
                                SyncRoomMessageEvent::Original(original_event) => {
                                    original_event.content
                                }
                                SyncRoomMessageEvent::Redacted(_) => continue,
                            };

                            let content = match event_content.msgtype {
                                MessageType::Text(text_content) => text_content.body,
                                MessageType::Notice(notice_content) => notice_content.body,
                                _ => continue,
                            };

                            total_bot_msgs_seen += 1;
                            let body_for_log = truncate_for_log(&content, 500);
                            tracing::info!(
                                "[TG-MONITOR user={}] attempt={}/120 bot_msg#{} body={:?}",
                                user_id,
                                attempt,
                                total_bot_msgs_seen,
                                body_for_log
                            );
                            last_bot_body = Some(content.clone());

                            // Check for successful login. We match the verified
                            // `!tg ping` healthy response: "You're logged in as @username".
                            // Empirically confirmed against mautrix-telegram v0.15.3.
                            // The substring "logged in as" is present in healthy
                            // responses but NOT in unhealthy responses like
                            // "That command requires you to be logged in.", so this
                            // is the safe specific match.
                            let content_lower = content.to_lowercase();
                            if content_lower.contains("logged in as")
                                || content_lower.contains("already logged in")
                            {
                                tracing::info!(
                                    "[TG-MONITOR user={}] SUCCESS detected logged-in pattern elapsed_ms={}",
                                    user_id,
                                    monitor_start.elapsed().as_millis()
                                );

                                // Extract the connected account (username or phone)
                                let connected_account = extract_connected_account(&content);
                                if let Some(ref account) = connected_account {
                                    tracing::info!(
                                        "[TG-MONITOR user={}] connected_account={}",
                                        user_id,
                                        account
                                    );
                                } else {
                                    tracing::warn!(
                                        "[TG-MONITOR user={}] could not extract connected_account from success message",
                                        user_id
                                    );
                                }

                                let current_time = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs()
                                    as i32;
                                let new_bridge = NewPgBridge {
                                    user_id,
                                    bridge_type: "telegram".to_string(),
                                    status: "connected".to_string(),
                                    room_id: Some(room_id.to_string()),
                                    data: connected_account,
                                    created_at: Some(current_time),
                                };
                                state.user_repository.delete_bridge(user_id, "telegram")?;
                                state.user_repository.create_bridge(new_bridge)?;

                                // Add client to app state and start sync
                                let mut matrix_clients = state.matrix_clients.lock().await;
                                let mut sync_tasks = state.matrix_sync_tasks.lock().await;

                                let state_for_handler = Arc::clone(&state);
                                client.add_event_handler(move |ev: matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent, room: matrix_sdk::room::Room, client| {
                                    let state = Arc::clone(&state_for_handler);
                                    async move {
                                        tracing::debug!("Received message in room {}", room.room_id());
                                        crate::utils::bridge::handle_bridge_message(ev, room, client, state).await;
                                    }
                                });

                                let client_arc = Arc::new(client.clone());
                                matrix_clients.insert(user_id, client_arc.clone());

                                let sync_settings = MatrixSyncSettings::default()
                                    .timeout(Duration::from_secs(30))
                                    .full_state(true);

                                let handle = tokio::spawn(async move {
                                    loop {
                                        match client_arc.sync(sync_settings.clone()).await {
                                            Ok(_) => {
                                                tracing::debug!(
                                                    "Sync completed normally for user {}",
                                                    user_id
                                                );
                                                tokio::time::sleep(Duration::from_secs(1)).await;
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "Matrix sync error for user {}: {}",
                                                    user_id,
                                                    e
                                                );
                                                tokio::time::sleep(Duration::from_secs(30)).await;
                                            }
                                        }
                                    }
                                });

                                // Abort old sync task to prevent duplicate message processing
                                if let Some(old_task) = sync_tasks.remove(&user_id) {
                                    old_task.abort();
                                }
                                sync_tasks.insert(user_id, handle);

                                if let Some(room) = client.get_room(room_id) {
                                    if let Err(e) = room
                                        .send(RoomMessageEventContent::text_plain("sync contacts"))
                                        .await
                                    {
                                        tracing::warn!(
                                            "Failed to send contacts sync for user {}: {}",
                                            user_id,
                                            e
                                        );
                                    }
                                    sleep(Duration::from_millis(500)).await;
                                    if let Err(e) = room
                                        .send(RoomMessageEventContent::text_plain("sync chats"))
                                        .await
                                    {
                                        tracing::warn!(
                                            "Failed to send chats sync for user {}: {}",
                                            user_id,
                                            e
                                        );
                                    }
                                }

                                return Ok(());
                            }

                            // Only treat fatal errors as failures - skip transient ones
                            let fatal_patterns =
                                ["authentication failed", "login failed", "invalid code"];
                            if fatal_patterns
                                .iter()
                                .any(|&pattern| content.to_lowercase().contains(pattern))
                            {
                                tracing::error!(
                                    "[TG-MONITOR user={}] FATAL pattern matched body={:?} elapsed_ms={}",
                                    user_id,
                                    truncate_for_log(&content, 500),
                                    monitor_start.elapsed().as_millis()
                                );
                                state.user_repository.delete_bridge(user_id, "telegram")?;
                                return Err(anyhow!("Telegram connection failed: {}", content));
                            }
                        }
                    }
                }
            }
        } else {
            tracing::warn!(
                "[TG-MONITOR user={}] attempt={}/120 client.get_room({}) returned None",
                user_id,
                attempt,
                room_id
            );
        }

        sleep(Duration::from_secs(5)).await;
    }

    tracing::error!(
        "[TG-MONITOR user={}] TIMEOUT after 10min elapsed_ms={} total_bot_msgs_seen={} last_bot_body={:?}",
        user_id,
        monitor_start.elapsed().as_millis(),
        total_bot_msgs_seen,
        last_bot_body
            .as_deref()
            .map(|b| truncate_for_log(b, 800))
            .unwrap_or_else(|| "<none>".to_string())
    );
    state.user_repository.delete_bridge(user_id, "telegram")?;
    Err(anyhow!("Telegram connection timed out after 10 minutes"))
}

pub async fn resync_telegram(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::debug!(
        "Starting Telegram resync process for user {}",
        auth_user.user_id
    );

    // Get the bridge information first
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "telegram")
        .map_err(|e| {
            tracing::error!("Failed to get Telegram bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Telegram bridge info"})),
            )
        })?;

    let Some(bridge) = bridge else {
        return Err((
            StatusCode::BAD_REQUEST,
            AxumJson(json!({"error": "Telegram is not connected"})),
        ));
    };

    // Get Matrix client using the cached version
    let client = matrix_auth::get_cached_client(auth_user.user_id, &state)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get Matrix client: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to initialize Matrix client: {}", e)})),
            )
        })?;

    // Get the room
    let room_id = OwnedRoomId::try_from(bridge.room_id.unwrap_or_default()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Invalid room ID format"})),
        )
    })?;

    if let Some(room) = client.get_room(&room_id) {
        tracing::debug!("Setting up Matrix event handler");

        // Set up event handler for the Matrix client
        client.add_event_handler(|ev: SyncRoomMessageEvent| async move {
            match ev {
                SyncRoomMessageEvent::Original(_msg) => {
                    // Add more specific message handling logic here if needed
                }
                SyncRoomMessageEvent::Redacted(_) => {
                    tracing::debug!("Received redacted message event");
                }
            }
        });

        // Start continuous sync in the background
        let sync_client = client.clone();
        tokio::spawn(async move {
            tracing::info!("🔄 Starting continuous Matrix sync for Telegram bridge");
            let sync_settings = MatrixSyncSettings::default()
                .timeout(Duration::from_secs(30))
                .full_state(true);

            if let Err(e) = sync_client.sync(sync_settings).await {
                tracing::error!("❌ Matrix sync error: {}", e);
            }
            tracing::info!("🛑 Continuous sync ended");
        });

        // Give the sync a moment to start up
        sleep(Duration::from_secs(2)).await;

        tracing::debug!("📱 Sending Telegram sync commands");

        // First sync all contacts
        if let Err(e) = room
            .send(RoomMessageEventContent::text_plain("sync contacts"))
            .await
        {
            tracing::error!("Failed to send contacts sync command: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to send contacts sync command"})),
            ));
        }
        tracing::debug!("✅ Sent contacts sync command");

        // Wait a bit for contacts to sync
        sleep(Duration::from_secs(2)).await;

        // Then sync all chats
        if let Err(e) = room
            .send(RoomMessageEventContent::text_plain("sync chats"))
            .await
        {
            tracing::error!("Failed to send chats sync command: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to send chats sync command"})),
            ));
        }
        tracing::debug!("✅ Sent chats sync command");

        tracing::debug!(
            "✅ Telegram resync process completed for user {}",
            auth_user.user_id
        );
        Ok(AxumJson(json!({
            "message": "Telegram resync initiated successfully"
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            AxumJson(json!({"error": "Telegram bridge room not found"})),
        ))
    }
}

pub async fn disconnect_telegram(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!(
        "🔌 Starting Telegram disconnection process for user {}",
        auth_user.user_id
    );

    // Get the bridge information first
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "telegram")
        .map_err(|e| {
            tracing::error!("Failed to get Telegram bridge info: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Telegram bridge info"})),
            )
        })?;

    let Some(bridge) = bridge else {
        return Ok(AxumJson(json!({
            "message": "Telegram was not connected"
        })));
    };

    let room_id_str = bridge.room_id.clone().unwrap_or_default();

    // Delete the bridge record IMMEDIATELY - user sees instant response
    state
        .user_repository
        .delete_bridge(auth_user.user_id, "telegram")
        .map_err(|e| {
            tracing::error!("Failed to delete Telegram bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to delete bridge record"})),
            )
        })?;

    tracing::info!(
        "✅ Telegram bridge record deleted for user {}",
        auth_user.user_id
    );

    // Spawn background task for cleanup - don't block the response
    let state_clone = state.clone();
    let user_id = auth_user.user_id;
    tokio::spawn(async move {
        tracing::info!(
            "🧹 Starting background cleanup for Telegram user {}",
            user_id
        );

        // Get Matrix client for cleanup
        let client = match matrix_auth::get_cached_client(user_id, &state_clone).await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Background cleanup: Failed to get Matrix client: {}", e);
                return;
            }
        };

        // Get the room and send cleanup commands
        if let Ok(room_id) = OwnedRoomId::try_from(room_id_str.as_str()) {
            if let Some(room) = client.get_room(&room_id) {
                // Send logout command
                if let Err(e) = room
                    .send(RoomMessageEventContent::text_plain("logout"))
                    .await
                {
                    tracing::error!("Background cleanup: Failed to send logout command: {}", e);
                }
                sleep(Duration::from_secs(2)).await;

                // Send command to clean rooms
                if let Err(e) = room
                    .send(RoomMessageEventContent::text_plain("clean-rooms"))
                    .await
                {
                    tracing::error!(
                        "Background cleanup: Failed to send clean-rooms command: {}",
                        e
                    );
                }
                sleep(Duration::from_secs(2)).await;
            }
        }

        // Check for remaining active bridges and cleanup if none left
        let has_active_bridges = state_clone
            .user_repository
            .has_active_bridges(user_id)
            .unwrap_or(false);

        if !has_active_bridges {
            // Clear user store if no other bridges
            if let Some(user_id_matrix) = client.user_id() {
                let username = user_id_matrix.localpart().to_string();
                let store_path = match get_store_path(&username) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!("Background cleanup: Failed to get store path: {}", e);
                        return;
                    }
                };
                if Path::new(&store_path).exists() {
                    if let Err(e) = fs::remove_dir_all(&store_path).await {
                        tracing::error!("Background cleanup: Failed to clear user store: {}", e);
                    } else {
                        tracing::info!(
                            "Background cleanup: Cleared Matrix store for user {}",
                            user_id
                        );
                    }
                }
            }

            // Remove client and sync task
            let mut matrix_clients = state_clone.matrix_clients.lock().await;
            let mut sync_tasks = state_clone.matrix_sync_tasks.lock().await;

            if let Some(task) = sync_tasks.remove(&user_id) {
                task.abort();
                tracing::debug!("Background cleanup: Aborted sync task for user {}", user_id);
            }
            if matrix_clients.remove(&user_id).is_some() {
                tracing::debug!(
                    "Background cleanup: Removed Matrix client for user {}",
                    user_id
                );
            }
        }

        tracing::info!(
            "🧹 Background cleanup completed for Telegram user {}",
            user_id
        );
    });

    Ok(AxumJson(json!({
        "message": "Telegram disconnected successfully"
    })))
}

/// Health check endpoint using login command
/// Returns the actual Telegram connection status from the bridge
pub async fn check_telegram_health(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!("🏥 Checking Telegram health for user {}", auth_user.user_id);

    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "telegram")
        .map_err(|e| {
            tracing::error!("Failed to get Telegram bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Telegram bridge info"})),
            )
        })?;

    let Some(bridge) = bridge else {
        return Ok(AxumJson(json!({
            "healthy": false,
            "message": "Telegram is not connected"
        })));
    };

    let client = matrix_auth::get_cached_client(auth_user.user_id, &state)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get Matrix client: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to initialize Matrix client: {}", e)})),
            )
        })?;

    let room_id = OwnedRoomId::try_from(bridge.room_id.unwrap_or_default()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Invalid room ID format"})),
        )
    })?;

    let room = client.get_room(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            AxumJson(json!({"error": "Telegram bridge room not found"})),
        )
    })?;

    let bridge_bot = std::env::var("TELEGRAM_BRIDGE_BOT").expect("TELEGRAM_BRIDGE_BOT not set");
    let bot_user_id = OwnedUserId::try_from(bridge_bot.as_str()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Invalid bridge bot user ID"})),
        )
    })?;

    // mautrix-telegram is the older Python bridge (v0.15.3) which DOES support
    // `!tg ping` (read-only). On healthy login it returns text containing
    // "Logged in as @username" or similar. On not-logged-in it says "You're
    // not logged in" or "ping requires you to be logged in".
    tracing::info!(
        "📤 Sending !tg ping for health check for user {}",
        auth_user.user_id
    );
    let responses = match crate::utils::bridge::probe_bridge_room(
        &client,
        &room,
        &bot_user_id,
        "!tg ping",
        Duration::from_secs(8),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("⚠️ probe_bridge_room failed: {}", e);
            return Ok(AxumJson(json!({
                "healthy": false,
                "ambiguous": true,
                "message": format!("probe failed: {}", e)
            })));
        }
    };

    if responses.is_empty() {
        // No fresh response - bridge bot may be down or sync is lagging.
        // DO NOT delete the bridge record.
        tracing::warn!(
            "⚠️ Telegram health check: no response from bridge bot for user {}",
            auth_user.user_id
        );
        return Ok(AxumJson(json!({
            "healthy": false,
            "ambiguous": true,
            "message": "Bridge bot did not respond. The bridge may be temporarily unreachable."
        })));
    }

    let combined = responses.join("\n");
    let combined_lower = combined.to_lowercase();
    tracing::info!(
        "📨 Telegram ping response for user {}: {:?}",
        auth_user.user_id,
        combined
    );

    // Verified empirically against deployed mautrix-telegram v0.15.3:
    //   healthy   `!tg ping` reply: "You're logged in as @ahtavarasmus"
    //   unhealthy `!tg ping` reply: "That command requires you to be logged in."
    let healthy = combined_lower.contains("logged in as");
    let unhealthy = combined_lower.contains("requires you to be logged in")
        || combined_lower.contains("not logged in");

    if healthy && !unhealthy {
        tracing::info!(
            "✅ Telegram health check passed for user {}",
            auth_user.user_id
        );
        if let Some(account) = extract_connected_account(&combined) {
            if let Err(e) =
                state
                    .user_repository
                    .update_bridge_data(auth_user.user_id, "telegram", &account)
            {
                tracing::warn!("Failed to save connected account: {}", e);
            }
        }
        Ok(AxumJson(json!({
            "healthy": true,
            "message": combined,
        })))
    } else if unhealthy {
        tracing::warn!(
            "❌ Telegram health check: bridge says not logged in for user {}: {:?}",
            auth_user.user_id,
            combined
        );
        // DO NOT delete the bridge record. Report unhealthy and let the user
        // decide whether to reconnect.
        Ok(AxumJson(json!({
            "healthy": false,
            "message": combined,
        })))
    } else {
        // Unrecognized response - surface it but don't make a healthy/unhealthy
        // decision. Don't change anything.
        tracing::info!(
            "ℹ️ Telegram health check: ambiguous response for user {}: {:?}",
            auth_user.user_id,
            combined
        );
        Ok(AxumJson(json!({
            "healthy": false,
            "ambiguous": true,
            "message": combined,
        })))
    }
}
