use crate::api::matrix_client::MatrixClientInterface;
// Re-export trait types and pure functions for external use
pub use crate::api::matrix_client::{
    infer_service_from_room, is_call_event_message, is_disconnection_message, is_error_message,
    is_health_check_message, should_process_message, IncomingBridgeEvent, IncomingMessageContent,
    MatrixClientWrapper, RoomInterface, RoomWrapper,
};
use crate::UserCoreOps;
use anyhow::{anyhow, Result};
use matrix_sdk::{
    room::Room,
    ruma::{
        events::room::message::{MessageType, SyncRoomMessageEvent},
        events::AnySyncTimelineEvent,
    },
    Client as MatrixClient,
};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicI64, AtomicU64, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};

use crate::AppState;
use serde::{Deserialize, Serialize};

/// Atomic counters for `handle_bridge_message` invocations, exposed via the
/// `/api/admin/handler-stats` admin endpoint. Lets us confirm at runtime that
/// portal events are reaching the handler post-deploy without scraping logs.
/// Each pair tracks "invoked" (handler fired for this service) and "stored"
/// (message was successfully written to ont_messages — survives all filters).
pub static HANDLER_INVOCATIONS_TG: AtomicU64 = AtomicU64::new(0);
pub static HANDLER_INVOCATIONS_WA: AtomicU64 = AtomicU64::new(0);
pub static HANDLER_INVOCATIONS_SIGNAL: AtomicU64 = AtomicU64::new(0);
pub static HANDLER_STORED_TG: AtomicU64 = AtomicU64::new(0);
pub static HANDLER_STORED_WA: AtomicU64 = AtomicU64::new(0);
pub static HANDLER_STORED_SIGNAL: AtomicU64 = AtomicU64::new(0);
/// Bumped when `insert_message` returns is_new=false — i.e. matrix-sdk
/// re-sync redelivered an event we already stored, or the bridge re-emitted
/// a message close in time. Should grow alongside `invocations` if dedup is
/// doing real work; flat means we're not seeing duplicate deliveries.
pub static HANDLER_SKIPPED_DUPLICATE_TG: AtomicU64 = AtomicU64::new(0);
pub static HANDLER_SKIPPED_DUPLICATE_WA: AtomicU64 = AtomicU64::new(0);
pub static HANDLER_SKIPPED_DUPLICATE_SIGNAL: AtomicU64 = AtomicU64::new(0);
/// Set once on first handler invocation; readable by the stats endpoint to
/// compute "events per minute since boot".
pub static HANDLER_BOOT_TS: OnceLock<AtomicI64> = OnceLock::new();

fn bump_invocation(service: &str) {
    HANDLER_BOOT_TS.get_or_init(|| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        AtomicI64::new(now)
    });
    let counter = match service {
        "telegram" => &HANDLER_INVOCATIONS_TG,
        "whatsapp" => &HANDLER_INVOCATIONS_WA,
        "signal" => &HANDLER_INVOCATIONS_SIGNAL,
        _ => return,
    };
    counter.fetch_add(1, Ordering::Relaxed);
}

fn bump_stored(service: &str) {
    let counter = match service {
        "telegram" => &HANDLER_STORED_TG,
        "whatsapp" => &HANDLER_STORED_WA,
        "signal" => &HANDLER_STORED_SIGNAL,
        _ => return,
    };
    counter.fetch_add(1, Ordering::Relaxed);
}

fn bump_skipped_duplicate(service: &str) {
    let counter = match service {
        "telegram" => &HANDLER_SKIPPED_DUPLICATE_TG,
        "whatsapp" => &HANDLER_SKIPPED_DUPLICATE_WA,
        "signal" => &HANDLER_SKIPPED_DUPLICATE_SIGNAL,
        _ => return,
    };
    counter.fetch_add(1, Ordering::Relaxed);
}

