use crate::pg_models::TuwunelCleanupEvent;
use crate::repositories::tuwunel_cleanup_repository::{
    now_timestamp, HistoricalBackfillCandidate, RoomHistoryPurge,
};
use crate::AppState;
use anyhow::{anyhow, Context, Result};
use matrix_sdk::ruma::OwnedRoomAliasId;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fmt;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const DEFAULT_HOMESERVER_URL: &str = "http://localhost:8008";
const DEFAULT_ADMIN_USER_ID: i32 = 1;
const DEFAULT_RETENTION_SECS: u64 = 60;
const DEFAULT_POLL_SECS: u64 = 30;
const DEFAULT_MAX_ATTEMPTS: i32 = 5;
const DEFAULT_BATCH_SIZE: usize = 50;
const DEFAULT_BACKFILL_BATCH_SIZE: usize = 50;
const DEFAULT_BACKFILL_SCAN_SECS: u64 = 60;
const DEFAULT_BACKFILL_MIN_AGE_SECS: u64 = 60;
const DEFAULT_BACKFILL_AUDIT_RECHECK_SECS: u64 = 300;
const DEFAULT_BACKFILL_AUDIT_MAX_PAGES: usize = 100;
const DEFAULT_BACKFILL_AUDIT_PAGE_SIZE: u64 = 100;
const DEFAULT_PORTAL_CENSUS_SCAN_SECS: u64 = 300;
const DEFAULT_PORTAL_CENSUS_TARGET_BATCH_SIZE: usize = 5;
const DEFAULT_PORTAL_CENSUS_ROOM_BATCH_SIZE: usize = 100;
const DEFAULT_PORTAL_CENSUS_PURGE_BATCH_SIZE: usize = 20;
const DEFAULT_SERVER_CENSUS_SCAN_SECS: u64 = 21_600;
const DEFAULT_SERVER_CENSUS_PAGE_SIZE: usize = 5_000;
const DEFAULT_SERVER_CENSUS_MAX_PAGES: usize = 10;
const DEFAULT_SERVER_CENSUS_HTTP_TIMEOUT_SECS: u64 = 120;
const DEFAULT_PURGE_STATUS_POLL_BATCH_SIZE: usize = 100;
const DEFAULT_PURGE_MAX_IN_FLIGHT: usize = 4;
const DEFAULT_STALE_INGEST_SECS: u64 = 300;
const DEFAULT_EXHAUSTED_RETRY_SECS: u64 = 900;
const BLOCKER_LOG_INTERVAL_SECS: i64 = 600;
const HTTP_TIMEOUT_SECS: u64 = 15;
const SERVER_CENSUS_SERVICE: &str = "server_all_rooms";

static DISABLED_LOGGED: OnceLock<()> = OnceLock::new();
static DRY_RUN_LOGGED: OnceLock<()> = OnceLock::new();
static CONFIG_LOGGED: OnceLock<()> = OnceLock::new();
static LAST_BLOCKER_LOGGED_AT: AtomicI64 = AtomicI64::new(0);

#[derive(Debug, Clone)]
struct EventPurgeConfig {
    homeserver_url: String,
    admin_user_id: i32,
    enabled: bool,
    dry_run: bool,
    retention_secs: u64,
    poll_secs: u64,
    max_attempts: i32,
    batch_size: usize,
    backfill_enabled: bool,
    backfill_audit_enabled: bool,
    backfill_execute_verified_enabled: bool,
    backfill_execute_blocked_enabled: bool,
    backfill_batch_size: usize,
    backfill_scan_secs: u64,
    backfill_min_age_secs: u64,
    backfill_audit_recheck_secs: u64,
    backfill_audit_max_pages: usize,
    backfill_audit_page_size: u64,
    portal_census_enabled: bool,
    portal_census_scan_secs: u64,
    portal_census_target_batch_size: usize,
    portal_census_room_batch_size: usize,
    portal_census_purge_batch_size: usize,
    server_census_enabled: bool,
    server_census_scan_secs: u64,
    server_census_page_size: usize,
    server_census_max_pages: usize,
    server_census_http_timeout_secs: u64,
    purge_status_poll_batch_size: usize,
    purge_max_in_flight: usize,
    stale_ingest_secs: u64,
    exhausted_retry_secs: u64,
}

#[derive(Debug, Default)]
struct PurgeCycleOutcome {
    backfilled: usize,
    forced_backfilled: usize,
    audited: usize,
    census_targets: usize,
    census_rooms: usize,
    server_census_discovered_rooms: usize,
    server_census_recorded_rooms: usize,
    server_census_succeeded: bool,
}

#[derive(Debug, Deserialize)]
struct ServerRoomListResponse {
    rooms: Vec<ServerRoomSummary>,
    offset: usize,
    total_rooms: usize,
    next_batch: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ServerRoomSummary {
    room_id: String,
}

#[derive(Debug)]
struct ServerRoomInventory {
    room_ids: Vec<String>,
    total_rooms: usize,
    pages: usize,
}

#[derive(Debug)]
struct HistoricalBackfillAudit {
    verified: bool,
    summary: String,
}

#[derive(Debug, Deserialize)]
struct PurgeSubmitResponse {
    purge_id: String,
}

#[derive(Debug, Deserialize)]
struct PurgeStatusResponse {
    status: String,
    error: Option<String>,
}

#[derive(Debug)]
struct PurgeApiError {
    status: Option<StatusCode>,
    body: String,
}

impl fmt::Display for PurgeApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.status {
            Some(status) => write!(f, "Tuwunel purge API returned {}: {}", status, self.body),
            None => write!(f, "Tuwunel purge API request failed: {}", self.body),
        }
    }
}

impl std::error::Error for PurgeApiError {}

pub fn record_bridge_event_ingesting(
    state: &Arc<AppState>,
    user_id: i32,
    service: &str,
    room_id: &str,
    event_id: &str,
    delete_media: bool,
) {
    if !is_matrix_event_id(event_id) || !room_id.starts_with('!') {
        tracing::error!(
            user_id,
            service,
            room_id,
            event_id,
            "Cannot create Tuwunel ingest safety marker for invalid Matrix identifiers"
        );
        return;
    }

    if let Err(error) = state.tuwunel_cleanup_repository.record_ingesting(
        user_id,
        service,
        room_id,
        event_id,
        delete_media,
    ) {
        tracing::error!(
            user_id,
            service,
            room_id,
            event_id,
            error = %error,
            "Failed to create Tuwunel ingest safety marker; purge must remain disabled"
        );
    }
}

pub fn record_bridge_event_ingest_failed(state: &Arc<AppState>, event_id: &str, error: &str) {
    if let Err(record_error) = state
        .tuwunel_cleanup_repository
        .record_ingest_failed(event_id, error)
    {
        tracing::error!(
            event_id,
            error = %error,
            record_error = %record_error,
            "Ontology ingest failed and its Tuwunel purge blocker could not be updated"
        );
    }
}

pub fn enqueue_processed_bridge_event(
    state: &Arc<AppState>,
    user_id: i32,
    service: &str,
    room_id: &str,
    event_id: &str,
    ontology_message_id: i64,
    delete_media: bool,
) {
    if !is_matrix_event_id(event_id) || !room_id.starts_with('!') {
        tracing::warn!(
            user_id,
            service,
            room_id,
            event_id,
            ontology_message_id,
            "Skipping Tuwunel purge candidate with invalid Matrix identifiers"
        );
        return;
    }

    match state.tuwunel_cleanup_repository.record_enqueued(
        user_id,
        ontology_message_id,
        service,
        room_id,
        event_id,
        delete_media,
    ) {
        Ok(()) => tracing::info!(
            user_id,
            service,
            room_id,
            event_id,
            ontology_message_id,
            delete_media,
            "Recorded durable Tuwunel purge candidate after ontology store"
        ),
        Err(error) => tracing::error!(
            user_id,
            service,
            room_id,
            event_id,
            ontology_message_id,
            error = %error,
            "Failed to record durable Tuwunel purge candidate"
        ),
    }
}

