use crate::AppState;
use anyhow::{anyhow, Result};
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};
use std::time::Duration;
use tokio::sync::{mpsc, OnceCell};
use uuid::Uuid;

const QUEUE_CAPACITY: usize = 1024;
const DEFAULT_MAX_ATTEMPTS: u8 = 5;
const DEFAULT_HOMESERVER_URL: &str = "http://localhost:8008";
const DEFAULT_ADMIN_USER_ID: i32 = 1;
const DEFAULT_ADMIN_ROOM_NAME: &str = "Lightfriend Tuwunel Admin";

static CLEANUP_TX: OnceLock<mpsc::Sender<EventCleanupJob>> = OnceLock::new();
static ADMIN_ROOM_ID_CACHE: OnceCell<String> = OnceCell::const_new();
static DISABLED_LOGGED: AtomicBool = AtomicBool::new(false);
static INVALID_EVENT_ID_LOGGED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
struct EventCleanupJob {
    user_id: i32,
    ontology_message_id: i64,
    service: String,
    room_id: String,
    event_id: String,
    delete_media: bool,
    attempt: u8,
}

#[derive(Debug, Clone)]
struct EventCleanupConfig {
    homeserver_url: String,
    admin_room_id: Option<String>,
    admin_access_token: Option<String>,
    admin_user_id: i32,
    max_attempts: u8,
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
    if !cleanup_enabled() {
        log_disabled_once();
        return;
    }

    if !is_command_safe_event_id(event_id) {
        log_invalid_event_id_once(event_id);
        tracing::warn!(
            user_id,
            service,
            room_id,
            event_id,
            "Skipping Tuwunel event cleanup because Matrix event_id is not command-safe"
        );
        return;
    }

    let state_for_worker = state.clone();
    let tx = CLEANUP_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel(QUEUE_CAPACITY);
        tokio::spawn(event_cleanup_worker(rx, tx.clone(), state_for_worker));
        tx
    });

    let job = EventCleanupJob {
        user_id,
        ontology_message_id,
        service: service.to_string(),
        room_id: room_id.to_string(),
        event_id: event_id.to_string(),
        delete_media,
        attempt: 1,
    };

    match tx.try_send(job.clone()) {
        Ok(()) => {
            record_cleanup_enqueued(state, &job);
            tracing::info!(
                user_id,
                service,
                room_id,
                event_id,
                ontology_message_id,
                delete_media,
                "Tuwunel event cleanup enqueued after ontology store"
            );
        }
        Err(e) => {
            tracing::warn!(
                user_id,
                service,
                room_id,
                event_id,
                ontology_message_id,
                delete_media,
                error = %e,
                "Tuwunel event cleanup queue is full or closed; ingest will continue"
            );
        }
    }
}

pub fn build_delete_media_by_event_command(event_id: &str) -> String {
    format!("!admin media delete-by-event --event-id {}", event_id)
}

pub fn build_redact_event_command(event_id: &str) -> String {
    format!("!admin users redact-event {}", event_id)
}

pub fn is_tuwunel_admin_redaction_reason(reason: Option<&str>) -> bool {
    reason.is_some_and(|reason| {
        reason.starts_with("The administrator(s) of ")
            && reason.ends_with(" has redacted this user's message.")
    })
}

fn cleanup_enabled() -> bool {
    std::env::var("TUWUNEL_EVENT_CLEANUP_ENABLED")
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(true)
}

fn log_disabled_once() {
    if !DISABLED_LOGGED.swap(true, Ordering::Relaxed) {
        tracing::info!("Tuwunel event cleanup is disabled by env");
    }
}

fn log_invalid_event_id_once(event_id: &str) {
    if !INVALID_EVENT_ID_LOGGED.swap(true, Ordering::Relaxed) {
        tracing::warn!(
            event_id,
            "Observed a Matrix event_id that cannot be safely used in a Tuwunel admin command"
        );
    }
}

pub fn is_command_safe_event_id(event_id: &str) -> bool {
    event_id.starts_with('$')
        && !event_id
            .chars()
            .any(|c| c.is_control() || c.is_whitespace())
}