fn should_cleanup_tuwunel_media(msgtype: &MessageType) -> bool {
    matches!(
        msgtype,
        MessageType::Image(_)
            | MessageType::Video(_)
            | MessageType::File(_)
            | MessageType::Audio(_)
    )
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BridgeRoom {
    pub room_id: String,
    pub display_name: String,
    pub last_activity: i64,
    pub last_activity_formatted: String,
    #[serde(default)]
    pub is_group: bool,
}

use chrono::DateTime;
use chrono_tz::Tz;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BridgeMessage {
    pub sender: String,
    pub sender_display_name: String,
    pub content: String,
    pub timestamp: i64,
    pub formatted_timestamp: String,
    pub message_type: String,
    pub room_name: String,
    pub media_url: Option<String>,
    pub room_id: Option<String>,
}

fn format_timestamp(timestamp: i64, timezone: Option<String>) -> String {
    // Convert timestamp to DateTime<Utc>
    let dt_utc = match DateTime::from_timestamp(timestamp, 0) {
        Some(dt) => dt,
        None => return "Invalid timestamp".to_string(),
    };

    // Convert to user's timezone if provided, otherwise use UTC
    let formatted = if let Some(tz_str) = timezone {
        match tz_str.parse::<Tz>() {
            Ok(tz) => dt_utc
                .with_timezone(&tz)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
            Err(_) => {
                tracing::warn!("Invalid timezone '{}', falling back to UTC", tz_str);
                dt_utc.format("%Y-%m-%d %H:%M:%S UTC").to_string()
            }
        }
    } else {
        dt_utc.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    };

    formatted
}

fn get_sender_prefix(service: &str) -> String {
    format!("{}_", service)
}

fn log_bridge_error_notice(
    direction: &str,
    user_id: i32,
    service: &str,
    room_id: &str,
    content: &str,
) {
    let lower = content.to_lowercase();
    let error_kind = if lower.contains("decrypt") {
        "decrypt"
    } else if lower.contains("media") {
        "media"
    } else if lower.contains("failed") {
        "bridge_failed"
    } else {
        "bridge_error"
    };

    tracing::warn!(
        user_id,
        service = %service,
        direction,
        room_id = %room_id,
        error_kind,
        "Bridge error notice skipped from notification pipeline"
    );
}

fn room_name_matches_service(room_name: &str, service: &str) -> bool {
    let room_name = room_name.trim().to_lowercase();
    match service {
        "whatsapp" => room_name.contains("(wa)") || room_name.contains("(wa~)"),
        "telegram" => room_name.contains("(telegram)") || room_name.contains("(tg)"),
        "signal" => room_name.contains("(signal)") || room_name.contains("(signal~)"),
        _ => false,
    }
}

fn has_service_member(service: &str, member_localparts: &[String]) -> bool {
    let sender_prefix = get_sender_prefix(service);
    member_localparts
        .iter()
        .any(|localpart| localpart.starts_with(&sender_prefix))
}

fn is_group_room(service: &str, member_localparts: &[String]) -> bool {
    let sender_prefix = get_sender_prefix(service);
    member_localparts
        .iter()
        .filter(|localpart| localpart.starts_with(&sender_prefix))
        .count()
        > 1
}

pub fn remove_bridge_suffix(chat_name: &str) -> String {
    let trimmed = chat_name.trim();
    let lower = trimmed.to_lowercase();
    for suffix in &[
        "(wa~)",
        "(wa)",
        "(signal~)",
        "(signal)",
        "(telegram)",
        "(tg)",
    ] {
        if lower.ends_with(suffix) {
            let end = trimmed.len().saturating_sub(suffix.len());
            return trimmed[..end].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn bridge_room_matches_search(display_name: &str, search_term_lower: &str) -> Option<f64> {
    let raw_lower = display_name.trim().to_lowercase();
    let clean = remove_bridge_suffix(display_name);
    let clean_lower = clean.to_lowercase();

    if raw_lower == search_term_lower || clean_lower == search_term_lower {
        Some(2.0)
    } else if raw_lower.contains(search_term_lower) || clean_lower.contains(search_term_lower) {
        Some(1.0)
    } else {
        let similarity = strsim::jaro_winkler(&clean_lower, search_term_lower)
            .max(strsim::jaro_winkler(&raw_lower, search_term_lower));
        (similarity >= 0.7).then_some(similarity)
    }
}

/// Returns whether the room name indicates a phone contact (from phone book).
/// (WA) / (Signal) = phone contact (has FullName/ContactName)
/// (WA~) / (Signal~) = not a phone contact (only PushName/ProfileName)
/// Telegram / unknown = None (can't determine)
pub fn is_phone_contact_from_room_name(room_name: &str) -> Option<bool> {
    let trimmed = room_name.trim();
    if trimmed.ends_with("(WA)") || trimmed.ends_with("(Signal)") {
        Some(true)
    } else if trimmed.ends_with("(WA~)") || trimmed.ends_with("(Signal~)") {
        Some(false)
    } else {
        None // Telegram, email, or unknown
    }
}

fn infer_service(room_name: &str, sender_localpart: &str) -> Option<String> {
    let sender_localpart = sender_localpart.trim().to_lowercase();
    let room_name = room_name.to_lowercase();

    if room_name.contains("(wa)")
        || room_name.contains("(wa~)")
        || sender_localpart.starts_with("whatsapp_")
        || sender_localpart.starts_with("whatsapp")
    {
        tracing::debug!("Detected WhatsApp");
        return Some("whatsapp".to_string());
    }
    if room_name.contains("(tg)")
        || sender_localpart.starts_with("telegram_")
        || sender_localpart.starts_with("telegram")
    {
        tracing::debug!("Detected Telegram");
        return Some("telegram".to_string());
    }
    if room_name.contains("(signal)")
        || room_name.contains("(signal~)")
        || sender_localpart.starts_with("signal_")
        || sender_localpart.starts_with("signal")
    {
        tracing::debug!("Detected Signal");
        return Some("signal".to_string());
    }
    tracing::debug!("No service detected");
    None
}

/// When `infer_service()` fails (sender is user, not a bridge ghost),
/// scan room members for ghost bot localparts to determine platform.
async fn infer_service_from_room_members(room: &Room) -> Option<String> {
    let members = room.members(matrix_sdk::RoomMemberships::JOIN).await.ok()?;
    for member in &members {
        let localpart = member.user_id().localpart();
        if localpart.starts_with("whatsapp_") {
            return Some("whatsapp".to_string());
        }
        if localpart.starts_with("telegram_") {
            return Some("telegram".to_string());
        }
        if localpart.starts_with("signal_") {
            return Some("signal".to_string());
        }
    }
    None
}

pub async fn get_service_rooms(client: &MatrixClient, service: &str) -> Result<Vec<BridgeRoom>> {
    let joined_rooms = client.joined_rooms();
    let service_cap = capitalize(service);
    let skip_terms = [
        format!("{}bot", service),
        format!("{}-bridge", service),
        format!("{} Bridge", service_cap),
        format!("{} bridge bot", service_cap),
    ];
    let mut futures = Vec::new();
    for room in joined_rooms {
        let skip_terms = skip_terms.clone();
        let service = service.to_string();
        futures.push(async move {
            let room_id_str = room.room_id().to_string();
            let timeout_room_id = room_id_str.clone();
            match tokio::time::timeout(Duration::from_secs(3), async move {
                let display_name = match room.display_name().await {
                    Ok(name) => name.to_string(),
                    Err(e) => {
                        tracing::warn!(
                            "get_service_rooms: room {} display_name() failed: {}",
                            room_id_str,
                            e
                        );
                        return None;
                    }
                };
                if skip_terms.iter().any(|t| display_name.contains(t)) {
                    tracing::info!(
                        "get_service_rooms: skipped bridge management/bot room for service={} room_id={}",
                        service,
                        room_id_str
                    );
                    return None;
                }
                // Check membership instead of last message sender
                let members = match room.members(RoomMemberships::JOIN).await {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!(
                            "get_service_rooms: members() failed for service={} room_id={} error={}",
                            service,
                            room_id_str,
                            e
                        );
                        return None;
                    }
                };
                let member_localparts: Vec<String> = members
                    .iter()
                    .map(|member| member.user_id().localpart().to_string())
                    .collect();
                let name_match = room_name_matches_service(&display_name, &service);
                let member_match = has_service_member(&service, &member_localparts);
                if !name_match && !member_match {
                    tracing::info!(
                        "get_service_rooms: skipped non-service room for service={} room_id={} member_count={}",
                        service,
                        room_id_str,
                        member_localparts.len()
                    );
                    return None;
                }
                tracing::info!(
                    "get_service_rooms: matched service room for service={} room_id={} name_match={} member_match={} member_count={}",
                    service,
                    room_id_str,
                    name_match,
                    member_match,
                    member_localparts.len()
                );
                let is_group = is_group_room(&service, &member_localparts);
                // Get last activity from most recent message, regardless of sender
                let mut options = matrix_sdk::room::MessagesOptions::backward();
                options.limit = matrix_sdk::ruma::UInt::new(1).unwrap();
                let last_activity = match room.messages(options).await {
                    Ok(response) => response
                        .chunk
                        .first()
                        .and_then(|event| event.raw().deserialize().ok())
                        .map(|e: AnySyncTimelineEvent| i64::from(e.origin_server_ts().0) / 1000)
                        .unwrap_or(0),
                    Err(_) => 0,
                };
                Some(BridgeRoom {
                    room_id: room.room_id().to_string(),
                    display_name,
                    last_activity,
                    last_activity_formatted: format_timestamp(last_activity, None),
                    is_group,
                })
            })
            .await
            {
                Ok(room) => room,
                Err(_) => {
                    tracing::warn!(
                        "get_service_rooms: room {} timed out while reading Matrix state",
                        timeout_room_id
                    );
                    None
                }
            }
        });
    }
    let results = join_all(futures).await;
    let mut rooms: Vec<BridgeRoom> = results.into_iter().flatten().collect();
    rooms.sort_by_key(|r| std::cmp::Reverse(r.last_activity));
    tracing::info!(
        "get_service_rooms: service={} matched_room_count={}",
        service,
        rooms.len()
    );
    Ok(rooms)
}

// ============================================================================
// Trait-Based Functions (for testability)
// ============================================================================

/// Get service rooms using the trait interface (testable version)
pub async fn get_service_rooms_trait(
    client: &dyn MatrixClientInterface,
    service: &str,
) -> Result<Vec<BridgeRoom>> {
    let joined_rooms = client.get_joined_rooms().await?;
    let service_cap = capitalize(service);
    let skip_terms = [
        format!("{}bot", service),
        format!("{}-bridge", service),
        format!("{} Bridge", service_cap),
        format!("{} bridge bot", service_cap),
    ];

    let mut rooms = Vec::new();
    for room_info in joined_rooms {
        // Skip management rooms
        if skip_terms
            .iter()
            .any(|t| room_info.display_name.contains(t))
        {
            continue;
        }

        // Get the room for more details
        if let Some(room) = client.get_room(&room_info.room_id).await {
            // Check membership for service users
            let members = match room.get_members().await {
                Ok(m) => m,
                Err(_) => continue,
            };
            let member_localparts: Vec<String> = members
                .iter()
                .map(|member| member.localpart.clone())
                .collect();

            if !room_name_matches_service(&room_info.display_name, service)
                && !has_service_member(service, &member_localparts)
            {
                continue;
            }

            let is_group = is_group_room(service, &member_localparts);
            let last_activity = room.get_last_activity().await;
            rooms.push(BridgeRoom {
                room_id: room_info.room_id,
                display_name: room_info.display_name,
                last_activity,
                last_activity_formatted: format_timestamp(last_activity, None),
                is_group,
            });
        }
    }

    rooms.sort_by_key(|r| std::cmp::Reverse(r.last_activity));
    Ok(rooms)
}

/// Send a bridge message using the trait interface (testable version)
pub async fn send_bridge_message_trait(
    client: &dyn MatrixClientInterface,
    service: &str,
    chat_name: &str,
    message: &str,
    media_url: Option<String>,
    timezone: Option<String>,
) -> Result<BridgeMessage> {
    let service_rooms = get_service_rooms_trait(client, service).await?;
    let exact_room = find_exact_room(&service_rooms, chat_name);

    let room = match exact_room {
        Some(room_info) => client
            .get_room(&room_info.room_id)
            .await
            .ok_or_else(|| anyhow!("Room not found"))?,
        None => {
            let suggestions = get_best_matches(&service_rooms, chat_name);
            let error_msg = if suggestions.is_empty() {
                format!(
                    "Could not find exact matching {} room for '{}'",
                    capitalize(service),
                    chat_name
                )
            } else {
                format!(
                    "Could not find exact matching {} room for '{}'. Did you mean one of these?\n{}",
                    capitalize(service),
                    chat_name,
                    suggestions.join("\n")
                )
            };
            return Err(anyhow!(error_msg));
        }
    };

    let is_image = media_url.is_some();
    if let Some(url) = media_url {
        // Download and upload image
        let resp = reqwest::get(&url).await?;
        let mime_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = resp.bytes().await?;
        let size = bytes.len() as u64;

        let mxc_uri = client.upload_media(&mime_type, bytes.to_vec()).await?;
        room.send_image(&mxc_uri, message, size).await?;
    } else {
        room.send_text(message).await?;
    }

    let room_name = room.display_name().await?;
    let room_id_str = room.room_id().to_string();
    let current_timestamp = chrono::Utc::now().timestamp();
    Ok(BridgeMessage {
        sender: "You".to_string(),
        sender_display_name: "You".to_string(),
        content: message.to_string(),
        timestamp: current_timestamp,
        formatted_timestamp: format_timestamp(current_timestamp, timezone),
        message_type: if is_image { "image" } else { "text" }.to_string(),
        room_name,
        media_url: None,
        room_id: Some(room_id_str),
    })
}

/// Fetch bridge messages using the trait interface (testable version)
pub async fn fetch_bridge_messages_trait(
    client: &dyn MatrixClientInterface,
    service: &str,
    start_time: i64,
    timezone: Option<String>,
) -> Result<Vec<BridgeMessage>> {
    let service_rooms = get_service_rooms_trait(client, service).await?;
    let sender_prefix = get_sender_prefix(service);

    let mut all_messages = Vec::new();

    // Process top 5 most active rooms
    for room_info in service_rooms.into_iter().take(5) {
        if let Some(room) = client.get_room(&room_info.room_id).await {
            // Skip muted rooms
            if room.is_muted().await {
                continue;
            }

            let room_name = remove_bridge_suffix(&room_info.display_name);

            // Fetch messages with the service prefix filter
            if let Ok(mut messages) = room.fetch_messages(50, Some(&sender_prefix)).await {
                // Filter by timestamp and format
                for msg in &mut messages {
                    if msg.timestamp > start_time {
                        msg.formatted_timestamp = format_timestamp(msg.timestamp, timezone.clone());
                        msg.room_name = room_name.clone();
                        all_messages.push(msg.clone());
                    }
                }
            }
        }
    }

    // Sort by timestamp (most recent first)
    all_messages.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
    Ok(all_messages)
}

// ============================================================================
// Original Functions (maintained for backward compatibility)
// ============================================================================

pub fn find_exact_room(bridge_rooms: &[BridgeRoom], search_term: &str) -> Option<BridgeRoom> {
    let search_term_lower = search_term.trim().to_lowercase();
    if let Some(room) = bridge_rooms
        .iter()
        .find(|r| remove_bridge_suffix(r.display_name.as_str()).to_lowercase() == search_term_lower)
    {
        tracing::info!("Found exact match for room");
        return Some(room.clone());
    }
    None
}

pub fn search_best_match(bridge_rooms: &[BridgeRoom], search_term: &str) -> Option<BridgeRoom> {
    let search_term_lower = search_term.trim().to_lowercase();
    // Try exact match first (fastest)
    if let Some(room) = bridge_rooms
        .iter()
        .find(|r| remove_bridge_suffix(r.display_name.as_str()).to_lowercase() == search_term_lower)
    {
        tracing::info!("Found exact match for room");
        return Some(room.clone());
    }
    // Then try substring match
    if let Some(room) = bridge_rooms
        .iter()
        .filter(|r| {
            remove_bridge_suffix(r.display_name.as_str())
                .to_lowercase()
                .contains(&search_term_lower)
        })
        .max_by_key(|r| r.last_activity)
    {
        tracing::info!("Found substring match for room");
        return Some(room.clone());
    }
    // Finally try similarity match
    let best_match = bridge_rooms
        .iter()
        .map(|r| {
            (
                strsim::jaro_winkler(
                    &search_term_lower,
                    &remove_bridge_suffix(r.display_name.as_str()).to_lowercase(),
                ),
                r,
            )
        })
        .filter(|(score, _)| *score >= 0.7)
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    if let Some((score, room)) = best_match {
        tracing::info!("Found similar match with score {}", score);
        Some(room.clone())
    } else {
        None
    }
}

pub fn get_best_matches(bridge_rooms: &[BridgeRoom], search_term: &str) -> Vec<String> {
    let search_term_lower = search_term.trim().to_lowercase();
    let mut matches: Vec<(f64, String)> = bridge_rooms
        .iter()
        .map(|r| {
            let name = remove_bridge_suffix(&r.display_name);
            let name_lower = name.to_lowercase();
            (
                strsim::jaro_winkler(&search_term_lower, &name_lower),
                name.to_string(),
            )
        })
        .filter(|(score, _)| *score >= 0.7)
        .collect();
    matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    matches.into_iter().take(5).map(|(_, name)| name).collect()
}

const AI_PROMPT_TEXT: &str = "Hi, I'm Lightfriend, your friend's AI assistant. This message looks time-sensitive—since they're not currently on their computer, would you like me to send them a notification about it? Reply \"yes\" or \"no.\"";

pub async fn get_triggering_message_in_room(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    room_id_str: &str,
) -> Result<Option<BridgeMessage>> {
    tracing::info!(
        "Fetching triggering message in {} - User: {}, room_id: {}",
        capitalize(service),
        user_id,
        room_id_str
    );

    // Validate bridge connection
    if let Some(bridge) = state.user_repository.get_bridge(user_id, service)? {
        if bridge.status != "connected" {
            return Err(anyhow!(
                "{} bridge is not connected. Please log in first.",
                capitalize(service)
            ));
        }
    } else {
        return Err(anyhow!("{} bridge not found", capitalize(service)));
    }

    // Get Matrix client
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;

    // Get user info for timezone
    let user_info = state.user_core.get_user_info(user_id)?;

    // Get the room
    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_str)?;
    let room = client.get_room(&room_id).ok_or(anyhow!("Room not found"))?;

    // Fetch room display name
    let room_display_name = room.display_name().await?.to_string();
    let cleaned_room_name = remove_bridge_suffix(&room_display_name);

    // Fetch messages backward (latest first)
    let mut options = MessagesOptions::backward();
    options.limit = matrix_sdk::ruma::UInt::new(100).unwrap(); // Limit to avoid fetching too many; increase if needed

    let response = room.messages(options).await?;

    // Sender prefix for bridge bots (incoming messages start with this)
    let sender_prefix = get_sender_prefix(service);

    // User's Matrix user ID (for sent messages)
    let user_matrix_id = client.user_id().ok_or(anyhow!("User ID not available"))?;

    // Iterate through messages from latest to oldest
    let mut found_prompt = false;
    for event in response.chunk {
        if let Ok(AnySyncTimelineEvent::MessageLike(
            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(
                SyncRoomMessageEvent::Original(e),
            ),
        )) = event.raw().deserialize()
        {
            let sender_localpart = e.sender.localpart().to_string();

            if !found_prompt {
                // Look for the AI prompt sent by the user
                if e.sender == user_matrix_id && !sender_localpart.starts_with(&sender_prefix) {
                    let body = match e.content.msgtype {
                        MessageType::Text(ref t) => t.body.clone(),
                        _ => continue,
                    };
                    if body.contains(AI_PROMPT_TEXT) {
                        found_prompt = true;
                        continue; // Skip to the next message (older)
                    }
                }
            } else {
                // After finding the prompt, look for the next incoming message
                if sender_localpart.starts_with(&sender_prefix) {
                    let timestamp = i64::from(e.origin_server_ts.0) / 1000;

                    // Extract message type and body
                    let (msgtype, body) = match e.content.msgtype {
                        MessageType::Text(t) => ("text", t.body),
                        MessageType::Notice(n) => ("notice", n.body),
                        MessageType::Image(i) => (
                            "image",
                            if i.body.is_empty() {
                                "📎 IMAGE".into()
                            } else {
                                i.body
                            },
                        ),
                        MessageType::Video(v) => (
                            "video",
                            if v.body.is_empty() {
                                "📎 VIDEO".into()
                            } else {
                                v.body
                            },
                        ),
                        MessageType::File(f) => (
                            "file",
                            if f.body.is_empty() {
                                "📎 FILE".into()
                            } else {
                                f.body
                            },
                        ),
                        MessageType::Audio(a) => (
                            "audio",
                            if a.body.is_empty() {
                                "📎 AUDIO".into()
                            } else {
                                a.body
                            },
                        ),
                        MessageType::Location(_l) => ("location", "📍 LOCATION".into()),
                        MessageType::Emote(t) => ("emote", t.body),
                        _ => continue,
                    };

                    // Skip error-like messages
                    if is_error_message(&body) {
                        continue;
                    }

                    return Ok(Some(BridgeMessage {
                        sender: e.sender.to_string(),
                        sender_display_name: sender_localpart,
                        content: body,
                        timestamp,
                        formatted_timestamp: format_timestamp(
                            timestamp,
                            user_info.timezone.clone(),
                        ),
                        message_type: msgtype.to_string(),
                        room_name: cleaned_room_name,
                        media_url: None,
                        room_id: Some(room_id.to_string()),
                    }));
                }
            }
        }
    }

    // If no triggering message found after the prompt
    tracing::info!(
        "No triggering incoming message found before the AI prompt in room '{}'",
        room_id_str
    );
    Ok(None)
}

pub async fn get_latest_sent_message_in_room(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    room_id_str: &str,
) -> Result<Option<BridgeMessage>> {
    tracing::info!(
        "Fetching latest sent message in {} - User: {}, room_id: {}",
        capitalize(service),
        user_id,
        room_id_str
    );

    // Validate bridge connection
    if let Some(bridge) = state.user_repository.get_bridge(user_id, service)? {
        if bridge.status != "connected" {
            return Err(anyhow!(
                "{} bridge is not connected. Please log in first.",
                capitalize(service)
            ));
        }
    } else {
        return Err(anyhow!("{} bridge not found", capitalize(service)));
    }

    // Get Matrix client
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;

    // Get user info for timezone
    let user_info = state.user_core.get_user_info(user_id)?;

    // Get the room
    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_str)?;
    let room = client.get_room(&room_id).ok_or(anyhow!("Room not found"))?;

    // Fetch room display name
    let room_display_name = room.display_name().await?.to_string();
    let cleaned_room_name = remove_bridge_suffix(&room_display_name);

    // Fetch messages backward (latest first)
    let mut options = MessagesOptions::backward();
    options.limit = matrix_sdk::ruma::UInt::new(100).unwrap(); // Limit to avoid fetching too many; increase if needed

    let response = room.messages(options).await?;

    // Sender prefix for bridge bots (to exclude incoming messages)
    let sender_prefix = get_sender_prefix(service);

    // User's Matrix user ID
    let user_matrix_id = client.user_id().ok_or(anyhow!("User ID not available"))?;

    for event in response.chunk {
        if let Ok(AnySyncTimelineEvent::MessageLike(
            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(
                SyncRoomMessageEvent::Original(e),
            ),
        )) = event.raw().deserialize()
        {
            let sender_localpart = e.sender.localpart().to_string();
            // Check if sender is the user (matches user_matrix_id and not a bridge bot prefix)
            if e.sender == user_matrix_id && !sender_localpart.starts_with(&sender_prefix) {
                let timestamp = i64::from(e.origin_server_ts.0) / 1000;

                // Extract message type and body
                let (msgtype, body) = match e.content.msgtype {
                    MessageType::Text(t) => ("text", t.body),
                    MessageType::Notice(n) => ("notice", n.body),
                    MessageType::Image(i) => (
                        "image",
                        if i.body.is_empty() {
                            "📎 IMAGE".into()
                        } else {
                            i.body
                        },
                    ),
                    MessageType::Video(v) => (
                        "video",
                        if v.body.is_empty() {
                            "📎 VIDEO".into()
                        } else {
                            v.body
                        },
                    ),
                    MessageType::File(f) => (
                        "file",
                        if f.body.is_empty() {
                            "📎 FILE".into()
                        } else {
                            f.body
                        },
                    ),
                    MessageType::Audio(a) => (
                        "audio",
                        if a.body.is_empty() {
                            "📎 AUDIO".into()
                        } else {
                            a.body
                        },
                    ),
                    MessageType::Location(_l) => ("location", "📍 LOCATION".into()),
                    MessageType::Emote(t) => ("emote", t.body),
                    _ => continue,
                };

                // Skip error-like messages if needed (adapted from existing logic)
                if is_error_message(&body) {
                    continue;
                }

                return Ok(Some(BridgeMessage {
                    sender: "You".to_string(),
                    sender_display_name: "You".to_string(),
                    content: body,
                    timestamp,
                    formatted_timestamp: format_timestamp(timestamp, user_info.timezone.clone()),
                    message_type: msgtype.to_string(),
                    room_name: cleaned_room_name,
                    media_url: None,
                    room_id: Some(room_id.to_string()),
                }));
            }
        }
    }

    // If no user-sent message found within the limit
    tracing::info!(
        "No sent message found in the last 100 messages for room '{}'",
        room_id_str
    );
    Ok(None)
}

pub async fn fetch_bridge_room_messages(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    chat_name: &str,
    limit: Option<u64>,
) -> Result<(Vec<BridgeMessage>, String)> {
    tracing::info!(
        "Starting {} message fetch from Postgres - User: {}, chat: {}, limit: {}",
        capitalize(service),
        user_id,
        chat_name,
        limit.unwrap_or(20)
    );

    // Find the room_id for this chat by searching ont_channels or matching room names
    let room_id = find_room_id_for_chat(service, state, user_id, chat_name).await?;
    let user_info = state.user_core.get_user_info(user_id)?;
    let msg_limit = limit.unwrap_or(20) as i64;

    let messages = state
        .ontology_repository
        .get_messages_for_room(user_id, &room_id, msg_limit)
        .map_err(|e| anyhow!("Failed to query messages: {}", e))?;

    let room_name = if !messages.is_empty() {
        messages[0].sender_name.clone()
    } else {
        chat_name.to_string()
    };

    let bridge_messages: Vec<BridgeMessage> = messages
        .into_iter()
        .map(|m| ont_message_to_bridge_message(&m, user_info.timezone.clone()))
        .collect();

    Ok((bridge_messages, room_name))
}

/// Resolve a chat name to a room_id. Checks ontology channels and stored messages first,
/// then uses Matrix room list for name-to-room-id resolution (not for reading messages).
async fn find_room_id_for_chat(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    chat_name: &str,
) -> Result<String> {
    // Try ontology Person lookup
    if let Ok(Some(person)) = state
        .ontology_repository
        .find_person_by_name(user_id, chat_name)
    {
        if let Some(ch) = person
            .channels
            .iter()
            .find(|c| c.platform == service && c.room_id.is_some())
        {
            return Ok(ch.room_id.clone().unwrap());
        }
    }

    // Search ont_messages for a room matching sender_name
    if let Ok(candidates) = state
        .ontology_repository
        .get_recent_messages(user_id, service, 0)
    {
        let chat_lower = chat_name.to_lowercase();
        if let Some(m) = candidates
            .iter()
            .find(|m| m.sender_name.to_lowercase().contains(&chat_lower))
        {
            return Ok(m.room_id.clone());
        }
    }

    Err(anyhow!(
        "No {} messages found for '{}'. They need to send you a message first, or create them as a contact.",
        capitalize(service),
        chat_name
    ))
}

/// Convert an OntMessage to a BridgeMessage for display.
fn ont_message_to_bridge_message(
    m: &crate::models::ontology_models::OntMessage,
    timezone: Option<String>,
) -> BridgeMessage {
    BridgeMessage {
        sender: m.sender_name.clone(),
        sender_display_name: m.sender_name.clone(),
        content: m.content.clone(),
        timestamp: m.created_at as i64,
        formatted_timestamp: format_timestamp(m.created_at as i64, timezone),
        message_type: "text".to_string(),
        room_name: m.sender_name.clone(),
        media_url: None,
        room_id: Some(m.room_id.clone()),
    }
}

use matrix_sdk::notification_settings::RoomNotificationMode;

pub async fn fetch_bridge_messages(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    start_time: i64,
    _unread_only: bool,
) -> Result<Vec<BridgeMessage>> {
    tracing::info!(
        "Fetching {} messages from Postgres for user {}",
        service,
        user_id
    );

    let user_info = state.user_core.get_user_info(user_id)?;
    let since_ts = start_time as i32;

    let messages = state
        .ontology_repository
        .get_recent_messages(user_id, service, since_ts)
        .map_err(|e| anyhow!("Failed to query messages: {}", e))?;

    // Group by room_id, take up to 5 per room (mirroring old behavior)
    let mut by_room: std::collections::HashMap<
        String,
        Vec<&crate::models::ontology_models::OntMessage>,
    > = std::collections::HashMap::new();
    for m in &messages {
        by_room.entry(m.room_id.clone()).or_default().push(m);
    }

    let mut bridge_messages: Vec<BridgeMessage> = Vec::new();
    // Take top 5 rooms by most recent message
    let mut rooms: Vec<_> = by_room.into_iter().collect();
    rooms.sort_by(|a, b| {
        let a_ts = a.1.first().map(|m| m.created_at).unwrap_or(0);
        let b_ts = b.1.first().map(|m| m.created_at).unwrap_or(0);
        b_ts.cmp(&a_ts)
    });
    rooms.truncate(5);

    for (_room_id, room_msgs) in rooms {
        for m in room_msgs.into_iter().take(5) {
            bridge_messages.push(ont_message_to_bridge_message(m, user_info.timezone.clone()));
        }
    }

    // Sort by timestamp (most recent first)
    bridge_messages.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
    tracing::info!("Retrieved {} messages from Postgres", bridge_messages.len());
    Ok(bridge_messages)
}

use futures::future::join_all;
use matrix_sdk::room::MessagesOptions;

/// Resolve a WhatsApp chat_id (JID) to a joined Matrix Room, materializing
/// the portal via `!wa start-chat` when needed.
///
/// Only used by the WhatsApp send path. Group JIDs with no mxid yet return
/// an error (user must wait for bridge sync to finish); DM JIDs with no mxid
/// fall through to start-chat.
///
/// Post-conditions on Ok: the returned Room is joined and reachable via
/// `client.get_room`. On Err: caller should surface the message verbatim.
async fn resolve_whatsapp_room(
    state: &Arc<AppState>,
    user_id: i32,
    chat_id: &str,
    client: &MatrixClient,
) -> Result<Room> {
    tracing::info!(
        "SEND_FLOW_BRIDGE resolve_whatsapp_room: chat_id={}",
        chat_id
    );
    let repo = state
        .whatsapp_bridge_repository
        .as_ref()
        .ok_or_else(|| anyhow!("WhatsApp bridge repository not configured"))?;

    // Figure out the login phone for this user. We need it to scope portal
    // lookups (bridgev2 portal rows are keyed by receiver = login_id).
    let matrix_user_id = client
        .user_id()
        .ok_or_else(|| anyhow!("Matrix client has no user_id"))?
        .to_string();
    let repo_for_login = Arc::clone(repo);
    let matrix_user_id_clone = matrix_user_id.clone();
    let login_phone = tokio::task::spawn_blocking(move || {
        repo_for_login.get_login_phone_for_matrix_user(&matrix_user_id_clone)
    })
    .await
    .map_err(|e| anyhow!("login-phone lookup task panicked: {}", e))?
    .map_err(|e| anyhow!("login-phone lookup failed: {}", e))?
    .ok_or_else(|| anyhow!("user not logged into WhatsApp bridge"))?;

    // Portal table lookup.
    let repo_for_portal = Arc::clone(repo);
    let chat_id_owned = chat_id.to_string();
    let login_phone_for_portal = login_phone.clone();
    let portal_mxid = tokio::task::spawn_blocking(move || {
        repo_for_portal.get_portal_mxid(&chat_id_owned, &login_phone_for_portal)
    })
    .await
    .map_err(|e| anyhow!("portal lookup task panicked: {}", e))?
    .map_err(|e| anyhow!("portal lookup failed: {}", e))?;

    let mxid_opt = match portal_mxid {
        Some(mxid) => {
            tracing::info!(
                "SEND_FLOW_BRIDGE portal table hit: chat_id={} mxid={}",
                chat_id,
                mxid
            );
            Some(mxid)
        }
        None => {
            tracing::info!("SEND_FLOW_BRIDGE portal table miss for chat_id={}", chat_id);
            None
        }
    };

    let is_group = chat_id.ends_with("@g.us");
    let mxid = match mxid_opt {
        Some(mxid) => mxid,
        None if is_group => {
            return Err(anyhow!(
                "WhatsApp group '{}' not yet bridged - retry shortly",
                chat_id
            ));
        }
        None => {
            // DM cold path: !wa start-chat to materialize portal.
            let owned = start_chat_whatsapp(state, user_id, chat_id).await?;
            owned.to_string()
        }
    };

    let owned_mxid = matrix_sdk::ruma::OwnedRoomId::try_from(mxid.as_str())
        .map_err(|e| anyhow!("invalid mxid from resolve_whatsapp_room: {}", e))?;
    if client.get_room(&owned_mxid).is_none() {
        tracing::info!(
            "SEND_FLOW_BRIDGE resolve: room {} not in client cache yet, sync_once",
            mxid
        );
        let _ = client
            .sync_once(matrix_sdk::config::SyncSettings::new())
            .await;
    }
    let room = client.get_room(&owned_mxid).ok_or_else(|| {
        anyhow!(
            "resolved WhatsApp room {} still not reachable after sync",
            mxid
        )
    })?;
    tracing::info!(
        "SEND_FLOW_BRIDGE resolve_whatsapp_room ok: chat_id={} mxid={}",
        chat_id,
        mxid
    );
    Ok(room)
}

/// Resolve a Telegram chat_id (numeric tgid as string) to a joined Matrix
/// Room, materializing the portal via `!tg pm <id>` when the bridge DB has
/// no portal yet.
///
/// Mirrors `resolve_whatsapp_room` but uses TelegramBridgeRepository for
/// portal lookup and the mautrix-telegram bot's `pm` command for cold-DM
/// materialization. Only the DM case is handled — group/channel chat_ids
/// must already have a portal mxid (covered by `search_telegram_chat_candidate`
/// on the read side).
///
/// Post-conditions on Ok: returned Room is joined and reachable via
/// `client.get_room`. On Err: caller surfaces the message verbatim.
async fn resolve_telegram_room(
    state: &Arc<AppState>,
    user_id: i32,
    chat_id: &str,
    client: &MatrixClient,
) -> Result<Room> {
    tracing::info!(
        "SEND_FLOW_BRIDGE resolve_telegram_room: chat_id={}",
        chat_id
    );
    let repo = state
        .telegram_bridge_repository
        .as_ref()
        .ok_or_else(|| anyhow!("Telegram bridge repository not configured"))?;

    let contact_tgid: i64 = chat_id.parse().map_err(|_| {
        anyhow!(
            "resolve_telegram_room: chat_id '{}' is not a valid tgid",
            chat_id
        )
    })?;

    // Resolve user_tgid for portal-row lookup (portal is keyed by
    // tg_receiver = user_tgid).
    let matrix_user_id = client
        .user_id()
        .ok_or_else(|| anyhow!("Matrix client has no user_id"))?
        .to_string();
    let repo_for_user = Arc::clone(repo);
    let mxid_for_user = matrix_user_id.clone();
    let user_tgid =
        tokio::task::spawn_blocking(move || repo_for_user.get_user_tgid(&mxid_for_user))
            .await
            .map_err(|e| anyhow!("user_tgid lookup task panicked: {}", e))?
            .map_err(|e| anyhow!("user_tgid lookup failed: {}", e))?
            .ok_or_else(|| anyhow!("user not logged into Telegram bridge"))?;

    // Try existing portal first.
    let repo_for_portal = Arc::clone(repo);
    let portal_mxid = tokio::task::spawn_blocking(move || {
        repo_for_portal.get_dm_portal_mxid(user_tgid, contact_tgid)
    })
    .await
    .map_err(|e| anyhow!("portal lookup task panicked: {}", e))?
    .map_err(|e| anyhow!("portal lookup failed: {}", e))?;

    let mxid = match portal_mxid {
        Some(mxid) => {
            tracing::info!(
                "SEND_FLOW_BRIDGE TG portal hit: chat_id={} mxid={}",
                chat_id,
                mxid
            );
            mxid
        }
        None => {
            tracing::info!(
                "SEND_FLOW_BRIDGE TG portal miss for chat_id={}, sending !tg pm",
                chat_id
            );
            let owned = start_chat_telegram(state, user_id, contact_tgid).await?;
            owned.to_string()
        }
    };

    let owned_mxid = matrix_sdk::ruma::OwnedRoomId::try_from(mxid.as_str())
        .map_err(|e| anyhow!("invalid mxid from resolve_telegram_room: {}", e))?;
    if client.get_room(&owned_mxid).is_none() {
        tracing::info!(
            "SEND_FLOW_BRIDGE resolve_telegram: room {} not in client cache, sync_once",
            mxid
        );
        let _ = client
            .sync_once(matrix_sdk::config::SyncSettings::new())
            .await;
    }
    let room = client.get_room(&owned_mxid).ok_or_else(|| {
        anyhow!(
            "resolved Telegram room {} still not reachable after sync",
            mxid
        )
    })?;
    tracing::info!(
        "SEND_FLOW_BRIDGE resolve_telegram_room ok: chat_id={} mxid={}",
        chat_id,
        mxid
    );
    Ok(room)
}

/// Send `!tg pm <tgid>` to the Telegram bridge management room, then poll
/// the bridge DB for the newly materialized portal mxid.
///
/// mautrix-telegram v0.15.3 `pm` command (mautrix_telegram/commands/telegram/
/// misc.py): accepts username, phone, or numeric tgid and creates a
/// private-chat portal. The bot replies "Created private chat room with X"
/// on success. Rather than parse the bot reply (format may shift across
/// versions), we poll the portal table for `tgid=contact AND tg_receiver=
/// user AND peer_type='user'`. Polling is more robust because the portal
/// row is what we actually need anyway.
async fn start_chat_telegram(
    state: &Arc<AppState>,
    user_id: i32,
    contact_tgid: i64,
) -> Result<matrix_sdk::ruma::OwnedRoomId> {
    let repo = state
        .telegram_bridge_repository
        .as_ref()
        .ok_or_else(|| anyhow!("Telegram bridge repository not configured"))?;

    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;

    // Resolve user_tgid for portal-row poll target.
    let matrix_user_id = client
        .user_id()
        .ok_or_else(|| anyhow!("Matrix client has no user_id"))?
        .to_string();
    let repo_for_user = Arc::clone(repo);
    let mxid_for_user = matrix_user_id.clone();
    let user_tgid =
        tokio::task::spawn_blocking(move || repo_for_user.get_user_tgid(&mxid_for_user))
            .await
            .map_err(|e| anyhow!("user_tgid lookup task panicked: {}", e))?
            .map_err(|e| anyhow!("user_tgid lookup failed: {}", e))?
            .ok_or_else(|| anyhow!("user not logged into Telegram bridge"))?;

    // Resolve management room for this user + bridge.
    let bridge = state
        .user_repository
        .get_bridge(user_id, "telegram")?
        .ok_or_else(|| anyhow!("no Telegram bridge record for user {}", user_id))?;
    let mgmt_room_id_str = bridge.room_id.ok_or_else(|| {
        anyhow!(
            "Telegram bridge has no management room_id for user {}",
            user_id
        )
    })?;
    let mgmt_room_id =
        matrix_sdk::ruma::OwnedRoomId::try_from(mgmt_room_id_str.as_str()).map_err(|e| {
            anyhow!(
                "invalid TG management room id '{}': {}",
                mgmt_room_id_str,
                e
            )
        })?;
    let room = client.get_room(&mgmt_room_id).ok_or_else(|| {
        anyhow!(
            "TG management room {} not found in client",
            mgmt_room_id_str
        )
    })?;

    // Resolve the bridge bot user id for reply capture.
    let bridge_bot =
        std::env::var("TELEGRAM_BRIDGE_BOT").map_err(|_| anyhow!("TELEGRAM_BRIDGE_BOT not set"))?;
    let bot_user_id = matrix_sdk::ruma::OwnedUserId::try_from(bridge_bot.as_str())
        .map_err(|e| anyhow!("invalid TELEGRAM_BRIDGE_BOT user id: {}", e))?;

    // Resolve the most reliable handle for `!tg pm`. Telethon's
    // `get_entity(<numeric_id>)` requires a cached access_hash which cold
    // contacts (saved but never messaged) don't have, producing the bot
    // reply "Invalid user identifier or user not found." Using @username
    // (or phone with `+`) resolves through Telegram's directory or the
    // local contacts list and works for cold DMs too.
    let repo_for_handle = Arc::clone(repo);
    let (pm_handle, handle_kind) =
        tokio::task::spawn_blocking(move || repo_for_handle.get_pm_handle(contact_tgid))
            .await
            .map_err(|e| anyhow!("get_pm_handle task panicked: {}", e))?
            .map_err(|e| anyhow!("get_pm_handle failed: {}", e))?;

    // Send the `!tg pm` command via probe_bridge_room so we capture the bot's
    // reply text. On failure (wrong syntax, user not found, bot offline) the
    // reply contains the verbatim error which is what we need to iterate.
    let cmd = format!("!tg pm {}", pm_handle);
    tracing::info!(
        "SEND_FLOW_BRIDGE start_chat_telegram user={} sending {:?} (handle_kind={}, tgid={})",
        user_id,
        cmd,
        handle_kind,
        contact_tgid
    );
    let bot_replies =
        match probe_bridge_room(&client, &room, &bot_user_id, &cmd, Duration::from_secs(8)).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                "SEND_FLOW_BRIDGE start_chat_telegram probe failed (will still poll portal): {}",
                e
            );
                Vec::new()
            }
        };
    if bot_replies.is_empty() {
        tracing::warn!(
            "SEND_FLOW_BRIDGE start_chat_telegram: bot did not reply within 8s for tgid={}",
            contact_tgid
        );
    } else {
        for (i, body) in bot_replies.iter().enumerate() {
            tracing::info!(
                "SEND_FLOW_BRIDGE start_chat_telegram bot reply [{}]: {:?}",
                i,
                body.chars().take(400).collect::<String>()
            );
        }
    }

    // Poll the bridge DB for the portal row to appear (up to ~12s additional).
    // Bot may have replied "Created private chat room with X" but the portal
    // row is what we actually need to send the message.
    for attempt in 1..=24 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let repo_for_poll = Arc::clone(repo);
        let mxid_opt = tokio::task::spawn_blocking(move || {
            repo_for_poll.get_dm_portal_mxid(user_tgid, contact_tgid)
        })
        .await
        .map_err(|e| anyhow!("portal poll task panicked: {}", e))?
        .map_err(|e| anyhow!("portal poll failed: {}", e))?;

        if let Some(mxid) = mxid_opt {
            tracing::info!(
                "SEND_FLOW_BRIDGE start_chat_telegram ok after {}ms: tgid={} mxid={}",
                attempt * 500,
                contact_tgid,
                mxid
            );
            return matrix_sdk::ruma::OwnedRoomId::try_from(mxid.as_str())
                .map_err(|e| anyhow!("invalid mxid from TG portal: {}", e));
        }
    }

    // Portal never materialized. Surface the bot's reply text in the error
    // so the caller (and us looking at logs) sees exactly what went wrong.
    let reply_summary = if bot_replies.is_empty() {
        "(no bot reply)".to_string()
    } else {
        bot_replies
            .iter()
            .map(|b| b.chars().take(200).collect::<String>())
            .collect::<Vec<_>>()
            .join(" | ")
    };
    Err(anyhow!(
        "Telegram portal for tgid={} not materialized within 12s after `!tg pm`. Bot reply: {}",
        contact_tgid,
        reply_summary
    ))
}