pub fn enqueue_intentionally_discarded_bridge_event(
    state: &Arc<AppState>,
    user_id: i32,
    service: &str,
    room_id: &str,
    event_id: &str,
    reason: &str,
) {
    if !is_matrix_event_id(event_id) || !room_id.starts_with('!') {
        tracing::warn!(
            user_id,
            service,
            room_id,
            event_id,
            reason,
            "Skipping invalid intentionally-discarded Tuwunel purge candidate"
        );
        return;
    }

    match state
        .tuwunel_cleanup_repository
        .record_intentionally_discarded(user_id, service, room_id, event_id, reason)
    {
        Ok(true) => tracing::info!(
            user_id,
            service,
            room_id,
            event_id,
            reason,
            "Recorded intentionally-discarded Tuwunel purge candidate"
        ),
        Ok(false) => tracing::debug!(
            user_id,
            service,
            room_id,
            event_id,
            reason,
            "Tuwunel purge candidate already existed"
        ),
        Err(error) => tracing::error!(
            user_id,
            service,
            room_id,
            event_id,
            reason,
            error = %error,
            "Failed to record intentionally-discarded Tuwunel purge candidate"
        ),
    }
}

pub fn record_unproven_bridge_event_blocker(
    state: &Arc<AppState>,
    user_id: i32,
    service: &str,
    room_id: &str,
    event_id: &str,
    reason: &str,
) {
    if !is_matrix_event_id(event_id) || !room_id.starts_with('!') {
        tracing::error!(
            user_id,
            service,
            room_id,
            event_id,
            reason,
            "Cannot create Tuwunel purge blocker for invalid Matrix identifiers"
        );
        return;
    }

    match state
        .tuwunel_cleanup_repository
        .record_unproven_blocker(user_id, service, room_id, event_id, reason)
    {
        Ok(true) => tracing::warn!(
            user_id,
            service,
            room_id,
            event_id,
            reason,
            "Created durable Tuwunel room purge blocker for unproven event"
        ),
        Ok(false) => tracing::debug!(
            user_id,
            service,
            room_id,
            event_id,
            reason,
            "Tuwunel cleanup audit row already exists for retained event"
        ),
        Err(error) => tracing::error!(
            user_id,
            service,
            room_id,
            event_id,
            reason,
            error = %error,
            "Failed to create durable Tuwunel room purge blocker"
        ),
    }
}

pub async fn start_tuwunel_event_purge_worker(state: Arc<AppState>) {
    tracing::info!("Tuwunel event purge worker started");
    let mut next_backfill_scan_at = 0_i32;
    let mut next_portal_census_at = 0_i32;
    let mut next_server_census_at = 0_i32;
    loop {
        let config = EventPurgeConfig::from_env();
        if CONFIG_LOGGED.set(()).is_ok() {
            tracing::info!(
                enabled = config.enabled,
                dry_run = config.dry_run,
                retention_secs = config.retention_secs,
                poll_secs = config.poll_secs,
                max_attempts = config.max_attempts,
                batch_size = config.batch_size,
                backfill_enabled = config.backfill_enabled,
                backfill_audit_enabled = config.backfill_audit_enabled,
                backfill_execute_verified_enabled = config.backfill_execute_verified_enabled,
                backfill_execute_blocked_enabled = config.backfill_execute_blocked_enabled,
                backfill_batch_size = config.backfill_batch_size,
                backfill_scan_secs = config.backfill_scan_secs,
                backfill_min_age_secs = config.backfill_min_age_secs,
                backfill_audit_recheck_secs = config.backfill_audit_recheck_secs,
                backfill_audit_max_pages = config.backfill_audit_max_pages,
                backfill_audit_page_size = config.backfill_audit_page_size,
                portal_census_enabled = config.portal_census_enabled,
                portal_census_scan_secs = config.portal_census_scan_secs,
                portal_census_target_batch_size = config.portal_census_target_batch_size,
                portal_census_room_batch_size = config.portal_census_room_batch_size,
                portal_census_purge_batch_size = config.portal_census_purge_batch_size,
                server_census_enabled = config.server_census_enabled,
                server_census_scan_secs = config.server_census_scan_secs,
                server_census_page_size = config.server_census_page_size,
                server_census_max_pages = config.server_census_max_pages,
                server_census_http_timeout_secs = config.server_census_http_timeout_secs,
                purge_status_poll_batch_size = config.purge_status_poll_batch_size,
                purge_max_in_flight = config.purge_max_in_flight,
                stale_ingest_secs = config.stale_ingest_secs,
                exhausted_retry_secs = config.exhausted_retry_secs,
                "Tuwunel event purge policy loaded"
            );
        }
        let now = now_timestamp();
        let destructive_backfill_enabled = config.backfill_enabled
            && (config.backfill_execute_verified_enabled
                || config.backfill_execute_blocked_enabled);
        let run_backfill = (config.backfill_audit_enabled || destructive_backfill_enabled)
            && now >= next_backfill_scan_at;
        let run_portal_census = config.portal_census_enabled && now >= next_portal_census_at;
        let run_server_census = config.server_census_enabled && now >= next_server_census_at;
        match run_purge_cycle(
            &state,
            &config,
            run_backfill,
            run_portal_census,
            run_server_census,
        )
        .await
        {
            Ok(outcome) => {
                if run_backfill {
                    next_backfill_scan_at = next_backfill_scan_timestamp(
                        now,
                        if destructive_backfill_enabled {
                            outcome.backfilled
                        } else {
                            0
                        },
                        config.backfill_batch_size,
                        config.poll_secs,
                        config.backfill_scan_secs,
                    );
                    tracing::info!(
                        audited = outcome.audited,
                        enqueued = outcome.backfilled,
                        forced_enqueued = outcome.forced_backfilled,
                        next_backfill_scan_at,
                        destructive_backfill_enabled,
                        "Tuwunel historical audit cycle scheduled"
                    );
                }
                if run_portal_census {
                    next_portal_census_at = now
                        .saturating_add(config.portal_census_scan_secs.min(i32::MAX as u64) as i32);
                    tracing::info!(
                        targets = outcome.census_targets,
                        rooms = outcome.census_rooms,
                        next_portal_census_at,
                        "Tuwunel portal-room census cycle scheduled"
                    );
                }
                if run_server_census {
                    let delay = if outcome.server_census_succeeded {
                        config.server_census_scan_secs
                    } else {
                        config.server_census_scan_secs.min(300)
                    };
                    next_server_census_at = now.saturating_add(delay.min(i32::MAX as u64) as i32);
                    tracing::info!(
                        succeeded = outcome.server_census_succeeded,
                        discovered_rooms = outcome.server_census_discovered_rooms,
                        recorded_rooms = outcome.server_census_recorded_rooms,
                        next_server_census_at,
                        "Tuwunel server-wide room census cycle scheduled"
                    );
                }
            }
            Err(error) => {
                tracing::error!(error = %error, "Tuwunel event purge cycle failed");
            }
        }
        tokio::time::sleep(Duration::from_secs(config.poll_secs)).await;
    }
}

pub fn next_backfill_scan_timestamp(
    now: i32,
    inserted: usize,
    batch_size: usize,
    poll_secs: u64,
    scan_secs: u64,
) -> i32 {
    let delay = if inserted >= batch_size {
        poll_secs
    } else {
        scan_secs
    };
    now.saturating_add(delay.min(i32::MAX as u64) as i32)
}

pub fn available_purge_submission_slots(max_in_flight: usize, active_tasks: i64) -> usize {
    max_in_flight.saturating_sub(usize::try_from(active_tasks.max(0)).unwrap_or(usize::MAX))
}

