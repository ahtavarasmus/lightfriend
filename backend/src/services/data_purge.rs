// Self-service "Delete my data" flow.
//
// This module is the single source of truth for what happens when a user
// clicks the "Delete my data" button in their settings. The frontend links
// directly to this file on GitHub so users can audit the code that will run.
//
// Scope (what this deletes):
// - All 25 user-owned database tables: bridges, messages, IMAP connections,
//   Tesla, YouTube, MCP servers, items, user_secrets, user_info,
//   user_settings, usage logs, message status log, bridge logs, refund
//   info, and the 8 ontology tables (persons, channels, messages, events,
//   links, rules, person_edits, changelog).
// - The Matrix session store on disk (SQLite, encryption keys, room cache).
// - All in-memory background tasks tied to the user (Matrix sync loop,
//   Tesla monitors, IMAP IDLE tasks) and their DashMap entries.
//
// Also performed (best-effort, within a 15s per-bridge timeout):
// - Logout from WhatsApp, Signal, and Telegram at the bridge level, so the
//   linked account on those external services is disconnected.
//
// Scope (what this keeps):
// - The `users` row itself: email, password hash, subscription tier,
//   Stripe customer id, phone number. The user can still log in.
// - Passkeys (webauthn_credentials) and TOTP secrets / backup codes, since
//   these are login credentials rather than user data.
//
// What this does NOT do:
// - Does not revoke refresh tokens held by external services (Stripe,
//   Tesla, YouTube). Those remain valid on the provider side until the
//   user revokes them there. The local copies here are destroyed.
// - Does not deactivate the Matrix homeserver account. The local session
//   store is wiped; the homeserver-side ghost remains idle.
//
// Called by:
// - `profile_handlers::purge_my_data` (user-initiated, password-gated).
// - `repositories::user_core::UserCore::delete_user` (which then also
//   deletes the `users` row itself, for full account deletion).

use crate::pg_schema;
use crate::{AppState, PgDbPool};
use anyhow::{anyhow, Result};
use dashmap::DashMap;
use diesel::prelude::*;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::OwnedRoomId;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout};

/// Minimal view of a bridge row — only the fields the purge flow needs.
struct BridgeEntry {
    bridge_type: String,
    room_id: Option<String>,
}

/// Per-bridge logout timeout. A stuck bridge must not prevent local data
/// wipe — data-at-rest on our servers is the primary concern.
const BRIDGE_LOGOUT_TIMEOUT: Duration = Duration::from_secs(15);

/// Status of one step of the purge as it's reported to the frontend.
#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Running,
    Done,
    Skipped,
    Failed,
}

/// One step of the purge, visible to the frontend for live progress display.
#[derive(Clone, Serialize)]
pub struct PurgeStep {
    pub id: &'static str,
    pub label: &'static str,
    pub status: StepStatus,
    pub detail: Option<String>,
}

/// Full progress state for one purge run.
///
/// The frontend polls this via GET /api/profile/purge-data/status/:purge_id
/// and re-renders the step list on every tick.
#[derive(Clone, Serialize)]
pub struct PurgeState {
    /// User who initiated the purge. Used on the status endpoint to
    /// refuse cross-user reads.
    #[serde(skip)]
    pub user_id: i32,
    pub steps: Vec<PurgeStep>,
    pub complete: bool,
    pub error: Option<String>,
}

impl PurgeState {
    pub fn new(user_id: i32) -> Self {
        Self {
            user_id,
            steps: Vec::new(),
            complete: false,
            error: None,
        }
    }
}

/// Shared map of in-flight purges keyed by purge_id. Stored on AppState.
pub type PurgeRegistry = Arc<DashMap<String, Arc<Mutex<PurgeState>>>>;

pub fn new_registry() -> PurgeRegistry {
    Arc::new(DashMap::new())
}