#[allow(clippy::too_many_arguments)]
pub async fn send_bridge_message(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    chat_name: &str,
    message: &str,
    media_url: Option<String>,
    target_room_id: Option<&str>,
    target_chat_id: Option<&str>,
) -> Result<BridgeMessage> {
    tracing::info!(
        "SEND_FLOW_BRIDGE send_bridge_message ENTER: service={}, user={}, chat_name='{}', room_id={:?}, chat_id={:?}, has_media={}",
        service, user_id, chat_name, target_room_id, target_chat_id, media_url.is_some()
    );

    tracing::info!(
        "SEND_FLOW_BRIDGE Getting cached Matrix client for user={}",
        user_id
    );
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
    tracing::info!("SEND_FLOW_BRIDGE Got Matrix client OK");

    let bridge = state.user_repository.get_bridge(user_id, service)?;
    tracing::info!(
        "SEND_FLOW_BRIDGE Bridge status: found={}, status={:?}",
        bridge.is_some(),
        bridge.as_ref().map(|b| b.status.clone())
    );
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        tracing::error!("SEND_FLOW_BRIDGE Bridge not connected, aborting");
        return Err(anyhow!(
            "{} bridge is not connected. Please log in first.",
            capitalize(service)
        ));
    }

    // Resolution order:
    //   1. target_room_id hits cache -> use it (warm path, most common).
    //   2. target_chat_id (WhatsApp only): query bridge portal table for mxid.
    //      - mxid found -> use it.
    //      - mxid None + DM JID -> !wa start-chat to materialize portal.
    //      - mxid None + group  -> error (groups are materialized on login
    //        sync; absence means sync hasn't caught up yet).
    //   3. Fall back to name-based fuzzy search against joined rooms (legacy
    //      path, still used by non-WA bridges and by web callers that don't
    //      have a chat_id).
    let room = if let Some(rid) = target_room_id {
        tracing::info!("SEND_FLOW_BRIDGE Using target_room_id directly: {}", rid);
        let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(rid).map_err(|e| {
            tracing::error!("SEND_FLOW_BRIDGE Invalid room ID '{}': {}", rid, e);
            anyhow!("Invalid room ID '{}': {}", rid, e)
        })?;
        if client.get_room(&room_id).is_none() {
            tracing::info!(
                "SEND_FLOW_BRIDGE Room not in cache, running sync_once to populate rooms..."
            );
            if let Err(e) = client
                .sync_once(matrix_sdk::config::SyncSettings::new())
                .await
            {
                tracing::error!("SEND_FLOW_BRIDGE sync_once failed: {}", e);
            } else {
                tracing::info!("SEND_FLOW_BRIDGE sync_once completed");
            }
        }
        match client.get_room(&room_id) {
            Some(r) => {
                tracing::info!(
                    "SEND_FLOW_BRIDGE client.get_room({}) returned: found=true",
                    rid
                );
                r
            }
            None => {
                // Room not reachable. If we have a chat_id, fall through to
                // the bridge-DB resolution path below; otherwise error out.
                tracing::warn!(
                    "SEND_FLOW_BRIDGE Room {} not in client after sync (stale mxid?); \
                     falling back to chat_id path (have_chat_id={})",
                    rid,
                    target_chat_id.is_some()
                );
                match (target_chat_id, service) {
                    (Some(chat_id), "whatsapp") => {
                        resolve_whatsapp_room(state, user_id, chat_id, &client).await?
                    }
                    (Some(chat_id), "telegram") => {
                        resolve_telegram_room(state, user_id, chat_id, &client).await?
                    }
                    _ => {
                        return Err(anyhow!(
                            "Room '{}' not in client and no chat_id fallback available for service '{}'",
                            rid,
                            service
                        ));
                    }
                }
            }
        }
    } else if let (Some(chat_id), "whatsapp") = (target_chat_id, service) {
        // Cold path: resolve via portal table (+start-chat for DMs).
        resolve_whatsapp_room(state, user_id, chat_id, &client).await?
    } else if let (Some(chat_id), "telegram") = (target_chat_id, service) {
        // Cold path: resolve via bridge DB portal table (+ !tg pm to materialize).
        resolve_telegram_room(state, user_id, chat_id, &client).await?
    } else {
        tracing::info!(
            "SEND_FLOW_BRIDGE No target_room_id, searching by name '{}'",
            chat_name
        );
        let service_rooms = get_service_rooms(&client, service).await?;
        tracing::info!("SEND_FLOW_BRIDGE Got {} service rooms", service_rooms.len());
        let exact_room = find_exact_room(&service_rooms, chat_name);
        tracing::info!(
            "SEND_FLOW_BRIDGE find_exact_room result: found={}",
            exact_room.is_some()
        );
        match exact_room {
            Some(room_info) => {
                tracing::info!(
                    "SEND_FLOW_BRIDGE Exact room: id={}, name={}",
                    room_info.room_id,
                    room_info.display_name
                );
                let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_info.room_id.as_str())
                    .map_err(|e| anyhow!("Invalid room ID: {}", e))?;
                client
                    .get_room(&room_id)
                    .ok_or_else(|| anyhow!("Room not found"))?
            }
            None => {
                let suggestions = get_best_matches(&service_rooms, chat_name);
                let error_msg = if suggestions.is_empty() {
                    format!(
                        "Could not find exact matching {} room for '{}'",
                        capitalize(service),
                        chat_name
                    )
                } else {
                    format!(
                        "Could not find exact matching {} room for '{}'. Did you mean one of these?\n{}",
                        capitalize(service),
                        chat_name,
                        suggestions.join("\n")
                    )
                };
                tracing::error!("SEND_FLOW_BRIDGE No room found: {}", error_msg);
                return Err(anyhow!(error_msg));
            }
        }
    };
    tracing::info!(
        "SEND_FLOW_BRIDGE Got Matrix room object: room_id={}, display_name will be fetched after send",
        room.room_id()
    );
    use matrix_sdk::ruma::events::room::message::{
        ImageMessageEventContent, MessageType, RoomMessageEventContent,
    };
    if let Some(url) = media_url {
        tracing::info!(
            "SEND_FLOW_BRIDGE Sending IMAGE message with caption, downloading from URL..."
        );
        // ── 1. Download the image and get MIME type ────────────────────────────────
        let resp = reqwest::get(&url).await?;
        // Get MIME type from headers before consuming the response
        let mime: mime_guess::mime::Mime = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| mime_guess::MimeGuess::from_path(&url).first_or_octet_stream());
        // Now consume the response to get the bytes
        let bytes = resp.bytes().await?;
        let size = bytes.len();
        tracing::info!(
            "SEND_FLOW_BRIDGE Downloaded image: {} bytes, mime={}",
            size,
            mime
        );
        // ── 2. Upload to the homeserver ──────────────────────────────────────────
        let upload_resp = client.media().upload(&mime, bytes.to_vec(), None).await?;
        let mxc: matrix_sdk::ruma::OwnedMxcUri = upload_resp.content_uri;
        tracing::info!("SEND_FLOW_BRIDGE Uploaded to homeserver: mxc={}", mxc);
        // ── 4. Build the image-message content with caption in *one* event ──────
        let mut img = ImageMessageEventContent::plain(
            message.to_owned(), // the caption / body
            mxc,
        );
        // Optional but nice: add basic metadata so bridges & clients know the size
        let mut imageinfo = matrix_sdk::ruma::events::room::ImageInfo::new();
        imageinfo.size = Some(matrix_sdk::ruma::UInt::new(size as u64).unwrap_or_default());
        img.info = Some(Box::new(imageinfo));
        // Wrap it as a generic "m.room.message"
        let content = RoomMessageEventContent::new(MessageType::Image(img));
        // ── 5. Send it ───────────────────────────────────────────────────────────
        let rid_str = room.room_id().to_string();
        tracing::info!(
            "SEND_FLOW_BRIDGE Calling room.send for image message, room_id={}",
            rid_str
        );
        room.send(content).await?;
        tracing::info!(
            "SEND_FLOW_BRIDGE room.send for image returned OK, room_id={}",
            rid_str
        );
    } else {
        // plain text
        let rid_str = room.room_id().to_string();
        tracing::info!(
            "SEND_FLOW_BRIDGE Sending plain text message to room_id={}",
            rid_str
        );
        room.send(RoomMessageEventContent::text_plain(message))
            .await?;
        tracing::info!(
            "SEND_FLOW_BRIDGE room.send for text returned OK, room_id={}",
            rid_str
        );
    }
    let rid_str = room.room_id().to_string();
    tracing::info!(
        "SEND_FLOW_BRIDGE Message sent via {}, building response, room_id={}",
        service,
        rid_str
    );
    let user_info = state.user_core.get_user_info(user_id)?;
    let current_timestamp = chrono::Utc::now().timestamp();
    let display_name = room.display_name().await?.to_string();
    tracing::info!(
        "SEND_FLOW_BRIDGE send_bridge_message COMPLETE: room_id={}, display_name={}, service={}",
        rid_str,
        display_name,
        service
    );

    // Post-send: if caller supplied a chat_id (bridge-DB handle), upsert the
    // ontology Person so future sends can skip bridge-DB lookup. Best-effort;
    // a failure here never fails the send.
    if let Some(chat_id) = target_chat_id {
        let cleaned_name = remove_bridge_suffix(&display_name);
        match state.ontology_repository.upsert_person(
            user_id,
            &cleaned_name,
            service,
            Some(chat_id),
            Some(&rid_str),
        ) {
            Ok(p) => tracing::info!(
                "SEND_FLOW_BRIDGE post-send upsert_person ok: person_id={} name='{}' chat_id={} room_id={}",
                p.id, p.name, chat_id, rid_str
            ),
            Err(e) => tracing::warn!(
                "SEND_FLOW_BRIDGE post-send upsert_person failed (non-fatal) for chat_id={}: {}",
                chat_id,
                e
            ),
        }
    }

    // Return the sent message details
    Ok(BridgeMessage {
        sender: "You".to_string(),
        sender_display_name: "You".to_string(),
        content: message.to_string(),
        timestamp: current_timestamp,
        formatted_timestamp: format_timestamp(current_timestamp, user_info.timezone),
        message_type: "text".to_string(),
        room_name: display_name,
        media_url: None,
        room_id: Some(room.room_id().to_string()),
    })
}

