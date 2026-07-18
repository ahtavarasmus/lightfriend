use crate::repositories::tuwunel_cleanup_repository::{
    now_timestamp, BridgeCleanupJob, BridgeCleanupRoom, BRIDGE_JOB_AUDIT_READY,
    BRIDGE_JOB_CANCELLED_RECONNECTED, BRIDGE_JOB_EXHAUSTED, BRIDGE_JOB_RETRYING,
    BRIDGE_JOB_SUCCEEDED, BRIDGE_ROOM_AUDIT_READY, BRIDGE_ROOM_BLOCKED_ACTIVE,
    BRIDGE_ROOM_DELETING, BRIDGE_ROOM_EXHAUSTED, BRIDGE_ROOM_RETRYING, BRIDGE_ROOM_SUCCEEDED,
};
use crate::AppState;
use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_POLL_SECS: u64 = 30;
const DEFAULT_GRACE_SECS: i32 = 120;
const DEFAULT_MAX_ATTEMPTS: i32 = 5;
const DEFAULT_RETRY_SECS: i32 = 300;
const DEFAULT_ORPHAN_SCAN_SECS: i32 = 3600;
const DEFAULT_BATCH_SIZE: i64 = 5;
const DEFAULT_ADMIN_USER_ID: i32 = 1;
const HTTP_TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Clone)]
struct Config {
    homeserver_url: String,
    admin_user_id: i32,
    audit_enabled: bool,
    execute_enabled: bool,
    orphan_execute_enabled: bool,
    poll_secs: u64,
    grace_secs: i32,
    max_attempts: i32,
    retry_secs: i32,
    orphan_scan_secs: i32,
    batch_size: i64,
}

#[derive(Deserialize)]
struct RoomMembersResponse {
    members: Vec<String>,
}

/// Queue cleanup while the exact bridge generation still exists. Callers must
/// do this before deleting the bridge row, otherwise a reconnect cannot be
/// distinguished from the disconnected session.
pub fn enqueue_disconnect_cleanup(
    state: &Arc<AppState>,
    bridge: &crate::pg_models::PgBridge,
    trigger_kind: &str,
) -> Result<i32> {
    let grace_secs = env_i32(
        "TUWUNEL_DISCONNECTED_BRIDGE_PURGE_GRACE_SECS",
        DEFAULT_GRACE_SECS,
        30,
    );
    let job_id = state.tuwunel_cleanup_repository.enqueue_bridge_cleanup(
        bridge,
        trigger_kind,
        grace_secs,
    )?;
    tracing::info!(
        job_id,
        user_id = bridge.user_id,
        service = bridge.bridge_type,
        bridge_id = bridge.id,
        bridge_created_at = bridge.created_at,
        trigger_kind,
        grace_secs,
        "Queued durable disconnected-bridge Tuwunel cleanup"
    );
    Ok(job_id)
}

pub async fn start_disconnected_bridge_cleanup_worker(state: Arc<AppState>) {
    let config = Config::from_env();
    tracing::info!(
        audit_enabled = config.audit_enabled,
        execute_enabled = config.execute_enabled,
        orphan_execute_enabled = config.orphan_execute_enabled,
        poll_secs = config.poll_secs,
        grace_secs = config.grace_secs,
        max_attempts = config.max_attempts,
        orphan_scan_secs = config.orphan_scan_secs,
        batch_size = config.batch_size,
        "Disconnected-bridge Tuwunel cleanup policy loaded"
    );
    let mut next_orphan_scan_at = 0;
    loop {
        let now = now_timestamp();
        if config.audit_enabled && now >= next_orphan_scan_at {
            match state
                .tuwunel_cleanup_repository
                .enqueue_orphan_bridge_cleanup_audits(config.batch_size, config.grace_secs)
            {
                Ok(enqueued) if enqueued > 0 => tracing::info!(
                    enqueued,
                    "Queued audit-only cleanup jobs for orphaned bridge history"
                ),
                Ok(_) => {}
                Err(error) => tracing::error!(
                    error = %error,
                    "Failed to discover orphaned bridge cleanup candidates"
                ),
            }
            next_orphan_scan_at = now.saturating_add(config.orphan_scan_secs);
        }

        if let Err(error) = run_cycle(&state, &config).await {
            tracing::error!(error = %error, "Disconnected-bridge cleanup cycle failed");
        }
        tokio::time::sleep(Duration::from_secs(config.poll_secs)).await;
    }
}