fn record_cleanup_enqueued(state: &Arc<AppState>, job: &EventCleanupJob) {
    if let Err(e) = state.tuwunel_cleanup_repository.record_enqueued(
        job.user_id,
        job.ontology_message_id,
        &job.service,
        &job.room_id,
        &job.event_id,
        job.delete_media,
    ) {
        tracing::warn!(
            user_id = job.user_id,
            ontology_message_id = job.ontology_message_id,
            service = %job.service,
            room_id = %job.room_id,
            event_id = %job.event_id,
            error = %e,
            "Failed to record Tuwunel cleanup enqueue instrumentation"
        );
    }
}

fn record_cleanup_attempt(state: &Arc<AppState>, job: &EventCleanupJob) {
    if let Err(e) = state
        .tuwunel_cleanup_repository
        .record_attempt(&job.event_id, job.attempt)
    {
        tracing::warn!(
            user_id = job.user_id,
            ontology_message_id = job.ontology_message_id,
            service = %job.service,
            room_id = %job.room_id,
            event_id = %job.event_id,
            attempt = job.attempt,
            error = %e,
            "Failed to record Tuwunel cleanup attempt instrumentation"
        );
    }
}

fn record_cleanup_command_accepted(
    state: &Arc<AppState>,
    job: &EventCleanupJob,
    sent_command: &SentAdminCommand,
) {
    if let Err(e) = state.tuwunel_cleanup_repository.record_command_accepted(
        &job.event_id,
        sent_command.kind,
        &sent_command.admin_room_id,
        &sent_command.admin_command_event_id,
    ) {
        tracing::warn!(
            user_id = job.user_id,
            ontology_message_id = job.ontology_message_id,
            service = %job.service,
            room_id = %job.room_id,
            event_id = %job.event_id,
            cleanup_command_kind = sent_command.kind,
            admin_room_id = %sent_command.admin_room_id,
            admin_command_event_id = %sent_command.admin_command_event_id,
            error = %e,
            "Failed to record Tuwunel cleanup accepted-command instrumentation"
        );
    }
}

fn record_cleanup_retrying(state: &Arc<AppState>, job: &EventCleanupJob, error: &str) {
    if let Err(e) =
        state
            .tuwunel_cleanup_repository
            .record_retrying(&job.event_id, job.attempt, error)
    {
        tracing::warn!(
            user_id = job.user_id,
            ontology_message_id = job.ontology_message_id,
            service = %job.service,
            room_id = %job.room_id,
            event_id = %job.event_id,
            attempt = job.attempt,
            error = %e,
            cleanup_error = %error,
            "Failed to record Tuwunel cleanup retry instrumentation"
        );
    }
}

fn record_cleanup_exhausted(state: &Arc<AppState>, job: &EventCleanupJob, error: &str) {
    if let Err(e) =
        state
            .tuwunel_cleanup_repository
            .record_exhausted(&job.event_id, job.attempt, error)
    {
        tracing::warn!(
            user_id = job.user_id,
            ontology_message_id = job.ontology_message_id,
            service = %job.service,
            room_id = %job.room_id,
            event_id = %job.event_id,
            attempt = job.attempt,
            error = %e,
            cleanup_error = %error,
            "Failed to record Tuwunel cleanup exhausted instrumentation"
        );
    }
}

fn record_cleanup_succeeded(state: &Arc<AppState>, job: &EventCleanupJob) {
    if let Err(e) = state
        .tuwunel_cleanup_repository
        .record_succeeded(&job.event_id)
    {
        tracing::warn!(
            user_id = job.user_id,
            ontology_message_id = job.ontology_message_id,
            service = %job.service,
            room_id = %job.room_id,
            event_id = %job.event_id,
            attempt = job.attempt,
            error = %e,
            "Failed to record Tuwunel cleanup success instrumentation"
        );
    }
}