use matrix_sdk::ruma::events::room::message::{OriginalSyncRoomMessageEvent, Relation};
use matrix_sdk::RoomMemberships;
use strsim;

/// Handle an incoming read receipt from a bridge.
/// When the user reads a message on the native platform (WhatsApp/Signal/Telegram),
/// the bridge forwards the read receipt as a Matrix m.receipt event.
/// We use this to mark the corresponding ont_messages as seen.
pub async fn handle_read_receipt(
    ev: matrix_sdk::ruma::events::SyncEphemeralRoomEvent<
        matrix_sdk::ruma::events::receipt::ReceiptEventContent,
    >,
    room: Room,
    client: MatrixClient,
    state: Arc<AppState>,
    user_id: i32,
) {
    use matrix_sdk::ruma::{api::client::room::get_room_event, events::receipt::ReceiptType};

    let own_user_id = match client.user_id() {
        Some(id) => id.to_owned(),
        None => return,
    };

    // Check if this receipt contains a read receipt from our user
    let (event_id, _receipt) = match ev.content.user_receipt(&own_user_id, ReceiptType::Read) {
        Some(r) => r,
        None => return,
    };

    let room_id_str = room.room_id().to_string();

    // Fetch the event that was read to get its origin_server_ts
    let request = get_room_event::v3::Request::new(room.room_id().to_owned(), event_id.to_owned());
    let event_ts = match client.send(request).await {
        Ok(response) => match response.event.deserialize_as::<AnySyncTimelineEvent>() {
            Ok(any_event) => i64::from(any_event.origin_server_ts().as_secs()) as i32,
            Err(_) => return,
        },
        Err(_) => return,
    };

    let now = chrono::Utc::now().timestamp() as i32;
    match state
        .ontology_repository
        .mark_messages_seen_in_room(user_id, &room_id_str, event_ts, now)
    {
        Ok(count) if count > 0 => {
            tracing::debug!(
                "Marked {} messages as seen in room {} for user {} (read receipt up to ts={})",
                count,
                room_id_str,
                user_id,
                event_ts
            );
        }
        _ => {}
    }
}

