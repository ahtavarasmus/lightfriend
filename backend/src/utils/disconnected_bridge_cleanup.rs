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
use tokio::process::Command;

const DEFAULT_POLL_SECS: u64 = 30;
const DEFAULT_GRACE_SECS: i32 = 120;
const DEFAULT_MAX_ATTEMPTS: i32 = 5;
const DEFAULT_RETRY_SECS: i32 = 300;
const DEFAULT_ORPHAN_SCAN_SECS: i32 = 3600;
const DEFAULT_BATCH_SIZE: i64 = 5;
const DEFAULT_ROOM_DELETE_LIMIT: i64 = 1;
const DEFAULT_ADMIN_USER_ID: i32 = 1;
const HTTP_TIMEOUT_SECS: u64 = 20;
const CONNECTION_LEASE_SECS: i32 = 300;

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
    room_delete_limit: i64,
}

#[derive(Deserialize)]
struct RoomMembersResponse {
    members: Vec<String>,
}

#[derive(Debug, Default)]
struct StorageSnapshot {
    rootfs_free_bytes: Option<i64>,
    tuwunel_bytes: Option<i64>,
}

pub struct BridgeConnectionLease {
    state: Arc<AppState>,
    user_id: i32,
    bridge_type: String,
    owner_token: String,
}

impl Drop for BridgeConnectionLease {
    fn drop(&mut self) {
        if let Err(error) = self
            .state
            .tuwunel_cleanup_repository
            .release_bridge_connection_lease(self.user_id, &self.bridge_type, &self.owner_token)
        {
            tracing::error!(
                user_id = self.user_id,
                service = self.bridge_type,
                error = %error,
                "Failed to release bridge connection lease"
            );
        }
    }
}

pub fn acquire_bridge_reconnect_lease(
    state: &Arc<AppState>,
    user_id: i32,
    bridge_type: &str,
) -> Result<BridgeConnectionLease> {
    let now = now_timestamp();
    let owner_token = format!("reconnect-{}", uuid::Uuid::new_v4());
    let acquired = state
        .tuwunel_cleanup_repository
        .try_acquire_bridge_connection_lease(
            user_id,
            bridge_type,
            "reconnect",
            &owner_token,
            now.saturating_add(CONNECTION_LEASE_SECS),
        )?;
    if !acquired {
        return Err(anyhow!(
            "{} cleanup is currently deleting old Matrix rooms; retry connection shortly",
            bridge_type
        ));
    }
    Ok(BridgeConnectionLease {
        state: state.clone(),
        user_id,
        bridge_type: bridge_type.to_string(),
        owner_token,
    })
}

pub fn acquire_bridge_portal_cleanup_lease(
    state: &Arc<AppState>,
    user_id: i32,
    bridge_type: &str,
    job_id: i32,
) -> Result<BridgeConnectionLease> {
    let now = now_timestamp();
    let owner_token = format!("portal-cleanup-job-{job_id}-{}", uuid::Uuid::new_v4());
    let acquired = state
        .tuwunel_cleanup_repository
        .try_acquire_bridge_connection_lease(
            user_id,
            bridge_type,
            "portal_cleanup",
            &owner_token,
            now.saturating_add(CONNECTION_LEASE_SECS),
        )?;
    if !acquired {
        return Err(anyhow!(
            "{} connection operation is already in progress; retry disconnect shortly",
            bridge_type
        ));
    }
    Ok(BridgeConnectionLease {
        state: state.clone(),
        user_id,
        bridge_type: bridge_type.to_string(),
        owner_token,
    })
}