async fn event_cleanup_worker(
    mut rx: mpsc::Receiver<EventCleanupJob>,
    tx: mpsc::Sender<EventCleanupJob>,
    state: Arc<AppState>,
) {
    while let Some(job) = rx.recv().await {
        let config = EventCleanupConfig::from_env();
        record_cleanup_enqueued(&state, &job);
        record_cleanup_attempt(&state, &job);

        match send_cleanup_commands(&state, &config, &job).await {
            Ok(sent_commands) => {
                record_cleanup_succeeded(&state, &job);
                for sent_command in &sent_commands {
                    tracing::info!(
                        user_id = job.user_id,
                        ontology_message_id = job.ontology_message_id,
                        service = %job.service,
                        room_id = %job.room_id,
                        source_event_id = %job.event_id,
                        cleanup_command_kind = sent_command.kind,
                        admin_room_id = %sent_command.admin_room_id,
                        admin_room_source = sent_command.admin_room_source,
                        admin_auth_source = sent_command.admin_auth_source,
                        admin_user_id = config.admin_user_id,
                        admin_command_event_id = %sent_command.admin_command_event_id,
                        txn_id = %sent_command.txn_id,
                        attempt = job.attempt,
                        "Tuwunel cleanup admin command message accepted by Matrix"
                    );
                }
                tracing::info!(
                    user_id = job.user_id,
                    ontology_message_id = job.ontology_message_id,
                    service = %job.service,
                    room_id = %job.room_id,
                    event_id = %job.event_id,
                    delete_media = job.delete_media,
                    commands_sent = sent_commands.len(),
                    attempt = job.attempt,
                    "Tuwunel event cleanup admin command messages sent"
                );
            }
            Err(e) => {
                if job.attempt >= config.max_attempts {
                    record_cleanup_exhausted(&state, &job, &e.to_string());
                    tracing::warn!(
                        user_id = job.user_id,
                        ontology_message_id = job.ontology_message_id,
                        service = %job.service,
                        room_id = %job.room_id,
                        event_id = %job.event_id,
                        attempt = job.attempt,
                        max_attempts = config.max_attempts,
                        error = %e,
                        "Tuwunel event cleanup exhausted retries"
                    );
                    continue;
                }

                let delay = retry_delay(job.attempt);
                let mut retry_job = job.clone();
                retry_job.attempt = retry_job.attempt.saturating_add(1);
                record_cleanup_retrying(&state, &job, &e.to_string());
                tracing::warn!(
                    user_id = job.user_id,
                    ontology_message_id = job.ontology_message_id,
                    service = %job.service,
                    room_id = %job.room_id,
                    event_id = %job.event_id,
                    attempt = job.attempt,
                    retry_in_secs = delay.as_secs(),
                    error = %e,
                    "Tuwunel event cleanup failed; retrying later"
                );

                let retry_tx = tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(delay).await;
                    if let Err(e) = retry_tx.send(retry_job).await {
                        tracing::warn!(
                            error = %e,
                            "Failed to requeue Tuwunel event cleanup job"
                        );
                    }
                });
            }
        }
    }
}

#[derive(Debug)]
struct SentAdminCommand {
    kind: &'static str,
    txn_id: String,
    admin_room_id: String,
    admin_room_source: &'static str,
    admin_auth_source: &'static str,
    admin_command_event_id: String,
}

struct AdminCommandTarget {
    room_id: String,
    room_source: &'static str,
    access_token: String,
    auth_source: &'static str,
}

async fn send_cleanup_commands(
    state: &Arc<AppState>,
    config: &EventCleanupConfig,
    job: &EventCleanupJob,
) -> Result<Vec<SentAdminCommand>> {
    let mut sent_commands = Vec::with_capacity(if job.delete_media { 2 } else { 1 });
    let target = resolve_admin_command_target(state, config, job).await?;

    if job.delete_media {
        let sent_command = send_admin_room_command(
            config,
            &target,
            job,
            "media_delete_by_event",
            &build_delete_media_by_event_command(&job.event_id),
        )
        .await
        .map_err(|e| anyhow!("media delete-by-event command failed: {}", e))?;
        record_cleanup_command_accepted(state, job, &sent_command);
        sent_commands.push(sent_command);
    }

    let sent_command = send_admin_room_command(
        config,
        &target,
        job,
        "redact_event",
        &build_redact_event_command(&job.event_id),
    )
    .await
    .map_err(|e| anyhow!("redact-event command failed: {}", e))?;
    record_cleanup_command_accepted(state, job, &sent_command);
    sent_commands.push(sent_command);

    Ok(sent_commands)
}