/// Handle a Matrix redaction event from a bridge.
///
/// When a user deletes a message on the source platform (e.g. WhatsApp's
/// "delete for everyone"), the mautrix bridge propagates it as
/// `m.room.redaction`. We mirror that delete in `ont_messages` so the row no
/// longer surfaces in the dashboard, digests, or signal-history context.
///
/// The redacted event_id lives at one of two places depending on the room
/// version: top-level `redacts` (pre-v11) or `content.redacts` (v11+). We
/// check both rather than threading the room version through, since both are
/// `Option<OwnedEventId>` and at most one is populated per event.
pub async fn handle_bridge_redaction(
    ev: matrix_sdk::ruma::events::room::redaction::OriginalSyncRoomRedactionEvent,
    room: Room,
    client: MatrixClient,
    state: Arc<AppState>,
) {
    let redacted_event_id = match ev.redacts.as_ref().or(ev.content.redacts.as_ref()) {
        Some(eid) => eid.to_string(),
        None => {
            tracing::debug!(
                "Redaction event {} has no redacts target in room {}",
                ev.event_id,
                room.room_id()
            );
            return;
        }
    };

    let matrix_user_id = match client.user_id() {
        Some(id) => id.to_owned(),
        None => return,
    };
    let local_user_id = matrix_user_id.localpart();
    let user_id = match state
        .user_repository
        .get_user_by_matrix_user_id(local_user_id)
    {
        Ok(Some(user)) => user.id,
        _ => return,
    };

    if crate::utils::tuwunel_event_cleanup::is_tuwunel_admin_redaction_reason(
        ev.content.reason.as_deref(),
    ) {
        tracing::info!(
            user_id,
            room_id = %room.room_id(),
            redacted_event_id,
            "Ignoring Tuwunel admin cleanup redaction; ontology row remains canonical"
        );
        return;
    }

    match state
        .ontology_repository
        .delete_message_by_matrix_event_id(user_id, &redacted_event_id)
    {
        Ok(0) => {
            tracing::debug!(
                "Redaction in room {} targets event {}: no matching ont_messages row",
                room.room_id(),
                redacted_event_id
            );
        }
        Ok(n) => {
            tracing::info!(
                "Deleted {} ont_messages row(s) for redacted event {} (user {}, room {})",
                n,
                redacted_event_id,
                user_id,
                room.room_id()
            );
        }
        Err(e) => {
            tracing::warn!(
                "Failed to delete message for redacted event {}: {}",
                redacted_event_id,
                e
            );
        }
    }
}

