use crate::pg_models::TuwunelCleanupEvent;
use crate::repositories::tuwunel_cleanup_repository::{now_timestamp, HistoricalBackfillCandidate};
use crate::AppState;
use anyhow::{anyhow, Result};
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
const DEFAULT_BATCH_SIZE: usize = 10;
const DEFAULT_BACKFILL_BATCH_SIZE: usize = 25;
const DEFAULT_BACKFILL_SCAN_SECS: u64 = 3600;
const DEFAULT_BACKFILL_MIN_AGE_SECS: u64 = 86_400;
const DEFAULT_BACKFILL_AUDIT_RECHECK_SECS: u64 = 86_400;
const DEFAULT_BACKFILL_AUDIT_MAX_PAGES: usize = 100;
const DEFAULT_BACKFILL_AUDIT_PAGE_SIZE: u64 = 100;
const DEFAULT_STALE_INGEST_SECS: u64 = 300;
const DEFAULT_EXHAUSTED_RETRY_SECS: u64 = 900;
const BLOCKER_LOG_INTERVAL_SECS: i64 = 600;
const HTTP_TIMEOUT_SECS: u64 = 15;

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
    stale_ingest_secs: u64,
    exhausted_retry_secs: u64,
}

#[derive(Debug, Default)]
struct PurgeCycleOutcome {
    backfilled: usize,
    forced_backfilled: usize,
    audited: usize,
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
        match run_purge_cycle(&state, &config, run_backfill).await {
            Ok(outcome) if run_backfill => {
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
            Ok(_) => {}
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

async fn run_purge_cycle(
    state: &Arc<AppState>,
    config: &EventPurgeConfig,
    run_backfill: bool,
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

    let (audited, backfilled, forced_backfilled) = if run_backfill {
        run_historical_backfill_audit(state, config, now).await?
    } else {
        (0, 0, 0)
    };

    log_stale_blockers(state, stale_ingest_cutoff, now)?;

    let cutoff = now.saturating_sub(config.retention_secs.min(i32::MAX as u64) as i32);
    let due = state
        .tuwunel_cleanup_repository
        .list_due_room_boundaries(cutoff, config.batch_size)?;
    let submitted = state
        .tuwunel_cleanup_repository
        .list_submitted(config.batch_size as i64)?;

    if config.dry_run {
        if DRY_RUN_LOGGED.set(()).is_ok() || !due.is_empty() || !submitted.is_empty() {
            tracing::warn!(
                due_rooms = due.len(),
                submitted_tasks = submitted.len(),
                retention_secs = config.retention_secs,
                "Tuwunel event purge dry-run: no purge API calls made"
            );
        }
        return Ok(PurgeCycleOutcome {
            backfilled,
            forced_backfilled,
            audited,
        });
    }

    if due.is_empty() && submitted.is_empty() {
        return Ok(PurgeCycleOutcome {
            backfilled,
            forced_backfilled,
            audited,
        });
    }

    let access_token = admin_access_token(state, config.admin_user_id).await?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()?;

    for candidate in submitted {
        poll_submitted_purge(state, config, &client, &access_token, &candidate).await;
    }

    for candidate in due {
        submit_purge(state, config, &client, &access_token, &candidate).await;
    }

    Ok(PurgeCycleOutcome {
        backfilled,
        forced_backfilled,
        audited,
    })
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
        let audit = match audit_historical_backfill_candidate(state, config, &candidate).await {
            Ok(audit) => audit,
            Err(error) => HistoricalBackfillAudit {
                verified: false,
                summary: format!("audit_error={error}"),
            },
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
        Err(error) if error.status == Some(StatusCode::NOT_FOUND) => record_purge_failure(
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
            enabled: env_flag("TUWUNEL_EVENT_PURGE_ENABLED", false),
            dry_run: env_flag("TUWUNEL_EVENT_PURGE_DRY_RUN", true),
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
                3600,
            ),
            backfill_audit_recheck_secs: env_u64(
                "TUWUNEL_EVENT_PURGE_BACKFILL_AUDIT_RECHECK_SECS",
                DEFAULT_BACKFILL_AUDIT_RECHECK_SECS,
                3600,
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