async fn resolve_admin_command_target(
    state: &Arc<AppState>,
    config: &EventCleanupConfig,
    job: &EventCleanupJob,
) -> Result<AdminCommandTarget> {
    let (access_token, auth_source) = match config.admin_access_token.as_ref() {
        Some(token) => (token.clone(), "env_token"),
        None => (
            admin_access_token_from_matrix_user(state, config.admin_user_id, job).await?,
            "matrix_user_session",
        ),
    };

    let (room_id, room_source) = match config.admin_room_id.as_ref() {
        Some(room_id) => (room_id.clone(), "env_room"),
        None => {
            let room_id = ADMIN_ROOM_ID_CACHE
                .get_or_try_init(|| create_admin_command_room(config, &access_token))
                .await?
                .clone();
            (room_id, "created_or_cached_room")
        }
    };

    Ok(AdminCommandTarget {
        room_id,
        room_source,
        access_token,
        auth_source,
    })
}

async fn admin_access_token_from_matrix_user(
    state: &Arc<AppState>,
    admin_user_id: i32,
    job: &EventCleanupJob,
) -> Result<String> {
    let client = crate::utils::matrix_auth::get_cached_client(admin_user_id, state)
        .await
        .map_err(|e| {
            anyhow!(
                "failed to get Matrix admin client for user {} while cleaning event {}: {}",
                admin_user_id,
                job.event_id,
                e
            )
        })?;
    let session = client.matrix_auth().session().ok_or_else(|| {
        anyhow!(
            "Matrix admin client for user {} has no active session while cleaning event {}",
            admin_user_id,
            job.event_id
        )
    })?;
    Ok(session.tokens.access_token.clone())
}

async fn create_admin_command_room(
    config: &EventCleanupConfig,
    access_token: &str,
) -> Result<String> {
    let url = format!(
        "{}/_matrix/client/v3/createRoom",
        config.homeserver_url.trim_end_matches('/')
    );

    tracing::warn!(
        admin_user_id = config.admin_user_id,
        homeserver_url = %config.homeserver_url,
        "TUWUNEL_ADMIN_ROOM_ID not set; creating a private Matrix room for Tuwunel admin commands"
    );

    let response = reqwest::Client::new()
        .post(url)
        .bearer_auth(access_token)
        .json(&json!({
            "preset": "private_chat",
            "visibility": "private",
            "is_direct": false,
            "name": DEFAULT_ADMIN_ROOM_NAME,
            "topic": "Lightfriend internal Tuwunel maintenance commands"
        }))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| {
            anyhow!(
                "failed to create Matrix room for Tuwunel admin commands: {}",
                e
            )
        })?;

    let status = response.status();
    let body = response.text().await.map_err(|e| {
        anyhow!(
            "failed to read Matrix createRoom response for Tuwunel admin room: {}",
            e
        )
    })?;

    if !status.is_success() {
        return Err(anyhow!(
            "Matrix createRoom returned status {} body {} for Tuwunel admin command room",
            status,
            body
        ));
    }

    let body_json: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
        anyhow!(
            "failed to parse Matrix createRoom response JSON for Tuwunel admin room body {}: {}",
            body,
            e
        )
    })?;
    let room_id = body_json["room_id"]
        .as_str()
        .ok_or_else(|| {
            anyhow!(
                "Matrix createRoom response missing room_id for Tuwunel admin room body {}",
                body
            )
        })?
        .to_string();

    tracing::warn!(
        admin_user_id = config.admin_user_id,
        admin_room_id = %room_id,
        "Created Matrix room for Tuwunel admin commands; set TUWUNEL_ADMIN_ROOM_ID to reuse it after restarts"
    );

    Ok(room_id)
}