pub async fn handle_bridge_message(
    event: OriginalSyncRoomMessageEvent,
    room: Room,
    client: MatrixClient,
    state: Arc<AppState>,
) {
    tracing::info!(
        "🔔 Bridge message handler - room: {}, sender: {}",
        room.room_id(),
        event.sender
    );

    // Skip m.replace edits. mautrix-whatsapp (and other mautrix bridges) emit
    // an OriginalSyncRoomMessageEvent with `relates_to = Replacement` when the
    // source-platform user edits a message. Treating it as a new message
    // produces a duplicate ont_messages row: same sender, same content (or
    // close enough), minutes/hours apart. We don't propagate edits into
    // summaries today; just drop the event.
    if let Some(Relation::Replacement(replaces)) = event.content.relates_to.as_ref() {
        tracing::info!(
            "Skipping m.replace edit in room {} (replaces event {})",
            room.room_id(),
            replaces.event_id
        );
        return;
    }

    if room.user_defined_notification_mode().await == Some(RoomNotificationMode::Mute) {
        tracing::info!("Skipping message from a muted room");
        return;
    }

    // Check message age using pure function
    const HALF_HOUR_MS: u64 = 30 * 60 * 1000;
    let message_ts: u64 = event.origin_server_ts.0.into();
    if !should_process_message(message_ts, HALF_HOUR_MS) {
        tracing::info!("Skipping old message (event ID: {})", event.event_id);
        return;
    }

    // Find the user ID for this Matrix client
    let matrix_user_id = match client.user_id() {
        Some(id) => id.to_owned(),
        None => {
            tracing::error!("Matrix client has no user ID, skipping message");
            return;
        }
    };
    let client_user_id = matrix_user_id.to_string();
    // Extract the local part of the Matrix user ID (before the domain)
    let local_user_id = client_user_id
        .split(':')
        .next()
        .map(|s| s.trim_start_matches('@')) // Remove leading '@'
        .unwrap_or(&client_user_id); // Fallback to original if parsing fails
    let user = match state
        .user_repository
        .get_user_by_matrix_user_id(local_user_id)
    {
        Ok(Some(user)) => user,
        _ => return,
    };
    let user_id = user.id;

    // Track which bridges are currently connecting (used below to skip
    // management-room processing for those bridges only).
    let mut connecting_bridges: Vec<String> = Vec::new();
    for bridge_type in &["signal", "telegram", "whatsapp"] {
        if let Ok(Some(bridge)) = state.user_repository.get_bridge(user_id, bridge_type) {
            if bridge.status == "connecting" {
                connecting_bridges.push(bridge_type.to_string());
            }
        }
    }

    // Check if this is a bridge management room
    let room_id_str = room.room_id().to_string();
    let bridge_types = vec!["signal", "telegram", "whatsapp"];
    let mut bridges = Vec::new();
    for bridge_type in &bridge_types {
        if let Ok(Some(bridge)) = state.user_repository.get_bridge(user_id, bridge_type) {
            bridges.push(bridge);
        }
    }
    tracing::info!(
        "🔍 Checking if room {} is a management room (found {} bridges)",
        room_id_str,
        bridges.len()
    );
    if let Some(bridge) = bridges
        .iter()
        .find(|b| b.room_id.as_ref() == Some(&room_id_str))
    {
        // This is a management room for a bridge
        tracing::info!(
            "📋 Processing message in {} bridge management room (status: {})",
            bridge.bridge_type,
            bridge.status
        );
        // Skip if bridge is in connecting state (handled by monitor task)
        if bridge.status == "connecting" {
            tracing::info!(
                "⏳ Skipping disconnection check during initial connection for {}",
                bridge.bridge_type
            );
            return;
        }

        // Get the bridge bot ID for this service
        let bridge_bot_var = match bridge.bridge_type.as_str() {
            "signal" => "SIGNAL_BRIDGE_BOT",
            "whatsapp" => "WHATSAPP_BRIDGE_BOT",
            "telegram" => "TELEGRAM_BRIDGE_BOT",
            _ => return, // Unknown bridge type
        };
        let bridge_bot = match std::env::var(bridge_bot_var) {
            Ok(bot) => bot,
            Err(_) => {
                tracing::error!("{} not set", bridge_bot_var);
                return;
            }
        };
        let bot_user_id = match matrix_sdk::ruma::OwnedUserId::try_from(bridge_bot) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("Invalid bridge bot ID: {}", e);
                return;
            }
        };

        // Check if sender is the bridge bot
        if event.sender != bot_user_id {
            tracing::info!(
                "📤 Message not from bridge bot (sender: {}, expected: {}), skipping",
                event.sender,
                bot_user_id
            );
            return;
        }
        tracing::info!("✅ Message IS from bridge bot");

        // Extract message content
        let content = match event.content.msgtype {
            MessageType::Text(t) => t.body,
            MessageType::Notice(n) => n.body,
            _ => {
                tracing::debug!("Non-text/notice message in management room, skipping");
                return;
            }
        };
        // Log bridge bot management room messages for debugging
        tracing::info!(
            "Bridge bot ({}) management room message: {:?}",
            bridge.bridge_type,
            content
        );

        // Skip health check related messages - these are handled by the health check endpoint
        if is_health_check_message(&content) {
            tracing::info!("Skipping health check / status message in management room");
            return;
        }

        // Check for disconnection patterns (external session termination).
        //
        // DESIGN NOTE: We intentionally do NOT delete the bridge record here,
        // even when we detect a disconnection event. Reasons:
        //
        //  1. The detection patterns are heuristic substring matches against
        //     bridge bot text. False positives are costly (silently bricks
        //     the user's bridge; we chased this bug all day). Only the user's
        //     explicit "Disconnect" action should delete the DB record.
        //
        //  2. If the session really did terminate externally, the next
        //     health-check call (from the UI's "Check Connection" button, or
        //     from periodic polling) will see `!<prefix> ping` / list-logins
        //     return an unhealthy response and can surface that state to the
        //     user without destroying the record.
        //
        //  3. Deleting on push events also deletes the room_id/config we
        //     need for the cleanup commands themselves, making orderly
        //     cleanup harder.
        //
        // So: we LOG the detection and record it for telemetry, but we do not
        // auto-delete or evict the Matrix client here.
        if is_disconnection_message(&content) {
            tracing::warn!(
                "🚨 Detected disconnection signal in {} bridge for user {} (NOT auto-deleting; content={:?})",
                bridge.bridge_type,
                user_id,
                content
            );

            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            if let Err(e) = state.user_repository.record_bridge_disconnection(
                user_id,
                &bridge.bridge_type,
                current_time,
            ) {
                tracing::error!(
                    "Failed to record disconnection event for user {}: {}",
                    user_id,
                    e
                );
            }
        } else {
            tracing::debug!("No disconnection detected in management room message");
        }

        // Return early since this is not a portal message
        return;
    }

    // Proceed with existing portal message handling if not a management room
    // Get room name
    let room_name = match room.display_name().await {
        Ok(name) => name.to_string(),
        Err(e) => {
            tracing::error!("Failed to get room name: {}", e);
            return;
        }
    };
    let sender_localpart = event.sender.localpart().to_string();
    let service = match infer_service(&room_name, &sender_localpart) {
        Some(s) => s,
        None => {
            // Not a ghost sender - try room members (user's own message in bridge room)
            match infer_service_from_room_members(&room).await {
                Some(s) => s,
                None => {
                    tracing::debug!("Could not infer service, skipping");
                    return;
                }
            }
        }
    };

    // Service is now known. Bump the handler-invocation counter so the
    // /api/admin/handler-stats diagnostic endpoint can show that portal
    // events ARE reaching the handler post-deploy. This counter ticks
    // BEFORE the connecting/sub/plan filters below so we count raw event
    // arrivals, not just stored messages.
    bump_invocation(&service);

    // Skip portal messages for a service that is currently connecting
    // (the connection monitor handles those). Only skip the connecting service,
    // not all services.
    if connecting_bridges.contains(&service) {
        tracing::info!(
            "⏳ Skipping {} portal message - bridge is connecting",
            service
        );
        return;
    }

    // Skip messages from the bridge bot itself (e.g. "Hello, I'm a Telegram bridge bot...")
    // Derive localparts from env vars (format: @localpart:server) to match deployment config
    let bot_localparts: Vec<String> = [
        "TELEGRAM_BRIDGE_BOT",
        "WHATSAPP_BRIDGE_BOT",
        "SIGNAL_BRIDGE_BOT",
    ]
    .iter()
    .filter_map(|var| std::env::var(var).ok())
    .filter_map(|full_id| {
        // Extract localpart from @localpart:server
        full_id
            .strip_prefix('@')
            .and_then(|s| s.split(':').next())
            .map(|s| s.to_string())
    })
    .collect();
    if bot_localparts.iter().any(|b| b == &sender_localpart) {
        tracing::debug!(
            "Skipping bridge bot message from {} in portal room",
            sender_localpart
        );
        return;
    }

    // Check if sender is a bridge ghost or the user themselves
    let sender_prefix = get_sender_prefix(&service);
    if !sender_localpart.starts_with(&sender_prefix) {
        // User's own outgoing message - store in ontology for context, skip AI processing
        let content = match &event.content.msgtype {
            MessageType::Text(t) => t.body.clone(),
            MessageType::Notice(n) => n.body.clone(),
            MessageType::Emote(t) => t.body.clone(),
            _ => return, // Skip user media (no useful text for LLM)
        };
        if is_error_message(&content) {
            log_bridge_error_notice(
                "outgoing",
                user_id,
                &service,
                room.room_id().as_str(),
                &content,
            );
            return;
        }
        let current_room_id = room.room_id().to_string();
        let is_group = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
            Ok(members) => {
                let localparts: Vec<String> = members
                    .iter()
                    .map(|m| m.user_id().localpart().to_string())
                    .collect();
                is_group_room(&service, &localparts)
            }
            Err(_) => false,
        };
        let msg = crate::models::ontology_models::NewOntMessage {
            user_id,
            room_id: current_room_id.clone(),
            platform: service.clone(),
            sender_name: "You".to_string(),
            sender_key: None,
            content: content.clone(),
            person_id: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i32,
            matrix_event_id: Some(event.event_id.to_string()),
        };
        let state_clone = state.clone();
        let stored_service = service.clone();
        tokio::spawn(async move {
            match state_clone.ontology_repository.insert_message(&msg) {
                Ok((created, is_new)) => {
                    if !is_new {
                        bump_skipped_duplicate(&stored_service);
                        return;
                    }
                    bump_stored(&stored_service);
                    let snapshot = serde_json::json!({
                        "message_id": created.id,
                        "platform": msg.platform,
                        "sender": "You",
                        "sender_name": "You",
                        "content": msg.content,
                        "room_id": msg.room_id,
                        "is_group": is_group,
                        "is_outgoing": true,
                    });
                    crate::proactive::rules::emit_ontology_change(
                        &state_clone,
                        user_id,
                        "Message",
                        created.id as i32,
                        "created",
                        snapshot,
                    )
                    .await;

                    // Auto-resolve: user replied, so clear pending digests and resolve urgency
                    let now = msg.created_at;
                    // User replied - they've seen all prior messages in this room
                    if let Err(e) = state_clone.ontology_repository.mark_messages_seen_in_room(
                        user_id,
                        &current_room_id,
                        now,
                        now,
                    ) {
                        tracing::warn!("Failed to mark room messages seen: {}", e);
                    }
                    if let Err(e) = state_clone.ontology_repository.mark_room_digest_delivered(
                        user_id,
                        &current_room_id,
                        now,
                    ) {
                        tracing::warn!("Failed to mark room digest delivered: {}", e);
                    }

                    // Check if outgoing message completes any tracked events
                    crate::proactive::system_behaviors::check_outgoing_event_resolution(
                        &state_clone,
                        user_id,
                        &current_room_id,
                        &msg.content,
                        created.id,
                    )
                    .await;
                }
                Err(e) => tracing::warn!("Failed to store user message: {}", e),
            }
        });
        return;
    }

    // Pending reply watch: if the user armed one for this room, SMS them
    // the first inbound message from the recipient and clear the watch.
    // Runs before the tier-2/auto-features gates because the user has
    // already paid for SMS and explicitly asked to be told.
    {
        let watch_content = match &event.content.msgtype {
            MessageType::Text(t) => t.body.clone(),
            MessageType::Notice(n) => n.body.clone(),
            MessageType::Emote(t) => t.body.clone(),
            MessageType::Image(_) => "[image]".to_string(),
            MessageType::Video(_) => "[video]".to_string(),
            MessageType::File(_) => "[file]".to_string(),
            MessageType::Audio(_) => "[audio]".to_string(),
            MessageType::Location(_) => "[location]".to_string(),
            _ => String::new(),
        };
        let room_id_for_watch = room.room_id().to_string();
        match state
            .pending_reply_watches_repository
            .find_active_bridge(user_id, &room_id_for_watch)
        {
            Ok(Some(watch)) => {
                let body = if watch_content.is_empty() {
                    format!("Reply from {}: (no text)", watch.contact_display_name)
                } else {
                    format!(
                        "Reply from {}: {}",
                        watch.contact_display_name, watch_content
                    )
                };
                // Only clear the watch once we've successfully notified the
                // user. If the SMS fails (e.g. transient Twilio outage), leave
                // the watch armed so the next inbound message retries.
                match state.channel_router.send_to_user(&user, &body, None).await {
                    Ok(_) => {
                        if let Err(e) = state.pending_reply_watches_repository.delete(watch.id) {
                            tracing::warn!(
                                "REPLY_WATCH failed to delete fired watch id={}: {}",
                                watch.id,
                                e
                            );
                        } else {
                            tracing::info!(
                                "REPLY_WATCH fired+cleared bridge watch id={} user={} room={}",
                                watch.id,
                                user_id,
                                room_id_for_watch
                            );
                        }
                    }
                    Err(e) => tracing::warn!(
                        "REPLY_WATCH SMS failed user={} watch={}, leaving armed for retry: {}",
                        user_id,
                        watch.id,
                        e
                    ),
                }
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(
                "REPLY_WATCH lookup failed user={} room={}: {}",
                user_id,
                room_id_for_watch,
                e
            ),
        }
    }

    // Check subscription. Tier 2 users should still get incoming bridge
    // messages written to the readable message history even if their plan
    // does not include proactive/background automation.
    let has_valid_sub = state
        .user_repository
        .has_valid_subscription_tier(user_id, "tier 2")
        .unwrap_or(false);
    if !has_valid_sub {
        tracing::debug!(
            "User {} does not have valid subscription, skipping",
            user_id
        );
        return;
    }
    let user_plan = state.user_repository.get_plan_type(user_id).unwrap_or(None);
    let has_auto_features = crate::utils::plan_features::has_auto_features(user_plan.as_deref());
    if !has_auto_features {
        tracing::debug!(
            "User {} on {:?} plan - storing bridge message without auto features",
            user_id,
            user_plan
        );
    }

    let cleanup_tuwunel_media = should_cleanup_tuwunel_media(&event.content.msgtype);
    let cleanup_matrix_event_id = event.event_id.to_string();

    // Extract message content and estimate message size for bandwidth tracking
    let (content, bytes_estimate) = match event.content.msgtype {
        MessageType::Text(ref t) => (t.body.clone(), t.body.len() as i32),
        MessageType::Notice(ref n) => (n.body.clone(), n.body.len() as i32),
        MessageType::Image(ref img) => {
            let size = img
                .info
                .as_ref()
                .and_then(|i| i.size)
                .map(|s| u64::from(s).min(i32::MAX as u64) as i32)
                .unwrap_or(50_000i32);
            ("IMAGE".into(), size)
        }
        MessageType::Video(ref vid) => {
            let size = vid
                .info
                .as_ref()
                .and_then(|i| i.size)
                .map(|s| u64::from(s).min(i32::MAX as u64) as i32)
                .unwrap_or(500_000i32);
            ("VIDEO".into(), size)
        }
        MessageType::File(ref f) => {
            let size = f
                .info
                .as_ref()
                .and_then(|i| i.size)
                .map(|s| u64::from(s).min(i32::MAX as u64) as i32)
                .unwrap_or(100_000i32);
            ("FILE".into(), size)
        }
        MessageType::Audio(ref a) => {
            let size = a
                .info
                .as_ref()
                .and_then(|i| i.size)
                .map(|s| u64::from(s).min(i32::MAX as u64) as i32)
                .unwrap_or(100_000i32);
            ("AUDIO".into(), size)
        }
        MessageType::Location(_) => ("LOCATION".into(), 200i32),
        MessageType::Emote(ref t) => (t.body.clone(), t.body.len() as i32),
        _ => return,
    };

    // Log bandwidth estimate for bridge traffic tracking
    if let Err(e) =
        state
            .bandwidth_repository
            .log_bandwidth(user_id, &service, "inbound", bytes_estimate)
    {
        tracing::warn!("Failed to log bandwidth for user {}: {}", user_id, e);
    }

    let chat_name = remove_bridge_suffix(room_name.as_str());
    let current_room_id = room.room_id().to_string();

    // Skip WhatsApp "Status Broadcast" (the disappearing-stories pseudo-chat
    // mautrix-whatsapp creates for status@broadcast). Every status post from
    // any contact lands here and is noise for digests/notifications.
    if service == "whatsapp" && chat_name.to_lowercase().contains("status broadcast") {
        return;
    }

    // Skip bridge-generated error notices so they are logged but never treated
    // as user messages for notification/proactive rules.
    if is_error_message(&content) {
        log_bridge_error_notice("incoming", user_id, &service, &current_room_id, &content);
        return;
    }

    // Detect call event notices from mautrix bridges (e.g. "Incoming call", "Missed call")
    let is_call_event = is_call_event_message(&content);

    // Check if this is a group chat (same heuristic as get_service_rooms)
    let is_group = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
        Ok(members) => {
            let member_localparts: Vec<String> = members
                .iter()
                .map(|member| member.user_id().localpart().to_string())
                .collect();
            is_group_room(&service, &member_localparts)
        }
        Err(_) => false,
    };

    // Ontology Person lookup (for person_id on the message)
    let matching_person = state
        .ontology_repository
        .find_person_by_room_id(user_id, &current_room_id)
        .unwrap_or_else(|e| {
            tracing::warn!("Ontology lookup failed for room {}: {}", current_room_id, e);
            None
        });

    // Auto-create Person for recognized phone contacts (WhatsApp/Signal only).
    // Telegram returns None from is_phone_contact_from_room_name - no auto-creation.
    let matching_person = if matching_person.is_none() && !is_group {
        match is_phone_contact_from_room_name(&room_name) {
            Some(true) => {
                tracing::info!(
                    "Auto-creating person '{}' on {} for user {} (phone contact)",
                    chat_name,
                    service,
                    user_id
                );
                match state.ontology_repository.upsert_person(
                    user_id,
                    &chat_name,
                    &service,
                    None,
                    Some(&current_room_id),
                ) {
                    Ok(person) => {
                        tracing::info!("Created person '{}' (id={})", person.name, person.id);
                        // Fetch the full PersonWithChannels for consistency
                        state
                            .ontology_repository
                            .get_person_with_channels(user_id, person.id)
                            .ok()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to auto-create person '{}': {}", chat_name, e);
                        None
                    }
                }
            }
            _ => None, // Not a contact (Some(false)) or can't determine (None/Telegram)
        }
    } else {
        matching_person
    };

    // Store message in ont_messages + emit ontology change.
    // Rules handle all notification logic from here.
    let person_id = matching_person.as_ref().map(|p| p.person.id);
    let person_name: Option<String> = matching_person
        .as_ref()
        .map(|p| p.display_name().to_string());
    // Use the bridged sender's matrix user id as the stable per-sender key.
    // Bridge ghosts have a canonical form like @_telegram_<id>:homeserver, so
    // the full mxid is unique per external sender and survives display-name
    // changes. Falls back to None when localpart is unavailable.
    let sender_key_value = Some(event.sender.to_string());
    let msg = crate::models::ontology_models::NewOntMessage {
        user_id,
        room_id: current_room_id.clone(),
        platform: service.clone(),
        sender_name: chat_name.clone(),
        sender_key: sender_key_value,
        content: content.clone(),
        person_id,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32,
        matrix_event_id: Some(event.event_id.to_string()),
    };
    let state_clone = state.clone();
    let stored_service = service.clone();
    let cleanup_room_id = current_room_id.clone();
    tokio::spawn(async move {
        match state_clone.ontology_repository.insert_message(&msg) {
            Ok((created, is_new)) => {
                if !is_new {
                    bump_skipped_duplicate(&stored_service);
                    return;
                }
                bump_stored(&stored_service);
                crate::utils::tuwunel_event_cleanup::enqueue_processed_bridge_event(
                    &state_clone,
                    user_id,
                    &stored_service,
                    &cleanup_room_id,
                    &cleanup_matrix_event_id,
                    created.id,
                    cleanup_tuwunel_media,
                );
                let mut snapshot = serde_json::json!({
                    "message_id": created.id,
                    "platform": msg.platform,
                    "sender": msg.sender_name,
                    "sender_name": msg.sender_name,
                    "sender_key": msg.sender_key,
                    "content": msg.content,
                    "room_id": msg.room_id,
                    "is_group": is_group,
                });
                if let Some(ref pn) = person_name {
                    snapshot["person_name"] = serde_json::Value::String(pn.clone());
                }
                if let Some(pid) = person_id {
                    snapshot["person_id"] = serde_json::json!(pid);
                }
                if has_auto_features {
                    let entity_type = if is_call_event { "Call" } else { "Message" };
                    crate::proactive::rules::emit_ontology_change(
                        &state_clone,
                        user_id,
                        entity_type,
                        created.id as i32,
                        "created",
                        snapshot,
                    )
                    .await;
                } else {
                    tracing::debug!(
                        "Stored bridge message {} for user {} without emitting auto-feature ontology change",
                        created.id,
                        user_id
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to store bridge message: {}", e);
            }
        }
    });
}

/// Fetch contacts via mautrix bridge provisioning API (v3).
/// Returns (total_contacts, matched_results).
/// Requires WHATSAPP_BRIDGE_URL / SIGNAL_BRIDGE_URL / TELEGRAM_BRIDGE_URL env var.
async fn fetch_provision_contacts(
    client: &MatrixClient,
    service: &str,
    search_term: &str,
) -> Result<(usize, Vec<BridgeRoom>)> {
    let bridge_url_var = match service {
        "whatsapp" => "WHATSAPP_BRIDGE_URL",
        "signal" => "SIGNAL_BRIDGE_URL",
        "telegram" => "TELEGRAM_BRIDGE_URL",
        _ => return Err(anyhow!("Unknown service: {}", service)),
    };
    let bridge_url =
        std::env::var(bridge_url_var).map_err(|_| anyhow!("{} not set", bridge_url_var))?;

    let access_token = client
        .matrix_auth()
        .session()
        .map(|s| s.tokens.access_token.clone())
        .unwrap_or_default();

    if access_token.is_empty() {
        return Err(anyhow!("No Matrix access token"));
    }

    let contacts_url = format!(
        "{}/_matrix/provision/v3/contacts",
        bridge_url.trim_end_matches('/')
    );

    let http_client = reqwest::Client::new();
    let resp = http_client
        .get(&contacts_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow!("Provision API returned status {}", resp.status()));
    }

    let body: serde_json::Value = resp.json().await?;
    let contacts = body["contacts"].as_array();
    let total = contacts.map(|c| c.len()).unwrap_or(0);

    if total == 0 {
        return Ok((0, Vec::new()));
    }

    let search_lower = search_term.trim().to_lowercase();
    let mut matched = Vec::new();

    if let Some(contacts) = contacts {
        for contact in contacts {
            let name = contact["name"].as_str().unwrap_or_default();
            let name_clean = remove_bridge_suffix(name);
            if name_clean.is_empty() {
                continue;
            }
            let name_lower = name_clean.to_lowercase();
            let is_match = name_lower == search_lower
                || name_lower.contains(&search_lower)
                || strsim::jaro_winkler(&name_lower, &search_lower) >= 0.7;

            if is_match {
                let dm_room = contact["dm_room_mxid"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                matched.push(BridgeRoom {
                    room_id: dm_room,
                    display_name: name_clean,
                    last_activity: 0,
                    last_activity_formatted: "Contact".to_string(),
                    is_group: false,
                });
            }
        }
    }

    Ok((total, matched))
}

/// Fetch contacts via Matrix user directory search API.
/// Returns (total_users_returned, matched_results).
async fn fetch_directory_contacts(
    client: &MatrixClient,
    service: &str,
    search_term: &str,
) -> Result<(usize, Vec<BridgeRoom>)> {
    let homeserver_url =
        std::env::var("MATRIX_HOMESERVER").map_err(|_| anyhow!("MATRIX_HOMESERVER not set"))?;
    let access_token = client
        .matrix_auth()
        .session()
        .map(|s| s.tokens.access_token.clone())
        .unwrap_or_default();

    if access_token.is_empty() {
        return Err(anyhow!("No Matrix access token"));
    }

    let search_url = format!(
        "{}/_matrix/client/v3/user_directory/search",
        homeserver_url.trim_end_matches('/')
    );
    let http_client = reqwest::Client::new();
    let resp = http_client
        .post(&search_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&serde_json::json!({
            "search_term": search_term.trim(),
            "limit": 50
        }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow!("User directory returned status {}", resp.status()));
    }

    let body: serde_json::Value = resp.json().await?;
    let users = body["results"].as_array();
    let total = users.map(|u| u.len()).unwrap_or(0);

    if total == 0 {
        return Ok((0, Vec::new()));
    }

    let sender_prefix = get_sender_prefix(service);
    let bot_names: [String; 2] = [format!("{}bot", service), format!("{}-bridge", service)];
    let search_lower = search_term.trim().to_lowercase();
    let mut matched = Vec::new();

    if let Some(users) = users {
        for user in users {
            let uid = user["user_id"].as_str().unwrap_or_default();
            let localpart = uid
                .trim_start_matches('@')
                .split(':')
                .next()
                .unwrap_or_default();

            if !localpart.starts_with(&sender_prefix) {
                continue;
            }
            if bot_names.iter().any(|b| localpart == b.as_str()) {
                continue;
            }

            let raw_display = user["display_name"].as_str().unwrap_or_default();
            let display_name = if !raw_display.is_empty() {
                remove_bridge_suffix(raw_display)
            } else {
                let number = localpart.trim_start_matches(&sender_prefix);
                if number.chars().all(|c| c.is_ascii_digit()) && number.len() > 5 {
                    format!("+{}", number)
                } else {
                    continue;
                }
            };

            let name_lower = display_name.to_lowercase();
            let is_match = name_lower == search_lower
                || name_lower.contains(&search_lower)
                || strsim::jaro_winkler(&name_lower, &search_lower) >= 0.7;

            if is_match {
                matched.push(BridgeRoom {
                    room_id: String::new(),
                    display_name,
                    last_activity: 0,
                    last_activity_formatted: "Contact".to_string(),
                    is_group: false,
                });
            }
        }
    }

    Ok((total, matched))
}

/// Fetch contacts directly from the bridge bot via management room commands.
/// Sends `!<service> list contacts` and parses the response.
/// Returns (all_contact_names, matched_results) so the caller can log totals.
async fn fetch_bridge_contacts(
    client: &MatrixClient,
    service: &str,
    mgmt_room_id: &str,
    search_term: &str,
) -> Result<(usize, Vec<BridgeRoom>)> {
    use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;

    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(mgmt_room_id)?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| anyhow!("Management room not found in client"))?;

    // Determine bridge command. WA/Signal (bridgev2) expose `list contacts`
    // and we filter client-side. mautrix-telegram v0.15.3 (Python) has no
    // such command — `list contacts` returns "Unknown command", silently
    // hanging this function for 7.5s every call. Use its actual contact
    // search command instead, which filters server-side.
    let cmd = match service {
        "whatsapp" => "!wa list contacts".to_string(),
        "signal" => "!signal list contacts".to_string(),
        "telegram" => format!("!tg search {}", search_term.trim()),
        _ => return Err(anyhow!("Unknown service: {}", service)),
    };

    tracing::info!(
        "BRIDGE_CONTACTS: Sending '{}' to mgmt room {}",
        cmd,
        mgmt_room_id
    );
    room.send(RoomMessageEventContent::text_plain(&cmd)).await?;

    // Get bridge bot user ID
    let bridge_bot_var = match service {
        "signal" => "SIGNAL_BRIDGE_BOT",
        "whatsapp" => "WHATSAPP_BRIDGE_BOT",
        "telegram" => "TELEGRAM_BRIDGE_BOT",
        _ => return Err(anyhow!("Unknown service")),
    };
    let bridge_bot =
        std::env::var(bridge_bot_var).map_err(|_| anyhow!("{} not set", bridge_bot_var))?;
    let bot_user_id = matrix_sdk::ruma::OwnedUserId::try_from(bridge_bot)?;

    // Poll for the bot's response (up to 15 attempts, ~15s)
    let sync_settings =
        matrix_sdk::config::SyncSettings::default().timeout(std::time::Duration::from_secs(2));
    let mut contact_names: Vec<String> = Vec::new();

    for attempt in 1..=15 {
        client.sync_once(sync_settings.clone()).await.ok();

        let Some(room) = client.get_room(&room_id) else {
            continue;
        };
        let mut options = MessagesOptions::backward();
        options.limit = matrix_sdk::ruma::UInt::new(5).unwrap();
        let messages = match room.messages(options).await {
            Ok(m) => m,
            Err(_) => continue,
        };

        for msg in &messages.chunk {
            if let Ok(event) = msg.raw().deserialize() {
                if event.sender() != bot_user_id {
                    continue;
                }
                if let AnySyncTimelineEvent::MessageLike(
                    matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(
                        SyncRoomMessageEvent::Original(e),
                    ),
                ) = event
                {
                    let body = match &e.content.msgtype {
                        MessageType::Text(t) => &t.body,
                        MessageType::Notice(n) => &n.body,
                        _ => continue,
                    };

                    // Parse contact list response.
                    // Format: "* Name / [DisplayName](mxc_link) - `+phone`"
                    // or bridgev2: "* `user_id` / Name"
                    // Also handles "### Contacts" section headers.
                    if body.contains("Contacts") || body.contains("* ") {
                        for line in body.lines() {
                            let line = line.trim();
                            if !line.starts_with("* ") && !line.starts_with("- ") {
                                continue;
                            }
                            let entry = line.trim_start_matches("* ").trim_start_matches("- ");
                            // Extract name: take text before " / " or " - " delimiters
                            let name = entry
                                .split(" / ")
                                .next()
                                .unwrap_or(entry)
                                .split(" - ")
                                .next()
                                .unwrap_or(entry)
                                .trim()
                                // Strip markdown link syntax: [Name](url) -> Name
                                .trim_start_matches('[')
                                .split("](")
                                .next()
                                .unwrap_or("")
                                .trim_end_matches(']')
                                // Strip backtick wrapping
                                .trim_matches('`')
                                .trim();

                            if !name.is_empty() && name.len() > 1 {
                                contact_names.push(name.to_string());
                            }
                        }
                        if !contact_names.is_empty() {
                            tracing::info!(
                                "BRIDGE_CONTACTS: Parsed {} contacts from bot response on attempt {}",
                                contact_names.len(),
                                attempt
                            );
                            break;
                        }
                    }
                }
            }
        }
        if !contact_names.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let total_contacts = contact_names.len();
    if total_contacts == 0 {
        tracing::info!("BRIDGE_CONTACTS: No contacts returned from bridge command");
        return Ok((0, Vec::new()));
    }

    // Filter contacts by search term
    let search_lower = search_term.trim().to_lowercase();
    let matched: Vec<BridgeRoom> = contact_names
        .into_iter()
        .filter_map(|name| {
            let name_lower = name.to_lowercase();
            let name_cleaned = remove_bridge_suffix(&name).to_lowercase();
            let is_match = name_cleaned == search_lower
                || name_lower == search_lower
                || name_cleaned.contains(&search_lower)
                || name_lower.contains(&search_lower)
                || strsim::jaro_winkler(&name_cleaned, &search_lower) >= 0.7;

            if is_match {
                Some(BridgeRoom {
                    room_id: String::new(), // no room yet - will be created on first message
                    display_name: remove_bridge_suffix(&name),
                    last_activity: 0,
                    last_activity_formatted: "Contact".to_string(),
                    is_group: false,
                })
            } else {
                None
            }
        })
        .collect();

    tracing::info!(
        "BRIDGE_CONTACTS: service={} total_contacts={} matched_contacts={}",
        service,
        total_contacts,
        matched.len()
    );

    Ok((total_contacts, matched))
}

pub async fn search_bridge_rooms(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    search_term: &str,
) -> Result<Vec<BridgeRoom>> {
    // Validate bridge connection first
    let bridge = state.user_repository.get_bridge(user_id, service)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!(
            "{} bridge is not connected. Please log in first.",
            capitalize(service)
        ));
    }
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
    // Sync to pick up rooms created/joined by the bridge via double puppet
    client
        .sync_once(matrix_sdk::config::SyncSettings::new())
        .await
        .ok();
    let all_rooms = get_service_rooms(&client, service).await?;
    let search_term_lower = search_term.trim().to_lowercase();
    // Single-pass matching with prioritized results
    let mut matching_rooms: Vec<(f64, BridgeRoom)> = all_rooms
        .into_iter()
        .filter_map(|room| {
            bridge_room_matches_search(&room.display_name, &search_term_lower)
                .map(|score| (score, room))
        })
        .collect();
    // Sort by match quality (higher score = better match) and then by last activity
    matching_rooms.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.1.last_activity.cmp(&a.1.last_activity))
    });
    tracing::info!(
        "Found {} matching {} rooms",
        matching_rooms.len(),
        capitalize(service)
    );

    let mut results: Vec<BridgeRoom> = matching_rooms.into_iter().map(|(_, room)| room).collect();
    let room_result_count = results.len();

    // Try all three contact search methods, log counts, pick the one with most total contacts.
    if search_term.trim().len() >= 2 {
        // Method 1: Provisioning API (structured JSON, needs BRIDGE_URL env var)
        let provision_result = fetch_provision_contacts(&client, service, search_term).await;
        let (prov_total, prov_matched, prov_room_ids) = match &provision_result {
            Ok((total, matched)) => (
                *total,
                matched.len(),
                matched
                    .iter()
                    .filter(|room| !room.room_id.is_empty())
                    .count(),
            ),
            Err(e) => {
                tracing::info!("CONTACT_SEARCH: provision API skip for {}: {}", service, e);
                (0, 0, 0)
            }
        };

        // Method 2: Bridge command via management room
        let bridge = state.user_repository.get_bridge(user_id, service)?;
        let mgmt_room_id = bridge.and_then(|b| b.room_id);
        let bridge_cmd_result = if let Some(ref mgmt_room) = mgmt_room_id {
            match tokio::time::timeout(
                Duration::from_secs(4),
                fetch_bridge_contacts(&client, service, mgmt_room, search_term),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Err(anyhow!("Bridge command timed out")),
            }
        } else {
            Err(anyhow!("No management room"))
        };
        let (cmd_total, cmd_matched, cmd_room_ids) = match &bridge_cmd_result {
            Ok((total, matched)) => (
                *total,
                matched.len(),
                matched
                    .iter()
                    .filter(|room| !room.room_id.is_empty())
                    .count(),
            ),
            Err(e) => {
                tracing::info!("CONTACT_SEARCH: bridge command skip for {}: {}", service, e);
                (0, 0, 0)
            }
        };

        // Method 3: Matrix user directory search
        let directory_result = fetch_directory_contacts(&client, service, search_term).await;
        let (dir_total, dir_matched, dir_room_ids) = match &directory_result {
            Ok((total, matched)) => (
                *total,
                matched.len(),
                matched
                    .iter()
                    .filter(|room| !room.room_id.is_empty())
                    .count(),
            ),
            Err(e) => {
                tracing::info!("CONTACT_SEARCH: user directory skip for {}: {}", service, e);
                (0, 0, 0)
            }
        };

        // Log comparison of all three methods
        tracing::info!(
            "CONTACT_SEARCH_COMPARE: service={} query_len={} rooms=(matched:{}, room_ids:{}) provision=(total:{}, matched:{}, room_ids:{}) bridge_cmd=(total:{}, matched:{}, room_ids:{}) directory=(total:{}, matched:{}, room_ids:{})",
            service, search_term.trim().chars().count(), room_result_count,
            room_result_count,
            prov_total, prov_matched, prov_room_ids,
            cmd_total, cmd_matched, cmd_room_ids,
            dir_total, dir_matched, dir_room_ids,
        );

        // Pick the method with the most total contacts (best coverage)
        let best_contacts = if prov_total >= cmd_total && prov_total >= dir_total && prov_total > 0
        {
            tracing::info!(
                "CONTACT_SEARCH: WINNER=provision ({} total contacts)",
                prov_total
            );
            provision_result.unwrap().1
        } else if cmd_total >= dir_total && cmd_total > 0 {
            tracing::info!(
                "CONTACT_SEARCH: WINNER=bridge_cmd ({} total contacts)",
                cmd_total
            );
            bridge_cmd_result.unwrap().1
        } else if dir_total > 0 {
            tracing::info!(
                "CONTACT_SEARCH: WINNER=directory ({} total contacts)",
                dir_total
            );
            directory_result.unwrap().1
        } else {
            tracing::info!("CONTACT_SEARCH: no additional contacts from any method");
            Vec::new()
        };

        // Deduplicate: only add contacts not already in room results
        let mut existing_names: HashSet<String> = results
            .iter()
            .map(|r| remove_bridge_suffix(&r.display_name).to_lowercase())
            .collect();
        for contact in best_contacts {
            let contact_name = remove_bridge_suffix(&contact.display_name).to_lowercase();
            if existing_names.insert(contact_name) {
                results.push(contact);
            }
        }
    }

    tracing::info!(
        "search_bridge_rooms: final_results={} service={} query_len={}",
        results.len(),
        capitalize(service),
        search_term.trim().chars().count()
    );

    Ok(results)
}