/// Run the full data-purge flow for `user_id`, reporting live progress
/// via the shared `state_handle` so the frontend can poll and display
/// each step as it completes.
///
/// Leaves the `users` row, passkeys, TOTP, and subscription intact.
/// Everything else the user owns is destroyed.
///
/// Order of operations is deliberate:
/// 1. Bridge logouts first — requires a live Matrix client.
/// 2. Capture bookkeeping (Matrix username, IMAP connection ids) before
///    the DB wipe destroys the source data.
/// 3. Abort background tasks before step 4 — otherwise a running sync
///    loop or monitor can re-insert DB rows seconds after we delete them.
/// 4. DB transaction: delete every user-owned row.
/// 5. Filesystem: remove the Matrix session store on disk.
pub async fn purge_user_data(
    state: Arc<AppState>,
    user_id: i32,
    state_handle: Arc<Mutex<PurgeState>>,
) -> Result<()> {
    tracing::info!("Starting data purge for user {}", user_id);

    let matrix_username = step(
        &state_handle,
        "fetch_metadata",
        "Reading Matrix username and IMAP connection ids",
        async { fetch_matrix_username(&state, user_id) },
    )
    .await?;
    let imap_connection_ids = step(
        &state_handle,
        "fetch_imap",
        "Enumerating IMAP connections",
        async { fetch_imap_connection_ids(&state, user_id) },
    )
    .await?;

    step(
        &state_handle,
        "bridge_logout",
        "Logging out of connected bridges (WhatsApp, Signal, Telegram)",
        async {
            logout_all_bridges(&state, user_id).await;
            Ok::<(), anyhow::Error>(())
        },
    )
    .await?;

    step(
        &state_handle,
        "shutdown_tasks",
        "Stopping background tasks (Matrix sync, Tesla monitors, IMAP IDLE)",
        async {
            shutdown_background_tasks(&state, user_id, &imap_connection_ids).await;
            Ok::<(), anyhow::Error>(())
        },
    )
    .await?;

    step(
        &state_handle,
        "delete_db",
        "Deleting all user-owned database rows (25 tables)",
        async { cascade_delete_user_data(&state.pg_pool, user_id) },
    )
    .await?;

    if let Some(username) = matrix_username {
        step(
            &state_handle,
            "delete_matrix_store",
            "Removing Matrix session store from disk",
            async move { remove_matrix_store(&username).await },
        )
        .await?;
    } else {
        skip_step(
            &state_handle,
            "delete_matrix_store",
            "Removing Matrix session store from disk",
            "No Matrix account on file",
        )
        .await;
    }

    {
        let mut s = state_handle.lock().await;
        s.complete = true;
    }

    tracing::info!("Data purge complete for user {}", user_id);
    Ok(())
}

/// Run one step of the purge: push a Running entry, run the future, then
/// update the entry to Done / Failed based on the result. On failure the
/// error is also stored on the top-level PurgeState so the frontend can
/// show a summary banner.
async fn step<F, T>(
    handle: &Arc<Mutex<PurgeState>>,
    id: &'static str,
    label: &'static str,
    fut: F,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    {
        let mut s = handle.lock().await;
        s.steps.push(PurgeStep {
            id,
            label,
            status: StepStatus::Running,
            detail: None,
        });
    }
    let result = fut.await;
    let mut s = handle.lock().await;
    let last = s.steps.last_mut().expect("step was just pushed");
    match &result {
        Ok(_) => {
            last.status = StepStatus::Done;
        }
        Err(e) => {
            last.status = StepStatus::Failed;
            last.detail = Some(e.to_string());
            s.error = Some(format!("{}: {}", label, e));
        }
    }
    result
}

async fn skip_step(
    handle: &Arc<Mutex<PurgeState>>,
    id: &'static str,
    label: &'static str,
    detail: &str,
) {
    let mut s = handle.lock().await;
    s.steps.push(PurgeStep {
        id,
        label,
        status: StepStatus::Skipped,
        detail: Some(detail.to_string()),
    });
}