async fn send_admin_room_command(
    config: &EventCleanupConfig,
    target: &AdminCommandTarget,
    job: &EventCleanupJob,
    kind: &'static str,
    command: &str,
) -> Result<SentAdminCommand> {
    let room_id = urlencoding::encode(&target.room_id);
    let txn_id = Uuid::new_v4().to_string();
    let url = format!(
        "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
        config.homeserver_url.trim_end_matches('/'),
        room_id,
        txn_id
    );

    tracing::debug!(
        user_id = job.user_id,
        ontology_message_id = job.ontology_message_id,
        service = %job.service,
        room_id = %job.room_id,
        source_event_id = %job.event_id,
        cleanup_command_kind = kind,
        admin_room_id = %target.room_id,
        admin_room_source = target.room_source,
        admin_auth_source = target.auth_source,
        admin_user_id = config.admin_user_id,
        homeserver_url = %config.homeserver_url,
        txn_id = %txn_id,
        attempt = job.attempt,
        "Sending Tuwunel cleanup admin command message"
    );

    let response = reqwest::Client::new()
        .put(url)
        .bearer_auth(&target.access_token)
        .json(&json!({
            "msgtype": "m.text",
            "body": command
        }))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| anyhow!("failed to send Matrix admin-room command: {}", e))?;

    let status = response.status();
    let body = response.text().await.map_err(|e| {
        anyhow!(
            "failed to read Matrix admin-room command response body: {}",
            e
        )
    })?;

    if !status.is_success() {
        return Err(anyhow!(
            "Matrix admin-room command returned status {} body {} for kind {} txn_id {} admin_room_id {}",
            status,
            body,
            kind,
            txn_id,
            target.room_id
        ));
    }

    let body_json: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
        anyhow!(
            "failed to parse Matrix admin-room command response JSON for kind {} txn_id {} body {}: {}",
            kind,
            txn_id,
            body,
            e
        )
    })?;
    let admin_command_event_id = body_json["event_id"]
        .as_str()
        .ok_or_else(|| {
            anyhow!(
                "Matrix admin-room command response missing event_id for kind {} txn_id {} body {}",
                kind,
                txn_id,
                body
            )
        })?
        .to_string();

    Ok(SentAdminCommand {
        kind,
        txn_id,
        admin_room_id: target.room_id.clone(),
        admin_room_source: target.room_source,
        admin_auth_source: target.auth_source,
        admin_command_event_id,
    })
}

fn retry_delay(attempt: u8) -> Duration {
    match attempt {
        0 | 1 => Duration::from_secs(30),
        2 => Duration::from_secs(120),
        3 => Duration::from_secs(300),
        _ => Duration::from_secs(900),
    }
}

impl EventCleanupConfig {
    fn from_env() -> Self {
        let homeserver_url =
            std::env::var("MATRIX_HOMESERVER").unwrap_or_else(|_| DEFAULT_HOMESERVER_URL.into());
        let admin_room_id = std::env::var("TUWUNEL_ADMIN_ROOM_ID")
            .map(|value| value.trim().to_string())
            .ok()
            .filter(|value| !value.is_empty());
        let admin_access_token = std::env::var("TUWUNEL_ADMIN_ACCESS_TOKEN")
            .map(|value| value.trim().to_string())
            .ok()
            .filter(|value| !value.is_empty());
        let admin_user_id = std::env::var("TUWUNEL_ADMIN_USER_ID")
            .ok()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|user_id| *user_id > 0)
            .unwrap_or(DEFAULT_ADMIN_USER_ID);
        let max_attempts = std::env::var("TUWUNEL_EVENT_CLEANUP_MAX_ATTEMPTS")
            .ok()
            .and_then(|value| value.parse::<u8>().ok())
            .filter(|attempts| *attempts > 0)
            .unwrap_or(DEFAULT_MAX_ATTEMPTS);

        Self {
            homeserver_url,
            admin_room_id,
            admin_access_token,
            admin_user_id,
            max_attempts,
        }
    }
}