/// Search only the Matrix rooms the client already knows about.
///
/// This is intentionally narrower than `search_bridge_rooms`: it does not call
/// provisioning APIs, user-directory search, or bridge management-room commands.
/// Dashboard autocomplete needs a bounded, non-mutating lookup; bridge commands
/// can block the backend when the bridge bot does not answer.
pub async fn search_bridge_rooms_by_name(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    search_term: &str,
) -> Result<Vec<BridgeRoom>> {
    let bridge = state.user_repository.get_bridge(user_id, service)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!(
            "{} bridge is not connected. Please log in first.",
            capitalize(service)
        ));
    }

    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
    let all_rooms = get_service_rooms(&client, service).await?;
    let search_term_lower = search_term.trim().to_lowercase();

    let mut matching_rooms: Vec<(f64, BridgeRoom)> = all_rooms
        .into_iter()
        .filter_map(|room| {
            bridge_room_matches_search(&room.display_name, &search_term_lower)
                .map(|score| (score, room))
        })
        .collect();

    matching_rooms.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.1.last_activity.cmp(&a.1.last_activity))
    });

    let results: Vec<BridgeRoom> = matching_rooms.into_iter().map(|(_, room)| room).collect();
    tracing::info!(
        "search_bridge_rooms_by_name: final_results={} service={} query_len={}",
        results.len(),
        capitalize(service),
        search_term.trim().chars().count()
    );

    Ok(results)
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Send a read-only command to a bridge management room and collect responses
/// from the bridge bot that arrived AFTER the command was sent. Filters by
/// origin_server_ts to avoid matching stale messages from history.
///
/// Used by health checks to get a fresh, authoritative status from the bridge
/// without parsing potentially-stale historical messages.
///
/// Returns the raw text bodies of bridge bot messages newer than the send
/// timestamp, in chronological order. Empty vec means the bot didn't respond
/// within `max_wait` (could mean bot is down, command was unrecognized but
/// silent, or sync was lagging).
pub async fn probe_bridge_room(
    client: &MatrixClient,
    room: &Room,
    bot_user_id: &matrix_sdk::ruma::OwnedUserId,
    command: &str,
    max_wait: Duration,
) -> Result<Vec<String>> {
    use matrix_sdk::config::SyncSettings as MatrixSyncSettings;
    use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
    use std::time::Instant;
    use tokio::time::sleep;

    // Record send timestamp BEFORE sending so we can filter stale messages.
    // Matrix origin_server_ts is in milliseconds since epoch.
    let cmd_sent_ts_ms: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    room.send(RoomMessageEventContent::text_plain(command))
        .await
        .map_err(|e| anyhow!("send command failed: {}", e))?;

    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(1));
    let deadline = Instant::now() + max_wait;
    let mut responses: Vec<(u64, String)> = Vec::new();

    while Instant::now() < deadline {
        let _ = client.sync_once(sync_settings.clone()).await;

        let mut opts =
            matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
        opts.limit = matrix_sdk::ruma::UInt::new(30).unwrap();
        if let Ok(messages) = room.messages(opts).await {
            for msg in &messages.chunk {
                if let Ok(event) = msg.raw().deserialize() {
                    if event.sender() != bot_user_id {
                        continue;
                    }
                    let ts_ms: u64 = i64::from(event.origin_server_ts().0) as u64;
                    if ts_ms <= cmd_sent_ts_ms {
                        continue;
                    }
                    if responses.iter().any(|(t, _)| *t == ts_ms) {
                        continue;
                    }
                    if let AnySyncTimelineEvent::MessageLike(
                        matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(sync_event),
                    ) = event
                    {
                        let content = match sync_event {
                            SyncRoomMessageEvent::Original(e) => e.content,
                            SyncRoomMessageEvent::Redacted(_) => continue,
                        };
                        let body = match content.msgtype {
                            MessageType::Text(t) => t.body,
                            MessageType::Notice(n) => n.body,
                            _ => continue,
                        };
                        responses.push((ts_ms, body));
                    }
                }
            }
        }

        if !responses.is_empty() {
            // Got at least one response. Wait one more 500ms in case there are
            // multi-line follow-ups, then bail.
            sleep(Duration::from_millis(500)).await;
            // Re-sync once more to pick up any straggler.
            let _ = client.sync_once(sync_settings.clone()).await;
            if let Ok(messages) = room
                .messages({
                    let mut o = matrix_sdk::room::MessagesOptions::new(
                        matrix_sdk::ruma::api::Direction::Backward,
                    );
                    o.limit = matrix_sdk::ruma::UInt::new(30).unwrap();
                    o
                })
                .await
            {
                for msg in &messages.chunk {
                    if let Ok(event) = msg.raw().deserialize() {
                        if event.sender() != bot_user_id {
                            continue;
                        }
                        let ts_ms: u64 = i64::from(event.origin_server_ts().0) as u64;
                        if ts_ms <= cmd_sent_ts_ms {
                            continue;
                        }
                        if responses.iter().any(|(t, _)| *t == ts_ms) {
                            continue;
                        }
                        if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(
                                sync_event,
                            ),
                        ) = event
                        {
                            let content = match sync_event {
                                SyncRoomMessageEvent::Original(e) => e.content,
                                SyncRoomMessageEvent::Redacted(_) => continue,
                            };
                            let body = match content.msgtype {
                                MessageType::Text(t) => t.body,
                                MessageType::Notice(n) => n.body,
                                _ => continue,
                            };
                            responses.push((ts_ms, body));
                        }
                    }
                }
            }
            break;
        }

        sleep(Duration::from_millis(500)).await;
    }

    // Sort by ts ascending so caller sees responses in chronological order.
    responses.sort_by_key(|(t, _)| *t);
    Ok(responses.into_iter().map(|(_, b)| b).collect())
}