async fn run_purge_cycle(
    state: &Arc<AppState>,
    config: &EventPurgeConfig,
    run_backfill: bool,
    run_portal_census: bool,
    run_server_census: bool,
) -> Result<PurgeCycleOutcome> {
    if !config.enabled {
        if DISABLED_LOGGED.set(()).is_ok() {
            tracing::warn!(
                "Tuwunel event purge is disabled; candidates will remain durable and untouched"
            );
        }
        return Ok(PurgeCycleOutcome::default());
    }

    let now = now_timestamp();
    let stale_ingest_cutoff =
        now.saturating_sub(config.stale_ingest_secs.min(i32::MAX as u64) as i32);
    let recovered = state
        .tuwunel_cleanup_repository
        .recover_stale_ingest_blockers(stale_ingest_cutoff, config.batch_size as i64)?;
    if recovered > 0 {
        tracing::warn!(
            recovered,
            stale_ingest_secs = config.stale_ingest_secs,
            "Recovered stale Tuwunel ingest blockers already present in ontology"
        );
    }

    let exhausted_cutoff =
        now.saturating_sub(config.exhausted_retry_secs.min(i32::MAX as u64) as i32);
    let requeued = state
        .tuwunel_cleanup_repository
        .requeue_exhausted(exhausted_cutoff, config.batch_size as i64)?;
    if requeued > 0 {
        tracing::warn!(
            requeued,
            exhausted_retry_secs = config.exhausted_retry_secs,
            "Requeued exhausted Tuwunel purge candidates after cooldown"
        );
    }
    let room_history_requeued = state
        .tuwunel_cleanup_repository
        .requeue_exhausted_room_history(
            exhausted_cutoff,
            config.portal_census_purge_batch_size as i64,
        )?;
    if room_history_requeued > 0 {
        tracing::warn!(
            requeued = room_history_requeued,
            exhausted_retry_secs = config.exhausted_retry_secs,
            "Requeued exhausted portal-census room-history purges after cooldown"
        );
    }

    let (audited, backfilled, forced_backfilled) = if run_backfill {
        run_historical_backfill_audit(state, config, now).await?
    } else {
        (0, 0, 0)
    };
    let (census_targets, census_rooms) = if run_portal_census {
        run_portal_room_census(state, config, now).await?
    } else {
        (0, 0)
    };
    let (server_census_discovered_rooms, server_census_recorded_rooms, server_census_succeeded) =
        if run_server_census {
            match run_server_room_census(state, config, now).await {
                Ok((discovered, recorded)) => (discovered, recorded, true),
                Err(error) => {
                    let detail = error.to_string();
                    if let Err(record_error) =
                        state.tuwunel_cleanup_repository.record_portal_census_scan(
                            crate::repositories::tuwunel_cleanup_repository::PortalCensusScan {
                                user_id: config.admin_user_id,
                                service: SERVER_CENSUS_SERVICE,
                                status: "room_enumeration_failed",
                                room_count: 0,
                                room_cursor: None,
                                error: Some(&detail),
                                scanned_at: now,
                            },
                        )
                    {
                        tracing::error!(
                            error = %record_error,
                            "Failed to persist Tuwunel server-wide census failure"
                        );
                    }
                    tracing::error!(
                        error = %detail,
                        "Tuwunel server-wide room census failed; no partial inventory was queued"
                    );
                    (0, 0, false)
                }
            }
        } else {
            (0, 0, false)
        };

    log_stale_blockers(state, stale_ingest_cutoff, now)?;

    let cutoff = now.saturating_sub(config.retention_secs.min(i32::MAX as u64) as i32);
    let submitted = state
        .tuwunel_cleanup_repository
        .list_submitted(config.purge_status_poll_batch_size as i64)?;
    let submitted_room_history = state
        .tuwunel_cleanup_repository
        .list_submitted_room_history_purges(config.purge_status_poll_batch_size as i64)?;

    if config.dry_run {
        let due = state
            .tuwunel_cleanup_repository
            .list_due_room_boundaries(cutoff, config.batch_size)?;
        let due_room_history = state
            .tuwunel_cleanup_repository
            .list_due_room_history_purges(config.portal_census_purge_batch_size as i64)?;
        let active_purge_tasks = state
            .tuwunel_cleanup_repository
            .count_submitted_purge_tasks()?;
        if DRY_RUN_LOGGED.set(()).is_ok()
            || !due.is_empty()
            || !submitted.is_empty()
            || !due_room_history.is_empty()
            || !submitted_room_history.is_empty()
        {
            tracing::warn!(
                due_rooms = due.len(),
                submitted_tasks = submitted.len(),
                census_due_rooms = due_room_history.len(),
                census_submitted_tasks = submitted_room_history.len(),
                active_purge_tasks,
                purge_max_in_flight = config.purge_max_in_flight,
                retention_secs = config.retention_secs,
                "Tuwunel event purge dry-run: no purge API calls made"
            );
        }
        return Ok(PurgeCycleOutcome {
            backfilled,
            forced_backfilled,
            audited,
            census_targets,
            census_rooms,
            server_census_discovered_rooms,
            server_census_recorded_rooms,
            server_census_succeeded,
        });
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()?;
    let mut access_token = None;

    if !submitted.is_empty() || !submitted_room_history.is_empty() {
        access_token = Some(admin_access_token(state, config.admin_user_id).await?);
    }

    for candidate in submitted {
        poll_submitted_purge(
            state,
            config,
            &client,
            access_token.as_deref().expect("access token loaded"),
            &candidate,
        )
        .await;
    }
    for candidate in submitted_room_history {
        poll_submitted_room_history_purge(
            state,
            config,
            &client,
            access_token.as_deref().expect("access token loaded"),
            &candidate,
        )
        .await;
    }

    let active_purge_tasks = state
        .tuwunel_cleanup_repository
        .count_submitted_purge_tasks()?;
    let mut submission_slots =
        available_purge_submission_slots(config.purge_max_in_flight, active_purge_tasks);
    let due = state
        .tuwunel_cleanup_repository
        .list_due_room_boundaries(cutoff, submission_slots.min(config.batch_size))?;
    submission_slots = submission_slots.saturating_sub(due.len());
    let due_room_history = state
        .tuwunel_cleanup_repository
        .list_due_room_history_purges(
            submission_slots.min(config.portal_census_purge_batch_size) as i64,
        )?;

    if due.is_empty() && due_room_history.is_empty() {
        if active_purge_tasks > 0 {
            tracing::info!(
                active_purge_tasks,
                purge_max_in_flight = config.purge_max_in_flight,
                "Tuwunel history purge queue is waiting for active tasks to finish"
            );
        }
        return Ok(PurgeCycleOutcome {
            backfilled,
            forced_backfilled,
            audited,
            census_targets,
            census_rooms,
            server_census_discovered_rooms,
            server_census_recorded_rooms,
            server_census_succeeded,
        });
    }

    if access_token.is_none() {
        access_token = Some(admin_access_token(state, config.admin_user_id).await?);
    }
    let access_token = access_token.as_deref().expect("access token loaded");

    for candidate in due {
        submit_purge(state, config, &client, access_token, &candidate).await;
    }
    for candidate in due_room_history {
        submit_room_history_purge(state, config, &client, access_token, &candidate).await;
    }

    Ok(PurgeCycleOutcome {
        backfilled,
        forced_backfilled,
        audited,
        census_targets,
        census_rooms,
        server_census_discovered_rooms,
        server_census_recorded_rooms,
        server_census_succeeded,
    })
}

async fn run_portal_room_census(
    state: &Arc<AppState>,
    config: &EventPurgeConfig,
    now: i32,
) -> Result<(usize, usize)> {
    let targets = state
        .tuwunel_cleanup_repository
        .list_portal_census_targets(config.portal_census_target_batch_size as i64)?;
    let cutoff_ts = now.saturating_sub(config.retention_secs.min(i32::MAX as u64) as i32);
    let mut scanned_targets = 0;
    let mut recorded_rooms = 0;

    for target in targets {
        let matrix_client =
            match crate::utils::matrix_auth::get_cached_client(target.user_id, state).await {
                Ok(client) => client,
                Err(error) => {
                    let detail = error.to_string();
                    if let Err(record_error) =
                        state.tuwunel_cleanup_repository.record_portal_census_scan(
                            crate::repositories::tuwunel_cleanup_repository::PortalCensusScan {
                                user_id: target.user_id,
                                service: &target.service,
                                status: "matrix_session_failed",
                                room_count: 0,
                                room_cursor: target.room_cursor.as_deref(),
                                error: Some(&detail),
                                scanned_at: now,
                            },
                        )
                    {
                        tracing::error!(
                            user_id = target.user_id,
                            service = target.service,
                            error = %record_error,
                            "Failed to persist portal census session failure"
                        );
                    }
                    tracing::warn!(
                        user_id = target.user_id,
                        service = target.service,
                        error = %detail,
                        "Tuwunel portal census skipped unavailable Matrix session"
                    );
                    continue;
                }
            };
        let rooms =
            match crate::utils::bridge::get_service_rooms(&matrix_client, &target.service).await {
                Ok(rooms) => rooms,
                Err(error) => {
                    let detail = error.to_string();
                    if let Err(record_error) =
                        state.tuwunel_cleanup_repository.record_portal_census_scan(
                            crate::repositories::tuwunel_cleanup_repository::PortalCensusScan {
                                user_id: target.user_id,
                                service: &target.service,
                                status: "room_enumeration_failed",
                                room_count: 0,
                                room_cursor: target.room_cursor.as_deref(),
                                error: Some(&detail),
                                scanned_at: now,
                            },
                        )
                    {
                        tracing::error!(
                            user_id = target.user_id,
                            service = target.service,
                            error = %record_error,
                            "Failed to persist portal census enumeration failure"
                        );
                    }
                    tracing::warn!(
                        user_id = target.user_id,
                        service = target.service,
                        error = %detail,
                        "Tuwunel portal census failed to enumerate service rooms"
                    );
                    continue;
                }
            };
        let mut room_ids: Vec<String> = rooms
            .into_iter()
            .map(|room| room.room_id)
            .filter(|room_id| room_id.starts_with('!'))
            .collect();
        room_ids.sort();
        room_ids.dedup();
        let discovered = room_ids.len();
        let (room_ids, next_room_cursor) = select_portal_census_room_batch(
            &room_ids,
            target.room_cursor.as_deref(),
            config.portal_census_room_batch_size,
        );
        let recorded = state
            .tuwunel_cleanup_repository
            .record_portal_census_rooms(
                target.user_id,
                &target.service,
                &room_ids,
                cutoff_ts,
                now,
            )?;
        state.tuwunel_cleanup_repository.record_portal_census_scan(
            crate::repositories::tuwunel_cleanup_repository::PortalCensusScan {
                user_id: target.user_id,
                service: &target.service,
                status: "succeeded",
                room_count: discovered,
                room_cursor: next_room_cursor.as_deref(),
                error: None,
                scanned_at: now,
            },
        )?;
        scanned_targets += 1;
        recorded_rooms += recorded;
        tracing::info!(
            user_id = target.user_id,
            service = target.service,
            discovered_rooms = discovered,
            recorded_rooms = recorded,
            room_limit = config.portal_census_room_batch_size,
            cutoff_ts,
            "Tuwunel portal census persisted room-history purge boundaries"
        );
    }

    Ok((scanned_targets, recorded_rooms))
}

async fn run_server_room_census(
    state: &Arc<AppState>,
    config: &EventPurgeConfig,
    now: i32,
) -> Result<(usize, usize)> {
    let matrix_client = crate::utils::matrix_auth::get_cached_client(config.admin_user_id, state)
        .await
        .with_context(|| {
            format!(
                "could not load Matrix admin user {} for server-wide room census",
                config.admin_user_id
            )
        })?;
    let admin_user = matrix_client
        .user_id()
        .ok_or_else(|| anyhow!("configured Matrix admin client has no authenticated user"))?;
    let session = matrix_client.matrix_auth().session().ok_or_else(|| {
        anyhow!(
            "Matrix admin user {} has no active session for server-wide room census",
            config.admin_user_id
        )
    })?;
    let admin_alias = OwnedRoomAliasId::try_from(format!("#admins:{}", admin_user.server_name()))
        .context("invalid Tuwunel admin room alias")?;
    let admin_room = matrix_client
        .resolve_room_alias(&admin_alias)
        .await
        .context("could not resolve Tuwunel admin room before server-wide purge census")?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.server_census_http_timeout_secs))
        .build()?;
    let inventory = fetch_server_room_inventory(
        &client,
        &config.homeserver_url,
        &session.tokens.access_token,
        config.server_census_page_size,
        config.server_census_max_pages,
    )
    .await?;

    let discovered = inventory.room_ids.len();
    let mut purge_rooms = inventory.room_ids;
    purge_rooms.retain(|room_id| room_id != admin_room.room_id.as_str());
    let excluded_admin_rooms = discovered.saturating_sub(purge_rooms.len());
    if excluded_admin_rooms != 1 {
        return Err(anyhow!(
            "Tuwunel room inventory did not contain the resolved admin room {}",
            admin_room.room_id
        ));
    }

    let cutoff_ts = now.saturating_sub(config.retention_secs.min(i32::MAX as u64) as i32);
    let recorded = state
        .tuwunel_cleanup_repository
        .record_portal_census_rooms(
            config.admin_user_id,
            SERVER_CENSUS_SERVICE,
            &purge_rooms,
            cutoff_ts,
            now,
        )?;
    state.tuwunel_cleanup_repository.record_portal_census_scan(
        crate::repositories::tuwunel_cleanup_repository::PortalCensusScan {
            user_id: config.admin_user_id,
            service: SERVER_CENSUS_SERVICE,
            status: "succeeded",
            room_count: discovered,
            room_cursor: None,
            error: None,
            scanned_at: now,
        },
    )?;

    tracing::info!(
        discovered_rooms = discovered,
        reported_total_rooms = inventory.total_rooms,
        pages = inventory.pages,
        recorded_rooms = recorded,
        excluded_admin_rooms,
        admin_room_id = %admin_room.room_id,
        cutoff_ts,
        "Tuwunel server-wide room census queued exhaustive history purges"
    );

    Ok((discovered, recorded))
}