async fn run_cycle(state: &Arc<AppState>, config: &Config) -> Result<()> {
    if !config.audit_enabled && !config.execute_enabled {
        return Ok(());
    }
    let jobs = state
        .tuwunel_cleanup_repository
        .list_due_bridge_cleanup_jobs(now_timestamp(), config.batch_size)?;
    if jobs.is_empty() {
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()?;
    let access_token = admin_access_token(state, config.admin_user_id).await?;
    for job in jobs {
        process_job(state, config, &client, &access_token, &job).await;
    }
    Ok(())
}

async fn process_job(
    state: &Arc<AppState>,
    config: &Config,
    client: &reqwest::Client,
    access_token: &str,
    job: &BridgeCleanupJob,
) {
    let now = now_timestamp();
    match state
        .tuwunel_cleanup_repository
        .bridge_generation_present(job)
    {
        Ok(Some((current_id, current_status, current_created_at))) => {
            let same_generation = job.expected_bridge_id == Some(current_id)
                && job.expected_bridge_created_at == current_created_at;
            if same_generation {
                let reason =
                    format!("expected bridge generation still present status={current_status}");
                let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_job(
                    job.id,
                    BRIDGE_JOB_RETRYING,
                    job.attempt_count,
                    now.saturating_add(config.retry_secs),
                    Some(&reason),
                    false,
                );
            } else {
                let reason = format!(
                    "cancelled: newer bridge generation id={current_id} created_at={current_created_at:?} status={current_status}"
                );
                let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_job(
                    job.id,
                    BRIDGE_JOB_CANCELLED_RECONNECTED,
                    job.attempt_count,
                    now,
                    Some(&reason),
                    true,
                );
                tracing::warn!(
                    job_id = job.id,
                    user_id = job.user_id,
                    service = job.bridge_type,
                    reason,
                    "Cancelled disconnected-bridge cleanup after reconnect"
                );
            }
            return;
        }
        Ok(None) => {}
        Err(error) => {
            retry_job(
                state,
                config,
                job,
                &format!("bridge generation check failed: {error}"),
            );
            return;
        }
    }

    discover_visible_service_rooms(state, job).await;

    let rooms = match state
        .tuwunel_cleanup_repository
        .list_bridge_cleanup_rooms(job.id)
    {
        Ok(rooms) => rooms,
        Err(error) => {
            retry_job(
                state,
                config,
                job,
                &format!("room candidate load failed: {error}"),
            );
            return;
        }
    };

    let execute = config.execute_enabled
        && (job.trigger_kind != "orphan_audit" || config.orphan_execute_enabled);
    let mut blocked = 0;
    let mut failed = 0;
    let mut deleted = 0;
    for room in rooms {
        match audit_room(state, config, client, access_token, job, &room).await {
            Ok(()) if !execute => {
                let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_room(
                    room.id,
                    BRIDGE_ROOM_AUDIT_READY,
                    room.attempt_count,
                    None,
                    Some("eligible; destructive execution disabled by policy"),
                    false,
                );
            }
            Ok(()) => match delete_room(state, config, client, access_token, job, &room).await {
                Ok(()) => deleted += 1,
                Err(()) => failed += 1,
            },
            Err(reason) => {
                blocked += 1;
                let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_room(
                    room.id,
                    BRIDGE_ROOM_BLOCKED_ACTIVE,
                    room.attempt_count,
                    None,
                    Some(&reason),
                    true,
                );
                tracing::warn!(
                    job_id = job.id,
                    room_id = room.room_id,
                    source = room.source,
                    reason,
                    "Blocked disconnected-bridge room deletion"
                );
            }
        }
    }

    let summary = format!(
        "rooms_deleted={deleted} rooms_blocked={blocked} rooms_failed={failed} execute={execute}"
    );
    if failed > 0 {
        retry_job(state, config, job, &summary);
    } else if execute {
        let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_job(
            job.id,
            BRIDGE_JOB_SUCCEEDED,
            job.attempt_count,
            now,
            Some(&summary),
            true,
        );
        tracing::info!(
            job_id = job.id,
            user_id = job.user_id,
            service = job.bridge_type,
            summary,
            "Disconnected-bridge Tuwunel cleanup completed"
        );
    } else {
        let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_job(
            job.id,
            BRIDGE_JOB_AUDIT_READY,
            job.attempt_count,
            now.saturating_add(config.orphan_scan_secs),
            Some(&summary),
            false,
        );
        tracing::info!(
            job_id = job.id,
            user_id = job.user_id,
            service = job.bridge_type,
            summary,
            "Disconnected-bridge cleanup audit completed"
        );
    }
}

async fn discover_visible_service_rooms(state: &Arc<AppState>, job: &BridgeCleanupJob) {
    let discovery = tokio::time::timeout(Duration::from_secs(15), async {
        let matrix_client =
            crate::utils::matrix_auth::get_cached_client(job.user_id, state).await?;
        crate::utils::bridge::get_service_rooms(&matrix_client, &job.bridge_type).await
    })
    .await;
    match discovery {
        Ok(Ok(rooms)) => {
            let room_ids: Vec<String> = rooms.into_iter().map(|room| room.room_id).collect();
            match state.tuwunel_cleanup_repository.add_bridge_cleanup_rooms(
                job.id,
                &room_ids,
                "matrix_visibility",
            ) {
                Ok(inserted) if inserted > 0 => tracing::info!(
                    job_id = job.id,
                    inserted,
                    "Added Matrix-visible rooms to disconnected-bridge cleanup"
                ),
                Ok(_) => {}
                Err(error) => tracing::warn!(
                    job_id = job.id,
                    error = %error,
                    "Could not persist Matrix-visible bridge rooms"
                ),
            }
        }
        Ok(Err(error)) => tracing::warn!(
            job_id = job.id,
            error = %error,
            "Matrix-visible bridge-room discovery failed; ontology snapshot retained"
        ),
        Err(_) => tracing::warn!(
            job_id = job.id,
            "Matrix-visible bridge-room discovery timed out; ontology snapshot retained"
        ),
    }
}

async fn audit_room(
    state: &Arc<AppState>,
    config: &Config,
    client: &reqwest::Client,
    access_token: &str,
    job: &BridgeCleanupJob,
    room: &BridgeCleanupRoom,
) -> std::result::Result<(), String> {
    let ontology_owners = state
        .tuwunel_cleanup_repository
        .active_ontology_room_owners(&room.room_id, &job.bridge_type, job.user_id)
        .map_err(|error| format!("active ontology owner check failed: {error}"))?;
    if ontology_owners > 0 {
        return Err(format!(
            "shared room has {ontology_owners} other active ontology owner(s)"
        ));
    }

    let response = client
        .get(build_room_members_url(
            &config.homeserver_url,
            &room.room_id,
        ))
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|error| format!("admin room-members request failed: {error}"))?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(());
    }
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("admin room-members body failed: {error}"))?;
    if !status.is_success() {
        return Err(format!("admin room-members returned {status}: {body}"));
    }
    let members: RoomMembersResponse = serde_json::from_str(&body)
        .map_err(|error| format!("admin room-members invalid JSON: {error}; body={body}"))?;
    let localparts: Vec<String> = members
        .members
        .iter()
        .filter_map(|member| member.strip_prefix('@'))
        .filter_map(|member| {
            member
                .split_once(':')
                .map(|(localpart, _)| localpart.to_string())
        })
        .collect();
    let active_members = state
        .tuwunel_cleanup_repository
        .active_bridge_members(&localparts, &job.bridge_type, job.user_id)
        .map_err(|error| format!("active Matrix member check failed: {error}"))?;
    if active_members > 0 {
        return Err(format!(
            "shared room has {active_members} other active Matrix member(s)"
        ));
    }
    Ok(())
}