// NOTE: `list_logins_has_connected` and `extract_first_connected_identifier`
// were removed in favour of the strict exact-match parser in
// `crate::utils::bridge_responses`. The new parser requires verified backtick
// formatting around both the login id and the status field; fuzzy variants
// (e.g. "- CONNECTED" without backticks) are rejected on purpose.

/// Log out every CONNECTED login in a bridgev2 management room.
///
/// Empirical finding: mautrix-whatsapp and mautrix-signal v26.04 both REQUIRE
/// the login_id argument to `logout`. Bare `!<prefix> logout` returns a
/// usage message and is a silent no-op. To log out properly we must:
///
/// 1. Probe `!<prefix> list-logins` and parse each CONNECTED entry.
/// 2. For each, send `!<prefix> logout <login_id>`.
/// 3. Verify the reply is exactly `"Logged out"` (anything else means the
///    bridge state differs from what we expected - log a warning).
///
/// Safe no-op when nothing is logged in. `cmd_prefix` must be `"!wa"` for
/// WhatsApp or `"!signal"` for Signal. Returns the count of logins
/// confirmed-logged-out (not counting skipped/unexpected responses).
pub async fn logout_all_bridgev2_logins(
    client: &MatrixClient,
    room: &Room,
    bot_user_id: &matrix_sdk::ruma::OwnedUserId,
    cmd_prefix: &str,
) -> Result<usize> {
    use crate::utils::bridge_responses::{parse_list_logins, verified};

    let list_cmd = format!("{} list-logins", cmd_prefix);
    let list_responses =
        probe_bridge_room(client, room, bot_user_id, &list_cmd, Duration::from_secs(8)).await?;
    let combined = list_responses.join("\n");
    let entries = parse_list_logins(&combined);
    let connected: Vec<_> = entries
        .iter()
        .filter(|e| e.status == verified::bridgev2::STATUS_CONNECTED)
        .collect();

    if connected.is_empty() {
        tracing::debug!(
            "{} logout: no CONNECTED logins (list-logins body: {:?}), skipping",
            cmd_prefix,
            combined
        );
        return Ok(0);
    }

    let mut logged_out = 0usize;
    for entry in connected {
        let logout_cmd = format!("{} logout {}", cmd_prefix, entry.login_id);
        let logout_responses = match probe_bridge_room(
            client,
            room,
            bot_user_id,
            &logout_cmd,
            Duration::from_secs(8),
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    "{} logout probe failed for login_id={}: {}",
                    cmd_prefix,
                    entry.login_id,
                    e
                );
                continue;
            }
        };
        let body = logout_responses
            .first()
            .map(String::as_str)
            .unwrap_or("(no reply)");

        // Both WA and Signal return the same exact body on success.
        if body == "Logged out" {
            logged_out += 1;
            tracing::info!("{} logout ok for login_id={}", cmd_prefix, entry.login_id);
        } else {
            // Divergence: log the full body so we can update classifiers if
            // the bridge changes wire format.
            tracing::warn!(
                "{} logout unexpected reply for login_id={} body={:?}",
                cmd_prefix,
                entry.login_id,
                body
            );
        }
    }
    Ok(logged_out)
}

/// Send `!wa start-chat +<phone>` to the WhatsApp bridge management room and
/// return the Matrix room ID of the (possibly newly created) portal.
///
/// Usage: call this when the bridge DB portal lookup returns None for a DM
/// JID. The bridge will materialize the portal, invite our Matrix user, and
/// reply with a single message containing the matrix.to URL of the new room.
/// We parse that reply via `classify_whatsapp_start_chat` to extract the room
/// ID without race-prone Matrix invite polling.
///
/// `chat_id` must be a full DM JID like `"358442055570@s.whatsapp.net"`. The
/// phone is extracted (localpart before `@`) and prefixed with `+` per the
/// verified command syntax.
pub async fn start_chat_whatsapp(
    state: &Arc<AppState>,
    user_id: i32,
    chat_id: &str,
) -> Result<matrix_sdk::ruma::OwnedRoomId> {
    use crate::utils::bridge_responses::{classify_whatsapp_start_chat, WhatsAppStartChatReply};

    // Extract the phone localpart from the JID.
    let (localpart, server) = chat_id
        .split_once('@')
        .ok_or_else(|| anyhow!("start_chat_whatsapp: chat_id '{}' missing '@'", chat_id))?;
    if server != "s.whatsapp.net" {
        return Err(anyhow!(
            "start_chat_whatsapp: chat_id '{}' is not a DM JID (expected @s.whatsapp.net)",
            chat_id
        ));
    }
    // Some JIDs have a device suffix like "phone:device". Strip it.
    let phone = localpart.split(['.', ':']).next().unwrap_or(localpart);
    if phone.is_empty() || !phone.chars().all(|c| c.is_ascii_digit()) {
        return Err(anyhow!(
            "start_chat_whatsapp: chat_id '{}' has non-numeric phone part",
            chat_id
        ));
    }

    // Resolve management room for this user + bridge.
    let bridge = state
        .user_repository
        .get_bridge(user_id, "whatsapp")?
        .ok_or_else(|| anyhow!("no WhatsApp bridge record for user {}", user_id))?;
    let mgmt_room_id_str = bridge.room_id.ok_or_else(|| {
        anyhow!(
            "WhatsApp bridge has no management room_id for user {}",
            user_id
        )
    })?;
    let mgmt_room_id =
        matrix_sdk::ruma::OwnedRoomId::try_from(mgmt_room_id_str.as_str()).map_err(|e| {
            anyhow!(
                "invalid WA management room id '{}': {}",
                mgmt_room_id_str,
                e
            )
        })?;

    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
    let room = client.get_room(&mgmt_room_id).ok_or_else(|| {
        anyhow!(
            "WA management room {} not found in client",
            mgmt_room_id_str
        )
    })?;

    let bridge_bot =
        std::env::var("WHATSAPP_BRIDGE_BOT").map_err(|_| anyhow!("WHATSAPP_BRIDGE_BOT not set"))?;
    let bot_user_id = matrix_sdk::ruma::OwnedUserId::try_from(bridge_bot.as_str())
        .map_err(|e| anyhow!("invalid WHATSAPP_BRIDGE_BOT user id: {}", e))?;

    let cmd = format!("!wa start-chat +{}", phone);
    tracing::info!(
        "SEND_FLOW_BRIDGE start_chat_whatsapp user={} sending {:?}",
        user_id,
        cmd
    );
    let responses =
        probe_bridge_room(&client, &room, &bot_user_id, &cmd, Duration::from_secs(10)).await?;
    if responses.is_empty() {
        return Err(anyhow!(
            "WA start-chat bot did not reply within timeout (phone=+{})",
            phone
        ));
    }

    // Scan all responses and take the first one that matches either shape.
    for body in &responses {
        match classify_whatsapp_start_chat(body) {
            Some(WhatsAppStartChatReply::Created {
                room_id,
                display_name,
            }) => {
                tracing::info!(
                    "SEND_FLOW_BRIDGE start_chat_whatsapp ok: room_id={} display_name={}",
                    room_id,
                    display_name
                );
                return matrix_sdk::ruma::OwnedRoomId::try_from(room_id.as_str())
                    .map_err(|e| anyhow!("invalid room id from WA start-chat reply: {}", e));
            }
            Some(WhatsAppStartChatReply::Failed { reason }) => {
                return Err(anyhow!("WA start-chat failed: {}", reason));
            }
            None => continue,
        }
    }

    Err(anyhow!(
        "WA start-chat bot reply was unrecognised: {:?}",
        responses
    ))
}