async fn fetch_server_room_inventory(
    client: &reqwest::Client,
    homeserver_url: &str,
    access_token: &str,
    page_size: usize,
    max_pages: usize,
) -> Result<ServerRoomInventory> {
    let mut offset = 0;
    let mut seen_offsets = HashSet::from([offset]);
    let mut room_ids = HashSet::new();
    let mut total_rooms = 0;

    for page_index in 0..max_pages {
        let url = build_server_rooms_url(homeserver_url, offset, page_size);
        let response = client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await
            .with_context(|| format!("Tuwunel server room census request failed for {url}"))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .context("could not read Tuwunel server room census response")?;
        if !status.is_success() {
            return Err(anyhow!(
                "Tuwunel server room census returned {}: {}",
                status,
                body.chars().take(2000).collect::<String>()
            ));
        }

        let (page_rooms, response_offset, response_total, next_batch) =
            parse_server_room_list_page(&body)?;
        if response_offset != offset {
            return Err(anyhow!(
                "Tuwunel server room census offset mismatch: requested {} got {}",
                offset,
                response_offset
            ));
        }
        total_rooms = total_rooms.max(response_total);
        room_ids.extend(page_rooms);

        let pages = page_index + 1;
        let Some(next_offset) = next_batch else {
            let mut room_ids = room_ids.into_iter().collect::<Vec<_>>();
            room_ids.sort();
            if room_ids.len() != total_rooms {
                return Err(anyhow!(
                    "Tuwunel server room census was incomplete: collected {} unique rooms but server reported {}",
                    room_ids.len(),
                    total_rooms
                ));
            }
            return Ok(ServerRoomInventory {
                room_ids,
                total_rooms,
                pages,
            });
        };
        if next_offset <= offset || !seen_offsets.insert(next_offset) {
            return Err(anyhow!(
                "Tuwunel server room census returned invalid repeated offset {}",
                next_offset
            ));
        }
        offset = next_offset;
    }

    Err(anyhow!(
        "Tuwunel server room census exceeded {} pages before completion",
        max_pages
    ))
}

pub fn parse_server_room_list_page(
    body: &str,
) -> Result<(Vec<String>, usize, usize, Option<usize>)> {
    let response: ServerRoomListResponse = serde_json::from_str(body).with_context(|| {
        format!(
            "invalid Tuwunel server room census JSON: {}",
            body.chars().take(500).collect::<String>()
        )
    })?;
    let mut room_ids = Vec::with_capacity(response.rooms.len());
    for room in response.rooms {
        if !room.room_id.starts_with('!') || room.room_id.chars().any(char::is_control) {
            return Err(anyhow!(
                "Tuwunel server room census returned invalid room id {:?}",
                room.room_id
            ));
        }
        room_ids.push(room.room_id);
    }
    Ok((
        room_ids,
        response.offset,
        response.total_rooms,
        response.next_batch,
    ))
}

pub fn build_server_rooms_url(homeserver_url: &str, offset: usize, limit: usize) -> String {
    format!(
        "{}/_synapse/admin/v1/rooms?from={offset}&limit={limit}",
        homeserver_url.trim_end_matches('/')
    )
}