/// Delete every user-owned database row in a single transaction.
///
/// Pub because `user_core::delete_user` reuses this for full account
/// deletion (it then deletes the `users` row itself after this returns).
///
/// Intentionally NOT deleted: `users`, `webauthn_credentials`,
/// `webauthn_challenges`, `totp_secrets`, `totp_backup_codes`. Those are
/// login credentials, not user data. Purging keeps the user's ability
/// to sign in.
pub fn cascade_delete_user_data(pg_pool: &PgDbPool, user_id: i32) -> Result<()> {
    use pg_schema::*;
    let mut conn = pg_pool
        .get()
        .map_err(|e| anyhow!("failed to get DB connection: {}", e))?;

    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        diesel::delete(bridges::table.filter(bridges::user_id.eq(user_id))).execute(conn)?;
        diesel::delete(
            bridge_bandwidth_logs::table.filter(bridge_bandwidth_logs::user_id.eq(user_id)),
        )
        .execute(conn)?;
        diesel::delete(
            bridge_disconnection_events::table
                .filter(bridge_disconnection_events::user_id.eq(user_id)),
        )
        .execute(conn)?;
        diesel::delete(imap_connection::table.filter(imap_connection::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(items::table.filter(items::user_id.eq(user_id))).execute(conn)?;
        diesel::delete(llm_usage_logs::table.filter(llm_usage_logs::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(mcp_servers::table.filter(mcp_servers::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(message_history::table.filter(message_history::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(message_status_log::table.filter(message_status_log::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(ont_changelog::table.filter(ont_changelog::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(ont_channels::table.filter(ont_channels::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(ont_events::table.filter(ont_events::user_id.eq(user_id))).execute(conn)?;
        diesel::delete(ont_links::table.filter(ont_links::user_id.eq(user_id))).execute(conn)?;
        diesel::delete(ont_messages::table.filter(ont_messages::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(ont_person_edits::table.filter(ont_person_edits::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(ont_persons::table.filter(ont_persons::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(ont_rules::table.filter(ont_rules::user_id.eq(user_id))).execute(conn)?;
        diesel::delete(processed_emails::table.filter(processed_emails::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(refund_info::table.filter(refund_info::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(tesla::table.filter(tesla::user_id.eq(user_id))).execute(conn)?;
        diesel::delete(usage_logs::table.filter(usage_logs::user_id.eq(user_id))).execute(conn)?;
        diesel::delete(user_info::table.filter(user_info::user_id.eq(user_id))).execute(conn)?;
        diesel::delete(user_secrets::table.filter(user_secrets::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .execute(conn)?;
        diesel::delete(youtube::table.filter(youtube::user_id.eq(user_id))).execute(conn)?;
        Ok(())
    })
    .map_err(|e| anyhow!("cascade delete failed: {}", e))
}

fn fetch_matrix_username(state: &Arc<AppState>, user_id: i32) -> Result<Option<String>> {
    use pg_schema::user_secrets;
    let mut conn = state
        .pg_pool
        .get()
        .map_err(|e| anyhow!("failed to get DB connection: {}", e))?;
    let username: Option<Option<String>> = user_secrets::table
        .filter(user_secrets::user_id.eq(user_id))
        .select(user_secrets::matrix_username)
        .first(&mut conn)
        .optional()
        .map_err(|e| anyhow!("failed to fetch matrix_username: {}", e))?;
    Ok(username.flatten())
}

fn fetch_imap_connection_ids(state: &Arc<AppState>, user_id: i32) -> Result<Vec<i32>> {
    use pg_schema::imap_connection;
    let mut conn = state
        .pg_pool
        .get()
        .map_err(|e| anyhow!("failed to get DB connection: {}", e))?;
    imap_connection::table
        .filter(imap_connection::user_id.eq(user_id))
        .select(imap_connection::id)
        .load(&mut conn)
        .map_err(|e| anyhow!("failed to fetch imap_connection ids: {}", e))
}

fn fetch_user_bridges(state: &Arc<AppState>, user_id: i32) -> Result<Vec<BridgeEntry>> {
    use pg_schema::bridges;
    let mut conn = state
        .pg_pool
        .get()
        .map_err(|e| anyhow!("failed to get DB connection: {}", e))?;
    let rows: Vec<(String, Option<String>)> = bridges::table
        .filter(bridges::user_id.eq(user_id))
        .select((bridges::bridge_type, bridges::room_id))
        .load(&mut conn)
        .map_err(|e| anyhow!("failed to fetch bridges: {}", e))?;
    Ok(rows
        .into_iter()
        .map(|(bridge_type, room_id)| BridgeEntry {
            bridge_type,
            room_id,
        })
        .collect())
}

/// Send logout commands to each connected bridge. Best-effort: any
/// individual bridge failing is logged and ignored so a stuck bridge
/// cannot block the rest of the purge.
async fn logout_all_bridges(state: &Arc<AppState>, user_id: i32) {
    let bridges = match fetch_user_bridges(state, user_id) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("purge: failed to list bridges for user {}: {}", user_id, e);
            return;
        }
    };
    if bridges.is_empty() {
        return;
    }

    let client = match crate::utils::matrix_auth::get_cached_client(user_id, state).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                "purge: Matrix client unavailable for user {} — skipping bridge logouts: {}",
                user_id,
                e
            );
            return;
        }
    };

    for bridge in bridges {
        let Some(room_id_str) = bridge.room_id else {
            continue;
        };
        let Ok(room_id) = OwnedRoomId::try_from(room_id_str.as_str()) else {
            continue;
        };
        let Some(room) = client.get_room(&room_id) else {
            continue;
        };
        let bridge_type = bridge.bridge_type;
        let result = timeout(
            BRIDGE_LOGOUT_TIMEOUT,
            logout_single_bridge(&client, &room, &bridge_type),
        )
        .await;
        match result {
            Ok(Ok(())) => tracing::info!(
                "purge: bridge {} logged out for user {}",
                bridge_type,
                user_id
            ),
            Ok(Err(e)) => tracing::warn!(
                "purge: bridge {} logout failed for user {}: {}",
                bridge_type,
                user_id,
                e
            ),
            Err(_) => tracing::warn!(
                "purge: bridge {} logout timed out for user {}",
                bridge_type,
                user_id
            ),
        }
    }
}

async fn logout_single_bridge(
    client: &matrix_sdk::Client,
    room: &matrix_sdk::Room,
    bridge_type: &str,
) -> Result<()> {
    match bridge_type {
        "whatsapp" => {
            let bot_env = std::env::var("WHATSAPP_BRIDGE_BOT")
                .map_err(|_| anyhow!("WHATSAPP_BRIDGE_BOT not set"))?;
            let bot_user_id = matrix_sdk::ruma::OwnedUserId::try_from(bot_env.as_str())
                .map_err(|e| anyhow!("invalid WHATSAPP_BRIDGE_BOT user id: {}", e))?;
            crate::utils::bridge::logout_all_bridgev2_logins(client, room, &bot_user_id, "!wa")
                .await?;
        }
        "signal" => {
            let bot_env = std::env::var("SIGNAL_BRIDGE_BOT")
                .map_err(|_| anyhow!("SIGNAL_BRIDGE_BOT not set"))?;
            let bot_user_id = matrix_sdk::ruma::OwnedUserId::try_from(bot_env.as_str())
                .map_err(|e| anyhow!("invalid SIGNAL_BRIDGE_BOT user id: {}", e))?;
            crate::utils::bridge::logout_all_bridgev2_logins(client, room, &bot_user_id, "!signal")
                .await?;
        }
        "telegram" => {
            // mautrix-telegram uses a plain "logout" command in the
            // management room rather than bridgev2 list-logins.
            room.send(RoomMessageEventContent::text_plain("logout"))
                .await?;
            sleep(Duration::from_secs(2)).await;
        }
        _ => {
            // Unknown or non-Matrix bridge type (e.g. "sms"). Nothing to log out.
        }
    }
    Ok(())
}

async fn shutdown_background_tasks(
    state: &Arc<AppState>,
    user_id: i32,
    imap_connection_ids: &[i32],
) {
    // Matrix client + sync task. Abort through the per-user cell to
    // respect the same lock discipline as ensure_matrix_user_running, then
    // remove the cell entry entirely — unlike the bridge-disconnect path,
    // no concurrent caller needs to reinstall a client.
    if let Some(cell_arc) = state.matrix_users.get(&user_id).map(|r| r.clone()) {
        let mut slot = cell_arc.lock().await;
        if let Some(old) = slot.take() {
            old.sync_task.abort();
        }
    }
    state.matrix_users.remove(&user_id);

    if let Some((_, task)) = state.tesla_monitoring_tasks.remove(&user_id) {
        task.abort();
    }
    if let Some((_, task)) = state.tesla_charging_monitor_tasks.remove(&user_id) {
        task.abort();
    }

    // IMAP IDLE tasks are keyed by imap_connection.id, not user_id.
    for conn_id in imap_connection_ids {
        if let Some((_, task)) = state.imap_idle_tasks.remove(conn_id) {
            task.abort();
        }
    }

    {
        let mut lock = state.pending_message_senders.lock().await;
        lock.remove(&user_id);
    }

    state.digest_cooldowns.remove(&user_id);
    state
        .system_notify_cooldowns
        .retain(|key, _| key.0 != user_id);
}

async fn remove_matrix_store(matrix_username: &str) -> Result<()> {
    let persistent_store_path = std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
        .map_err(|_| anyhow!("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"))?;
    let store_path = format!("{}/{}", persistent_store_path, matrix_username);
    if Path::new(&store_path).exists() {
        fs::remove_dir_all(&store_path).await?;
        tracing::info!("purge: removed Matrix store at {}", store_path);
    }
    Ok(())
}