async fn delete_room(
    state: &Arc<AppState>,
    config: &Config,
    client: &reqwest::Client,
    access_token: &str,
    job: &BridgeCleanupJob,
    room: &BridgeCleanupRoom,
) -> std::result::Result<(), ()> {
    let attempt = room.attempt_count.saturating_add(1);
    let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_room(
        room.id,
        BRIDGE_ROOM_DELETING,
        attempt,
        None,
        None,
        false,
    );
    let response = client
        .delete(build_delete_room_url(&config.homeserver_url, &room.room_id))
        .bearer_auth(access_token)
        .json(&json!({"block": false, "purge": true}))
        .send()
        .await;

    let result = match response {
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.is_success()
                || (status == StatusCode::NOT_FOUND
                    && body.to_ascii_lowercase().contains("room not found"))
            {
                Ok(())
            } else {
                Err(format!("delete room returned {status}: {body}"))
            }
        }
        Err(error) => Err(format!("delete room request failed: {error}")),
    };

    match result {
        Ok(()) => {
            let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_room(
                room.id,
                BRIDGE_ROOM_SUCCEEDED,
                attempt,
                None,
                None,
                true,
            );
            tracing::info!(
                job_id = job.id,
                room_id = room.room_id,
                source = room.source,
                attempt,
                "Deleted disconnected-bridge room from Tuwunel"
            );
            Ok(())
        }
        Err(error) => {
            let exhausted = attempt >= config.max_attempts;
            let status = if exhausted {
                BRIDGE_ROOM_EXHAUSTED
            } else {
                BRIDGE_ROOM_RETRYING
            };
            let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_room(
                room.id,
                status,
                attempt,
                None,
                Some(&error),
                exhausted,
            );
            tracing::error!(
                job_id = job.id,
                room_id = room.room_id,
                attempt,
                exhausted,
                error,
                "Disconnected-bridge room deletion failed"
            );
            Err(())
        }
    }
}