pub fn select_portal_census_room_batch(
    sorted_room_ids: &[String],
    cursor: Option<&str>,
    limit: usize,
) -> (Vec<String>, Option<String>) {
    if sorted_room_ids.is_empty() || limit == 0 {
        return (Vec::new(), None);
    }
    let start = cursor
        .map(|cursor| sorted_room_ids.partition_point(|room_id| room_id.as_str() <= cursor))
        .unwrap_or(0);
    let selected: Vec<String> = sorted_room_ids
        .iter()
        .skip(start.min(sorted_room_ids.len()))
        .take(limit)
        .cloned()
        .collect();
    let next_cursor = if start.saturating_add(selected.len()) < sorted_room_ids.len() {
        selected.last().cloned()
    } else {
        None
    };
    (selected, next_cursor)
}

async fn submit_room_history_purge(
    state: &Arc<AppState>,
    config: &EventPurgeConfig,
    client: &reqwest::Client,
    access_token: &str,
    candidate: &RoomHistoryPurge,
) {
    let attempt = candidate.attempt_count.saturating_add(1);
    if let Err(error) = state
        .tuwunel_cleanup_repository
        .record_room_history_attempt(candidate.id, attempt)
    {
        tracing::error!(
            purge_row_id = candidate.id,
            room_id = candidate.room_id,
            error = %error,
            "Failed to record portal-census purge attempt"
        );
        return;
    }

    let timestamp_ms = (candidate.cutoff_ts.max(1) as u64).saturating_mul(1000);
    let response = client
        .post(build_purge_history_url(
            &config.homeserver_url,
            &candidate.room_id,
        ))
        .bearer_auth(access_token)
        .json(&purge_history_timestamp_request(timestamp_ms))
        .send()
        .await;
    match parse_submit_response(response).await {
        Ok(submitted) => {
            if let Err(error) = state
                .tuwunel_cleanup_repository
                .record_room_history_submitted(
                    candidate.id,
                    attempt,
                    candidate.cutoff_ts,
                    &submitted.purge_id,
                )
            {
                tracing::error!(
                    purge_row_id = candidate.id,
                    room_id = candidate.room_id,
                    purge_id = submitted.purge_id,
                    error = %error,
                    "Portal-census purge was accepted but durable status update failed"
                );
                return;
            }
            tracing::info!(
                purge_row_id = candidate.id,
                user_id = candidate.user_id,
                service = candidate.service,
                room_id = candidate.room_id,
                cutoff_ts = candidate.cutoff_ts,
                purge_id = submitted.purge_id,
                attempt,
                "Submitted durable portal-census room-history purge"
            );
        }
        Err(error) if room_history_was_already_clean(&error) => {
            match state
                .tuwunel_cleanup_repository
                .record_room_history_noop_succeeded(candidate.id, attempt, candidate.cutoff_ts)
            {
                Ok(()) => tracing::info!(
                    purge_row_id = candidate.id,
                    user_id = candidate.user_id,
                    service = candidate.service,
                    room_id = candidate.room_id,
                    cutoff_ts = candidate.cutoff_ts,
                    attempt,
                    "Tuwunel room history was already empty before the requested cutoff"
                ),
                Err(record_error) => tracing::error!(
                    purge_row_id = candidate.id,
                    room_id = candidate.room_id,
                    error = %record_error,
                    "Failed to persist already-clean Tuwunel room history"
                ),
            }
        }
        Err(error) => {
            record_room_history_purge_failure(state, config, candidate, attempt, &error.to_string())
        }
    }
}

async fn poll_submitted_room_history_purge(
    state: &Arc<AppState>,
    config: &EventPurgeConfig,
    client: &reqwest::Client,
    access_token: &str,
    candidate: &RoomHistoryPurge,
) {
    let Some(purge_id) = candidate.purge_id.as_deref() else {
        record_room_history_purge_failure(
            state,
            config,
            candidate,
            candidate.attempt_count,
            "submitted portal-census purge is missing purge_id",
        );
        return;
    };
    let response = client
        .get(build_purge_status_url(&config.homeserver_url, purge_id))
        .bearer_auth(access_token)
        .send()
        .await;
    match parse_status_response(response).await {
        Ok(status) if status.status == "complete" => {
            match state
                .tuwunel_cleanup_repository
                .record_room_history_succeeded(candidate.id)
            {
                Ok(()) => tracing::info!(
                    purge_row_id = candidate.id,
                    user_id = candidate.user_id,
                    service = candidate.service,
                    room_id = candidate.room_id,
                    submitted_cutoff_ts = candidate.submitted_cutoff_ts,
                    purge_id,
                    "Portal-census room-history purge completed"
                ),
                Err(error) => tracing::error!(
                    purge_row_id = candidate.id,
                    room_id = candidate.room_id,
                    error = %error,
                    "Failed to persist completed portal-census purge"
                ),
            }
        }
        Ok(status) if status.status == "failed" => record_room_history_purge_failure(
            state,
            config,
            candidate,
            candidate.attempt_count,
            status
                .error
                .as_deref()
                .unwrap_or("Tuwunel portal-census purge task failed"),
        ),
        Ok(status) if status.status == "active" => tracing::debug!(
            purge_row_id = candidate.id,
            room_id = candidate.room_id,
            purge_id,
            "Portal-census room-history purge remains active"
        ),
        Ok(status) => record_room_history_purge_failure(
            state,
            config,
            candidate,
            candidate.attempt_count,
            &format!("unknown Tuwunel purge status: {}", status.status),
        ),
        Err(error) if purge_task_status_is_missing(error.status) => {
            record_room_history_purge_failure(
                state,
                config,
                candidate,
                candidate.attempt_count,
                "purge task status disappeared, likely after Tuwunel restart; resubmitting room boundary",
            )
        }
        Err(error) => tracing::warn!(
            purge_row_id = candidate.id,
            room_id = candidate.room_id,
            purge_id,
            error = %error,
            "Could not poll portal-census purge; retaining submitted state"
        ),
    }
}

fn record_room_history_purge_failure(
    state: &Arc<AppState>,
    config: &EventPurgeConfig,
    candidate: &RoomHistoryPurge,
    attempt: i32,
    error: &str,
) {
    if let Err(record_error) = state
        .tuwunel_cleanup_repository
        .record_room_history_failure(candidate.id, attempt, config.max_attempts, error)
    {
        tracing::error!(
            purge_row_id = candidate.id,
            room_id = candidate.room_id,
            error,
            record_error = %record_error,
            "Failed to persist portal-census purge failure"
        );
        return;
    }
    tracing::warn!(
        purge_row_id = candidate.id,
        user_id = candidate.user_id,
        service = candidate.service,
        room_id = candidate.room_id,
        attempt,
        max_attempts = config.max_attempts,
        error,
        "Portal-census room-history purge failed"
    );
}

async fn run_historical_backfill_audit(
    state: &Arc<AppState>,
    config: &EventPurgeConfig,
    now: i32,
) -> Result<(usize, usize, usize)> {
    let boundary_cutoff =
        now.saturating_sub(config.backfill_min_age_secs.min(i32::MAX as u64) as i32);
    let destructive_backfill_enabled = config.backfill_enabled
        && (config.backfill_execute_verified_enabled || config.backfill_execute_blocked_enabled);
    let audit_recheck_cutoff = if destructive_backfill_enabled {
        now
    } else {
        now.saturating_sub(config.backfill_audit_recheck_secs.min(i32::MAX as u64) as i32)
    };
    let batch_size = if destructive_backfill_enabled {
        config.backfill_batch_size
    } else {
        config.backfill_batch_size.min(5)
    };
    let candidates = state
        .tuwunel_cleanup_repository
        .list_historical_backfill_candidates(boundary_cutoff, audit_recheck_cutoff, batch_size)?;

    let mut audited = 0;
    let mut enqueued = 0;
    let mut forced_enqueued = 0;
    for candidate in candidates {
        let audit = if historical_backfill_requires_proof_scan(
            config.backfill_enabled,
            config.backfill_execute_blocked_enabled,
        ) {
            match audit_historical_backfill_candidate(state, config, &candidate).await {
                Ok(audit) => audit,
                Err(error) => HistoricalBackfillAudit {
                    verified: false,
                    summary: format!("audit_error={error}"),
                },
            }
        } else {
            HistoricalBackfillAudit {
                verified: false,
                summary: "proof_scan_bypassed=forced_unverified_policy".to_string(),
            }
        };
        state
            .tuwunel_cleanup_repository
            .record_historical_backfill_audit(&candidate, audit.verified, &audit.summary)?;
        audited += 1;

        let execution_kind = historical_backfill_execution_kind(
            audit.verified,
            config.backfill_enabled,
            config.backfill_execute_verified_enabled,
            config.backfill_execute_blocked_enabled,
        );
        match execution_kind {
            Some("historical_backfill_verified") => {
                state
                    .tuwunel_cleanup_repository
                    .enqueue_verified_historical_backfill(&candidate, &audit.summary)?;
                enqueued += 1;
            }
            Some("historical_backfill_forced_unverified") => {
                state
                    .tuwunel_cleanup_repository
                    .enqueue_forced_historical_backfill(&candidate, &audit.summary)?;
                enqueued += 1;
                forced_enqueued += 1;
            }
            Some(_) | None => {}
        }

        let forced = execution_kind == Some("historical_backfill_forced_unverified");
        tracing::warn!(
            user_id = candidate.user_id,
            service = candidate.service,
            room_id = candidate.room_id,
            boundary_event_id = candidate.event_id,
            boundary_created_at = candidate.created_at,
            verified = audit.verified,
            enqueued = execution_kind.is_some(),
            forced,
            execution_kind = execution_kind.unwrap_or("audit_only"),
            audit = audit.summary,
            "Tuwunel historical room audit completed"
        );
    }

    Ok((audited, enqueued, forced_enqueued))
}