pub async fn record_portal_cleanup_outcome(
    state: &Arc<AppState>,
    job_id: i32,
    user_id: i32,
    bridge_type: &str,
    client: &matrix_sdk::Client,
    mut errors: Vec<String>,
) {
    let mut remaining_rooms = Vec::new();
    for attempt in 1..=3 {
        let sync_settings =
            matrix_sdk::config::SyncSettings::default().timeout(Duration::from_secs(2));
        if let Err(error) = client.sync_once(sync_settings).await {
            tracing::warn!(job_id, user_id, service = bridge_type, attempt, error = %error, "Portal cleanup verification sync failed");
        }
        match crate::utils::bridge::get_service_rooms(client, bridge_type).await {
            Ok(rooms) if rooms.is_empty() => {
                remaining_rooms.clear();
                break;
            }
            Ok(rooms) => {
                remaining_rooms = rooms.into_iter().map(|room| room.room_id).collect();
            }
            Err(error) => errors.push(format!("portal verification failed: {error}")),
        }
        if attempt < 3 {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
    if !remaining_rooms.is_empty() {
        errors.push(format!(
            "{} service portal room(s) still visible: {}",
            remaining_rooms.len(),
            remaining_rooms.join(",")
        ));
    }

    let confirmed = errors.is_empty();
    let summary = (!confirmed).then(|| errors.join("; "));
    if let Err(error) = state.tuwunel_cleanup_repository.mark_bridge_portal_cleanup(
        job_id,
        confirmed,
        summary.as_deref(),
    ) {
        tracing::error!(job_id, user_id, service = bridge_type, error = %error, "Failed to persist bridge portal cleanup proof");
    } else if confirmed {
        tracing::info!(
            job_id,
            user_id,
            service = bridge_type,
            "Confirmed bridge portal cleanup; Tuwunel room deletion may proceed"
        );
    } else {
        tracing::error!(
            job_id,
            user_id,
            service = bridge_type,
            reason = summary.as_deref().unwrap_or("unknown"),
            "Bridge portal cleanup was not confirmed; Tuwunel room deletion remains blocked"
        );
    }
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
        room_delete_limit = config.room_delete_limit,
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
    let mut remaining_room_deletes = config.room_delete_limit;
    for job in jobs {
        process_job(
            state,
            config,
            &client,
            &access_token,
            &job,
            &mut remaining_room_deletes,
        )
        .await;
    }
    Ok(())
}

async fn process_job(
    state: &Arc<AppState>,
    config: &Config,
    client: &reqwest::Client,
    access_token: &str,
    job: &BridgeCleanupJob,
    remaining_room_deletes: &mut i64,
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

    let execute_requested = config.execute_enabled
        && (job.trigger_kind != "orphan_audit" || config.orphan_execute_enabled);
    let execute = bridge_cleanup_execution_allowed(
        &job.trigger_kind,
        &job.portal_cleanup_status,
        config.execute_enabled,
        config.orphan_execute_enabled,
    );
    let mut cleanup_lease = None;
    if execute && *remaining_room_deletes > 0 {
        let owner_token = format!("cleanup-job-{}-{}", job.id, uuid::Uuid::new_v4());
        match state
            .tuwunel_cleanup_repository
            .try_acquire_bridge_connection_lease(
                job.user_id,
                &job.bridge_type,
                "cleanup",
                &owner_token,
                now.saturating_add(CONNECTION_LEASE_SECS),
            ) {
            Ok(true) => {
                cleanup_lease = Some(BridgeConnectionLease {
                    state: state.clone(),
                    user_id: job.user_id,
                    bridge_type: job.bridge_type.clone(),
                    owner_token,
                });
            }
            Ok(false) => {
                retry_job(
                    state,
                    config,
                    job,
                    "reconnect operation currently holds bridge lease",
                );
                return;
            }
            Err(error) => {
                retry_job(
                    state,
                    config,
                    job,
                    &format!("cleanup lease acquisition failed: {error}"),
                );
                return;
            }
        }

        match state
            .tuwunel_cleanup_repository
            .bridge_generation_present(job)
        {
            Ok(None) => {}
            Ok(Some((current_id, current_status, current_created_at))) => {
                let reason = format!(
                    "cancelled after cleanup lease: bridge id={current_id} created_at={current_created_at:?} status={current_status}"
                );
                let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_job(
                    job.id,
                    BRIDGE_JOB_CANCELLED_RECONNECTED,
                    job.attempt_count,
                    now,
                    Some(&reason),
                    true,
                );
                return;
            }
            Err(error) => {
                retry_job(
                    state,
                    config,
                    job,
                    &format!("post-lease bridge generation check failed: {error}"),
                );
                return;
            }
        }

        let snapshot = storage_snapshot().await;
        let _ = state
            .tuwunel_cleanup_repository
            .record_bridge_cleanup_storage(
                job.id,
                true,
                snapshot.rootfs_free_bytes,
                snapshot.tuwunel_bytes,
            );
        tracing::info!(
            job_id = job.id,
            rootfs_free_bytes = snapshot.rootfs_free_bytes,
            tuwunel_bytes = snapshot.tuwunel_bytes,
            "Captured pre-delete bridge cleanup storage"
        );
    }
    let mut blocked = 0;
    let mut failed = 0;
    let mut deleted = 0;
    let mut deferred = 0;
    for room in rooms {
        match audit_room(state, config, client, access_token, job, &room).await {
            Ok(()) if !execute => {
                let reason = if execute_requested {
                    format!(
                        "blocked: portal cleanup status={} (confirmed required)",
                        job.portal_cleanup_status
                    )
                } else {
                    "eligible; destructive execution disabled by policy".to_string()
                };
                let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_room(
                    room.id,
                    BRIDGE_ROOM_AUDIT_READY,
                    room.attempt_count,
                    None,
                    Some(&reason),
                    false,
                );
            }
            Ok(()) if *remaining_room_deletes <= 0 => {
                deferred += 1;
                let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_room(
                    room.id,
                    BRIDGE_ROOM_AUDIT_READY,
                    room.attempt_count,
                    None,
                    Some("eligible; deferred by per-cycle destructive canary limit"),
                    false,
                );
            }
            Ok(()) => match delete_room(state, config, client, access_token, job, &room).await {
                Ok(()) => {
                    deleted += 1;
                    *remaining_room_deletes = (*remaining_room_deletes).saturating_sub(1);
                }
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

    if deleted > 0 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let snapshot = storage_snapshot().await;
        let _ = state
            .tuwunel_cleanup_repository
            .record_bridge_cleanup_storage(
                job.id,
                false,
                snapshot.rootfs_free_bytes,
                snapshot.tuwunel_bytes,
            );
        tracing::info!(
            job_id = job.id,
            rootfs_free_bytes = snapshot.rootfs_free_bytes,
            tuwunel_bytes = snapshot.tuwunel_bytes,
            "Captured post-delete bridge cleanup storage"
        );
    }
    let summary = format!(
        "rooms_deleted={deleted} rooms_blocked={blocked} rooms_failed={failed} rooms_deferred={deferred} execute={execute} portal_cleanup_status={} canary_limit={}",
        job.portal_cleanup_status, config.room_delete_limit
    );
    if failed > 0 {
        retry_job(state, config, job, &summary);
    } else if execute && deferred == 0 {
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
    } else if execute {
        let _ = state.tuwunel_cleanup_repository.update_bridge_cleanup_job(
            job.id,
            BRIDGE_JOB_AUDIT_READY,
            job.attempt_count,
            now.saturating_add(config.retry_secs),
            Some(&summary),
            false,
        );
        tracing::info!(
            job_id = job.id,
            user_id = job.user_id,
            service = job.bridge_type,
            summary,
            "Disconnected-bridge cleanup paused at destructive canary limit"
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
    drop(cleanup_lease);
}

async fn storage_snapshot() -> StorageSnapshot {
    let rootfs_free_bytes = command_metric("df", &["-B1", "/"], |output| {
        output
            .lines()
            .last()?
            .split_whitespace()
            .nth(3)?
            .parse::<i64>()
            .ok()
    })
    .await;
    let tuwunel_bytes = command_metric("du", &["-sb", "/var/lib/tuwunel"], |output| {
        output.split_whitespace().next()?.parse::<i64>().ok()
    })
    .await;
    StorageSnapshot {
        rootfs_free_bytes,
        tuwunel_bytes,
    }
}

async fn command_metric(
    program: &str,
    args: &[&str],
    parse: impl FnOnce(&str) -> Option<i64>,
) -> Option<i64> {
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        Command::new(program).args(args).output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    parse(std::str::from_utf8(&output.stdout).ok()?)
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

pub fn bridge_cleanup_execution_allowed(
    trigger_kind: &str,
    portal_cleanup_status: &str,
    execute_enabled: bool,
    orphan_execute_enabled: bool,
) -> bool {
    execute_enabled
        && portal_cleanup_status == "confirmed"
        && (trigger_kind != "orphan_audit" || orphan_execute_enabled)
}

impl Config {
    fn from_env() -> Self {
        Self {
            homeserver_url: std::env::var("MATRIX_HOMESERVER")
                .unwrap_or_else(|_| "http://localhost:8008".to_string()),
            admin_user_id: env_i32("TUWUNEL_ADMIN_USER_ID", DEFAULT_ADMIN_USER_ID, 1),
            audit_enabled: env_flag("TUWUNEL_DISCONNECTED_BRIDGE_PURGE_AUDIT_ENABLED", true),
            execute_enabled: env_flag("TUWUNEL_DISCONNECTED_BRIDGE_PURGE_ENABLED", true),
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
            room_delete_limit: i64::from(
                env_i32(
                    "TUWUNEL_DISCONNECTED_BRIDGE_PURGE_ROOM_LIMIT",
                    DEFAULT_ROOM_DELETE_LIMIT as i32,
                    1,
                )
                .min(10),
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