fn retry_job(state: &Arc<AppState>, config: &Config, job: &BridgeCleanupJob, error: &str) {
    let attempt = job.attempt_count.saturating_add(1);
    let exhausted = attempt >= config.max_attempts;
    let status = if exhausted {
        BRIDGE_JOB_EXHAUSTED
    } else {
        BRIDGE_JOB_RETRYING
    };
    let now = now_timestamp();
    let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_job(
        job.id,
        status,
        attempt,
        now.saturating_add(config.retry_secs),
        Some(error),
        exhausted,
    );
}

async fn admin_access_token(state: &Arc<AppState>, admin_user_id: i32) -> Result<String> {
    let client = crate::utils::matrix_auth::get_cached_client(admin_user_id, state)
        .await
        .map_err(|error| anyhow!("failed to load Matrix admin user {admin_user_id}: {error}"))?;
    let session = client
        .matrix_auth()
        .session()
        .ok_or_else(|| anyhow!("Matrix admin user {admin_user_id} has no active session"))?;
    Ok(session.tokens.access_token.clone())
}

pub fn build_room_members_url(homeserver_url: &str, room_id: &str) -> String {
    format!(
        "{}/_synapse/admin/v1/rooms/{}/members",
        homeserver_url.trim_end_matches('/'),
        urlencoding::encode(room_id)
    )
}

pub fn build_delete_room_url(homeserver_url: &str, room_id: &str) -> String {
    format!(
        "{}/_synapse/admin/v1/rooms/{}",
        homeserver_url.trim_end_matches('/'),
        urlencoding::encode(room_id)
    )
}

impl Config {
    fn from_env() -> Self {
        Self {
            homeserver_url: std::env::var("MATRIX_HOMESERVER")
                .unwrap_or_else(|_| "http://localhost:8008".to_string()),
            admin_user_id: env_i32("TUWUNEL_ADMIN_USER_ID", DEFAULT_ADMIN_USER_ID, 1),
            audit_enabled: env_flag("TUWUNEL_DISCONNECTED_BRIDGE_PURGE_AUDIT_ENABLED", true),
            execute_enabled: env_flag("TUWUNEL_DISCONNECTED_BRIDGE_PURGE_ENABLED", false),
            orphan_execute_enabled: env_flag(
                "TUWUNEL_DISCONNECTED_BRIDGE_ORPHAN_PURGE_ENABLED",
                false,
            ),
            poll_secs: env_u64(
                "TUWUNEL_DISCONNECTED_BRIDGE_PURGE_POLL_SECS",
                DEFAULT_POLL_SECS,
                5,
            ),
            grace_secs: env_i32(
                "TUWUNEL_DISCONNECTED_BRIDGE_PURGE_GRACE_SECS",
                DEFAULT_GRACE_SECS,
                30,
            ),
            max_attempts: env_i32(
                "TUWUNEL_DISCONNECTED_BRIDGE_PURGE_MAX_ATTEMPTS",
                DEFAULT_MAX_ATTEMPTS,
                1,
            ),
            retry_secs: env_i32(
                "TUWUNEL_DISCONNECTED_BRIDGE_PURGE_RETRY_SECS",
                DEFAULT_RETRY_SECS,
                30,
            ),
            orphan_scan_secs: env_i32(
                "TUWUNEL_DISCONNECTED_BRIDGE_ORPHAN_SCAN_SECS",
                DEFAULT_ORPHAN_SCAN_SECS,
                300,
            ),
            batch_size: i64::from(
                env_i32(
                    "TUWUNEL_DISCONNECTED_BRIDGE_PURGE_BATCH_SIZE",
                    DEFAULT_BATCH_SIZE as i32,
                    1,
                )
                .min(50),
            ),
        }
    }
}

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(default)
}

fn env_i32(name: &str, default: i32, minimum: i32) -> i32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
        .max(minimum)
}

fn env_u64(name: &str, default: u64, minimum: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
        .max(minimum)
}