pub fn historical_backfill_execution_kind(
    verified: bool,
    backfill_enabled: bool,
    execute_verified_enabled: bool,
    execute_blocked_enabled: bool,
) -> Option<&'static str> {
    if !backfill_enabled {
        return None;
    }
    if verified && execute_verified_enabled {
        return Some("historical_backfill_verified");
    }
    if !verified && execute_blocked_enabled {
        return Some("historical_backfill_forced_unverified");
    }
    None
}

pub fn historical_backfill_requires_proof_scan(
    backfill_enabled: bool,
    execute_blocked_enabled: bool,
) -> bool {
    !(backfill_enabled && execute_blocked_enabled)
}

async fn audit_historical_backfill_candidate(
    state: &Arc<AppState>,
    config: &EventPurgeConfig,
    candidate: &HistoricalBackfillCandidate,
) -> Result<HistoricalBackfillAudit> {
    let room_id = matrix_sdk::ruma::RoomId::parse(&candidate.room_id)
        .map_err(|error| anyhow!("invalid_room_id={error}"))?;
    let client = crate::utils::matrix_auth::get_cached_client(candidate.user_id, state)
        .await
        .map_err(|error| anyhow!("matrix_client_unavailable={error}"))?;
    let room = client
        .get_room(&room_id)
        .ok_or_else(|| anyhow!("room_not_visible_to_owner_session"))?;

    let mut from = None;
    let mut seen_tokens = HashSet::new();
    let mut proof_event_ids = HashSet::new();
    let mut boundary_found = false;
    let mut room_create_found = false;
    let mut reached_start = false;
    let mut scanned_events = 0_usize;
    let mut pages = 0_usize;

    for _ in 0..config.backfill_audit_max_pages {
        let mut options = matrix_sdk::room::MessagesOptions::backward();
        options.from = from.clone();
        options.limit = matrix_sdk::ruma::UInt::new(config.backfill_audit_page_size)
            .ok_or_else(|| anyhow!("invalid_history_page_size"))?;
        let response = tokio::time::timeout(
            Duration::from_secs(HTTP_TIMEOUT_SECS),
            room.messages(options),
        )
        .await
        .map_err(|_| anyhow!("history_page_timeout"))?
        .map_err(|error| anyhow!("history_page_failed={error}"))?;
        pages += 1;

        if response.chunk.is_empty() {
            reached_start = true;
            break;
        }

        for timeline_event in &response.chunk {
            let event: Value = timeline_event
                .raw()
                .deserialize_as()
                .map_err(|error| anyhow!("history_event_decode_failed={error}"))?;
            let Some(event_id) = event.get("event_id").and_then(Value::as_str) else {
                return Err(anyhow!("history_event_missing_event_id"));
            };
            let Some(event_type) = event.get("type").and_then(Value::as_str) else {
                return Err(anyhow!("history_event_missing_type event_id={event_id}"));
            };

            if !boundary_found {
                if event_id == candidate.event_id {
                    boundary_found = true;
                }
                continue;
            }

            scanned_events = scanned_events.saturating_add(1);
            let is_state_event = event.get("state_key").is_some();
            if event_type == "m.room.create" && is_state_event {
                room_create_found = true;
            }
            if historical_event_requires_proof(event_type, is_state_event) {
                proof_event_ids.insert(event_id.to_string());
            }
        }

        let Some(next_token) = response.end else {
            reached_start = true;
            break;
        };
        if !seen_tokens.insert(next_token.clone()) {
            return Err(anyhow!("history_pagination_token_repeated"));
        }
        from = Some(next_token);
    }

    if !boundary_found {
        return Ok(HistoricalBackfillAudit {
            verified: false,
            summary: format!("boundary_not_found pages={pages}"),
        });
    }
    if !reached_start {
        return Ok(HistoricalBackfillAudit {
            verified: false,
            summary: format!(
                "history_scan_limit_reached pages={pages} max_pages={}",
                config.backfill_audit_max_pages
            ),
        });
    }
    if !room_create_found {
        return Ok(HistoricalBackfillAudit {
            verified: false,
            summary: format!(
                "room_creation_not_visible pages={pages} scanned_events={scanned_events}"
            ),
        });
    }

    let mut proof_event_ids: Vec<String> = proof_event_ids.into_iter().collect();
    proof_event_ids.sort();
    let unproven = state
        .tuwunel_cleanup_repository
        .unproven_event_ids(&proof_event_ids)?;
    if !unproven.is_empty() {
        let sample = unproven
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        return Ok(HistoricalBackfillAudit {
            verified: false,
            summary: format!(
                "unproven_payload_events={} proof_required={} pages={} scanned_events={} sample_event_ids={}",
                unproven.len(),
                proof_event_ids.len(),
                pages,
                scanned_events,
                sample
            ),
        });
    }

    Ok(HistoricalBackfillAudit {
        verified: true,
        summary: format!(
            "verified_full_history proof_events={} pages={} scanned_events={}",
            proof_event_ids.len(),
            pages,
            scanned_events
        ),
    })
}

pub fn historical_event_requires_proof(event_type: &str, is_state_event: bool) -> bool {
    if is_state_event {
        return false;
    }
    !matches!(event_type, "m.reaction" | "m.room.redaction")
}

fn log_stale_blockers(state: &Arc<AppState>, cutoff: i32, now: i32) -> Result<()> {
    let previous = LAST_BLOCKER_LOGGED_AT.load(Ordering::Relaxed);
    let now = i64::from(now);
    if now.saturating_sub(previous) < BLOCKER_LOG_INTERVAL_SECS {
        return Ok(());
    }
    if LAST_BLOCKER_LOGGED_AT
        .compare_exchange(previous, now, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return Ok(());
    }

    let counts = state
        .tuwunel_cleanup_repository
        .stale_blocker_counts(cutoff)?;
    if !counts.is_empty() {
        tracing::error!(
            ?counts,
            cutoff,
            "Tuwunel purge has stale blockers requiring operator attention"
        );
    }
    Ok(())
}

async fn submit_purge(
    state: &Arc<AppState>,
    config: &EventPurgeConfig,
    client: &reqwest::Client,
    access_token: &str,
    candidate: &TuwunelCleanupEvent,
) {
    let attempt = candidate.attempt_count.saturating_add(1);
    if let Err(error) = state
        .tuwunel_cleanup_repository
        .record_attempt(&candidate.event_id, attempt)
    {
        tracing::error!(event_id = %candidate.event_id, error = %error, "Failed to record Tuwunel purge attempt");
        return;
    }

    let url = build_purge_history_url(&config.homeserver_url, &candidate.room_id);
    let forced_historical =
        candidate.last_command_kind.as_deref() == Some("historical_backfill_forced_unverified");
    let (request, boundary_mode, boundary_timestamp_ms) = if forced_historical {
        let created_at = match state
            .tuwunel_cleanup_repository
            .ontology_message_created_at(candidate.ontology_message_id)
        {
            Ok(Some(created_at)) if created_at > 0 => created_at,
            Ok(Some(created_at)) => {
                record_purge_failure(
                    state,
                    config,
                    candidate,
                    attempt,
                    &format!(
                        "forced historical purge has invalid ontology boundary timestamp: {created_at}"
                    ),
                );
                return;
            }
            Ok(None) => {
                record_purge_failure(
                    state,
                    config,
                    candidate,
                    attempt,
                    "forced historical purge boundary ontology message is missing",
                );
                return;
            }
            Err(error) => {
                record_purge_failure(
                    state,
                    config,
                    candidate,
                    attempt,
                    &format!("failed to load forced historical purge timestamp: {error}"),
                );
                return;
            }
        };
        let timestamp_ms = u64::try_from(created_at)
            .unwrap_or_default()
            .saturating_mul(1_000);
        (
            purge_history_timestamp_request(timestamp_ms),
            "ontology_timestamp",
            Some(timestamp_ms),
        )
    } else {
        (purge_history_request(&candidate.event_id), "event_id", None)
    };
    let response = client
        .post(url)
        .bearer_auth(access_token)
        .json(&request)
        .send()
        .await;

    match parse_submit_response(response).await {
        Ok(submitted) => {
            if let Err(error) = state
                .tuwunel_cleanup_repository
                .record_submitted(&candidate.event_id, &submitted.purge_id)
            {
                tracing::error!(
                    room_id = %candidate.room_id,
                    event_id = %candidate.event_id,
                    purge_id = %submitted.purge_id,
                    error = %error,
                    "Purge was accepted but its task id could not be persisted"
                );
                return;
            }
            tracing::info!(
                room_id = %candidate.room_id,
                event_id = %candidate.event_id,
                purge_id = %submitted.purge_id,
                attempt,
                delete_local_events = true,
                forced_historical,
                boundary_mode,
                boundary_timestamp_ms = ?boundary_timestamp_ms,
                "Tuwunel room-history purge submitted"
            );
        }
        Err(error) if event_was_already_purged(&error) => match state
            .tuwunel_cleanup_repository
            .has_newer_successful_boundary(&candidate.room_id, candidate.ontology_message_id)
        {
            Ok(true) => record_purge_succeeded(
                state,
                candidate,
                "boundary absent and a newer successful boundary proves coverage",
            ),
            Ok(false) => record_purge_failure(
                state,
                config,
                candidate,
                attempt,
                "boundary event absent without a newer successful purge boundary",
            ),
            Err(proof_error) => record_purge_failure(
                state,
                config,
                candidate,
                attempt,
                &format!("failed to prove absent boundary coverage: {proof_error}"),
            ),
        },
        Err(error) => record_purge_failure(state, config, candidate, attempt, &error.to_string()),
    }
}

async fn poll_submitted_purge(
    state: &Arc<AppState>,
    config: &EventPurgeConfig,
    client: &reqwest::Client,
    access_token: &str,
    candidate: &TuwunelCleanupEvent,
) {
    let Some(purge_id) = candidate.last_admin_command_event_id.as_deref() else {
        record_purge_failure(
            state,
            config,
            candidate,
            candidate.attempt_count,
            "submitted purge row is missing purge_id",
        );
        return;
    };

    let url = build_purge_status_url(&config.homeserver_url, purge_id);
    let response = client.get(url).bearer_auth(access_token).send().await;
    match parse_status_response(response).await {
        Ok(status) if status.status == "complete" => {
            record_purge_succeeded(state, candidate, "Tuwunel task completed");
        }
        Ok(status) if status.status == "failed" => record_purge_failure(
            state,
            config,
            candidate,
            candidate.attempt_count,
            status
                .error
                .as_deref()
                .unwrap_or("Tuwunel purge task failed"),
        ),
        Ok(status) if status.status == "active" => tracing::debug!(
            room_id = %candidate.room_id,
            event_id = %candidate.event_id,
            purge_id,
            "Tuwunel room-history purge remains active"
        ),
        Ok(status) => record_purge_failure(
            state,
            config,
            candidate,
            candidate.attempt_count,
            &format!("unknown Tuwunel purge status {}", status.status),
        ),
        Err(error) if purge_task_status_is_missing(error.status) => record_purge_failure(
            state,
            config,
            candidate,
            candidate.attempt_count,
            "purge task status disappeared, likely after Tuwunel restart; resubmitting boundary",
        ),
        Err(error) => record_purge_failure(
            state,
            config,
            candidate,
            candidate.attempt_count,
            &error.to_string(),
        ),
    }
}

fn record_purge_succeeded(state: &Arc<AppState>, candidate: &TuwunelCleanupEvent, reason: &str) {
    match state
        .tuwunel_cleanup_repository
        .record_room_succeeded_through(&candidate.room_id, candidate.enqueued_at)
    {
        Ok(rows) => tracing::info!(
            room_id = %candidate.room_id,
            boundary_event_id = %candidate.event_id,
            completed_rows = rows,
            reason,
            "Tuwunel room-history purge completed"
        ),
        Err(error) => tracing::error!(
            room_id = %candidate.room_id,
            boundary_event_id = %candidate.event_id,
            error = %error,
            "Tuwunel purge completed but audit rows could not be updated"
        ),
    }
}

fn record_purge_failure(
    state: &Arc<AppState>,
    config: &EventPurgeConfig,
    candidate: &TuwunelCleanupEvent,
    attempt: i32,
    error: &str,
) {
    let exhausted = attempt >= config.max_attempts;
    let result = if exhausted {
        state
            .tuwunel_cleanup_repository
            .record_exhausted(&candidate.event_id, attempt, error)
    } else {
        state
            .tuwunel_cleanup_repository
            .record_retrying(&candidate.event_id, attempt, error)
    };

    if let Err(record_error) = result {
        tracing::error!(event_id = %candidate.event_id, error = %record_error, "Failed to persist Tuwunel purge failure");
    }
    tracing::error!(
        room_id = %candidate.room_id,
        event_id = %candidate.event_id,
        attempt,
        max_attempts = config.max_attempts,
        exhausted,
        error,
        "Tuwunel room-history purge failed"
    );
}

async fn admin_access_token(state: &Arc<AppState>, admin_user_id: i32) -> Result<String> {
    let client = crate::utils::matrix_auth::get_cached_client(admin_user_id, state)
        .await
        .map_err(|error| {
            anyhow!(
                "failed to load Matrix admin user {}: {}",
                admin_user_id,
                error
            )
        })?;
    let session = client.matrix_auth().session().ok_or_else(|| {
        anyhow!(
            "Matrix admin user {} has no active session for Tuwunel purge API",
            admin_user_id
        )
    })?;
    Ok(session.tokens.access_token.clone())
}

async fn parse_submit_response(
    response: std::result::Result<reqwest::Response, reqwest::Error>,
) -> std::result::Result<PurgeSubmitResponse, PurgeApiError> {
    parse_json_response(response).await
}

async fn parse_status_response(
    response: std::result::Result<reqwest::Response, reqwest::Error>,
) -> std::result::Result<PurgeStatusResponse, PurgeApiError> {
    parse_json_response(response).await
}

async fn parse_json_response<T: for<'de> Deserialize<'de>>(
    response: std::result::Result<reqwest::Response, reqwest::Error>,
) -> std::result::Result<T, PurgeApiError> {
    let response = response.map_err(|error| PurgeApiError {
        status: error.status(),
        body: error.to_string(),
    })?;
    let status = response.status();
    let body = response.text().await.map_err(|error| PurgeApiError {
        status: Some(status),
        body: error.to_string(),
    })?;
    if !status.is_success() {
        return Err(PurgeApiError {
            status: Some(status),
            body: body.chars().take(2000).collect(),
        });
    }
    serde_json::from_str(&body).map_err(|error| PurgeApiError {
        status: Some(status),
        body: format!("invalid JSON response: {}; body={}", error, body),
    })
}

fn event_was_already_purged(error: &PurgeApiError) -> bool {
    error.status == Some(StatusCode::NOT_FOUND)
        && error.body.to_ascii_lowercase().contains("event not found")
}

pub fn room_history_error_is_already_clean(status: Option<StatusCode>, body: &str) -> bool {
    status == Some(StatusCode::NOT_FOUND)
        && body
            .to_ascii_lowercase()
            .contains("no event found before the given timestamp")
}

fn room_history_was_already_clean(error: &PurgeApiError) -> bool {
    room_history_error_is_already_clean(error.status, &error.body)
}

pub fn purge_task_status_is_missing(status: Option<StatusCode>) -> bool {
    status == Some(StatusCode::NOT_FOUND)
}

pub fn build_purge_history_url(homeserver_url: &str, room_id: &str) -> String {
    format!(
        "{}/_synapse/admin/v1/purge_history/{}",
        homeserver_url.trim_end_matches('/'),
        urlencoding::encode(room_id)
    )
}

pub fn build_purge_status_url(homeserver_url: &str, purge_id: &str) -> String {
    format!(
        "{}/_synapse/admin/v1/purge_history_status/{}",
        homeserver_url.trim_end_matches('/'),
        urlencoding::encode(purge_id)
    )
}

pub fn purge_history_request(event_id: &str) -> Value {
    json!({
        "purge_up_to_event_id": event_id,
        "delete_local_events": true
    })
}

pub fn purge_history_timestamp_request(timestamp_ms: u64) -> Value {
    json!({
        "purge_up_to_ts": timestamp_ms,
        "delete_local_events": true
    })
}

pub fn is_tuwunel_admin_redaction_reason(reason: Option<&str>) -> bool {
    reason.is_some_and(|reason| {
        reason.starts_with("The administrator(s) of ")
            && reason.ends_with(" has redacted this user's message.")
    })
}

pub fn is_matrix_event_id(event_id: &str) -> bool {
    event_id.starts_with('$') && !event_id.chars().any(char::is_control)
}

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(default)
}

impl EventPurgeConfig {
    fn from_env() -> Self {
        Self {
            homeserver_url: std::env::var("MATRIX_HOMESERVER")
                .unwrap_or_else(|_| DEFAULT_HOMESERVER_URL.to_string()),
            admin_user_id: std::env::var("TUWUNEL_ADMIN_USER_ID")
                .ok()
                .and_then(|value| value.parse().ok())
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_ADMIN_USER_ID),
            enabled: env_flag("TUWUNEL_EVENT_PURGE_ENABLED", true),
            dry_run: env_flag("TUWUNEL_EVENT_PURGE_DRY_RUN", false),
            retention_secs: env_u64(
                "TUWUNEL_EVENT_PURGE_RETENTION_SECS",
                DEFAULT_RETENTION_SECS,
                1,
            ),
            poll_secs: env_u64("TUWUNEL_EVENT_PURGE_POLL_SECS", DEFAULT_POLL_SECS, 5),
            max_attempts: env_i32("TUWUNEL_EVENT_PURGE_MAX_ATTEMPTS", DEFAULT_MAX_ATTEMPTS, 1),
            batch_size: env_u64(
                "TUWUNEL_EVENT_PURGE_BATCH_SIZE",
                DEFAULT_BATCH_SIZE as u64,
                1,
            )
            .min(100) as usize,
            backfill_enabled: env_flag("TUWUNEL_EVENT_PURGE_BACKFILL_ENABLED", true),
            backfill_audit_enabled: env_flag("TUWUNEL_EVENT_PURGE_BACKFILL_AUDIT_ENABLED", true),
            backfill_execute_verified_enabled: env_flag(
                "TUWUNEL_EVENT_PURGE_BACKFILL_EXECUTE_VERIFIED_ENABLED",
                true,
            ),
            backfill_execute_blocked_enabled: env_flag(
                "TUWUNEL_EVENT_PURGE_BACKFILL_EXECUTE_BLOCKED_ENABLED",
                true,
            ),
            backfill_batch_size: env_u64(
                "TUWUNEL_EVENT_PURGE_BACKFILL_BATCH_SIZE",
                DEFAULT_BACKFILL_BATCH_SIZE as u64,
                1,
            )
            .min(100) as usize,
            backfill_scan_secs: env_u64(
                "TUWUNEL_EVENT_PURGE_BACKFILL_SCAN_SECS",
                DEFAULT_BACKFILL_SCAN_SECS,
                60,
            ),
            backfill_min_age_secs: env_u64(
                "TUWUNEL_EVENT_PURGE_BACKFILL_MIN_AGE_SECS",
                DEFAULT_BACKFILL_MIN_AGE_SECS,
                60,
            ),
            backfill_audit_recheck_secs: env_u64(
                "TUWUNEL_EVENT_PURGE_BACKFILL_AUDIT_RECHECK_SECS",
                DEFAULT_BACKFILL_AUDIT_RECHECK_SECS,
                60,
            ),
            backfill_audit_max_pages: env_u64(
                "TUWUNEL_EVENT_PURGE_BACKFILL_AUDIT_MAX_PAGES",
                DEFAULT_BACKFILL_AUDIT_MAX_PAGES as u64,
                1,
            )
            .min(1000) as usize,
            backfill_audit_page_size: env_u64(
                "TUWUNEL_EVENT_PURGE_BACKFILL_AUDIT_PAGE_SIZE",
                DEFAULT_BACKFILL_AUDIT_PAGE_SIZE,
                10,
            )
            .min(100),
            portal_census_enabled: env_flag("TUWUNEL_PORTAL_CENSUS_PURGE_ENABLED", true),
            portal_census_scan_secs: env_u64(
                "TUWUNEL_PORTAL_CENSUS_SCAN_SECS",
                DEFAULT_PORTAL_CENSUS_SCAN_SECS,
                60,
            ),
            portal_census_target_batch_size: env_u64(
                "TUWUNEL_PORTAL_CENSUS_TARGET_BATCH_SIZE",
                DEFAULT_PORTAL_CENSUS_TARGET_BATCH_SIZE as u64,
                1,
            )
            .min(50) as usize,
            portal_census_room_batch_size: env_u64(
                "TUWUNEL_PORTAL_CENSUS_ROOM_BATCH_SIZE",
                DEFAULT_PORTAL_CENSUS_ROOM_BATCH_SIZE as u64,
                1,
            )
            .min(1000) as usize,
            portal_census_purge_batch_size: env_u64(
                "TUWUNEL_PORTAL_CENSUS_PURGE_BATCH_SIZE",
                DEFAULT_PORTAL_CENSUS_PURGE_BATCH_SIZE as u64,
                1,
            )
            .min(100) as usize,
            server_census_enabled: env_flag("TUWUNEL_SERVER_CENSUS_PURGE_ENABLED", true),
            server_census_scan_secs: env_u64(
                "TUWUNEL_SERVER_CENSUS_SCAN_SECS",
                DEFAULT_SERVER_CENSUS_SCAN_SECS,
                300,
            ),
            server_census_page_size: env_u64(
                "TUWUNEL_SERVER_CENSUS_PAGE_SIZE",
                DEFAULT_SERVER_CENSUS_PAGE_SIZE as u64,
                100,
            )
            .min(10_000) as usize,
            server_census_max_pages: env_u64(
                "TUWUNEL_SERVER_CENSUS_MAX_PAGES",
                DEFAULT_SERVER_CENSUS_MAX_PAGES as u64,
                1,
            )
            .min(100) as usize,
            server_census_http_timeout_secs: env_u64(
                "TUWUNEL_SERVER_CENSUS_HTTP_TIMEOUT_SECS",
                DEFAULT_SERVER_CENSUS_HTTP_TIMEOUT_SECS,
                15,
            )
            .min(600),
            purge_status_poll_batch_size: env_u64(
                "TUWUNEL_PURGE_STATUS_POLL_BATCH_SIZE",
                DEFAULT_PURGE_STATUS_POLL_BATCH_SIZE as u64,
                1,
            )
            .min(1000) as usize,
            purge_max_in_flight: env_u64(
                "TUWUNEL_PURGE_MAX_IN_FLIGHT",
                DEFAULT_PURGE_MAX_IN_FLIGHT as u64,
                1,
            )
            .min(32) as usize,
            stale_ingest_secs: env_u64(
                "TUWUNEL_EVENT_PURGE_STALE_INGEST_SECS",
                DEFAULT_STALE_INGEST_SECS,
                60,
            ),
            exhausted_retry_secs: env_u64(
                "TUWUNEL_EVENT_PURGE_EXHAUSTED_RETRY_SECS",
                DEFAULT_EXHAUSTED_RETRY_SECS,
                60,
            ),
        }
    }
}

fn env_u64(name: &str, default: u64, minimum: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value >= minimum)
        .unwrap_or(default)
}

fn env_i32(name: &str, default: i32, minimum: i32) -> i32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value >= minimum)
        .unwrap_or(default)
}
