use anyhow::{anyhow, Result};
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

const QUEUE_CAPACITY: usize = 1024;
const DEFAULT_MAX_ATTEMPTS: u8 = 5;
const DEFAULT_HOMESERVER_URL: &str = "http://localhost:8008";

static CLEANUP_TX: OnceLock<mpsc::Sender<EventCleanupJob>> = OnceLock::new();
static DISABLED_LOGGED: AtomicBool = AtomicBool::new(false);
static MISSING_CONFIG_LOGGED: AtomicBool = AtomicBool::new(false);
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
    admin_room_id: String,
    admin_access_token: String,
    max_attempts: u8,
}

pub fn enqueue_processed_bridge_event(
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

    if let Err(e) = EventCleanupConfig::from_env() {
        log_missing_config_once(&e);
        tracing::debug!(
            user_id,
            service,
            room_id,
            event_id,
            ontology_message_id,
            delete_media,
            error = %e,
            "Skipping Tuwunel event cleanup because required config is missing"
        );
        return;
    }

    let tx = CLEANUP_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel(QUEUE_CAPACITY);
        tokio::spawn(event_cleanup_worker(rx, tx.clone()));
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

    match tx.try_send(job) {
        Ok(()) => {
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

fn log_missing_config_once(error: &anyhow::Error) {
    if !MISSING_CONFIG_LOGGED.swap(true, Ordering::Relaxed) {
        tracing::warn!(
            error = %error,
            "Tuwunel event cleanup config missing; bridge ingest will continue without cleanup"
        );
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

async fn event_cleanup_worker(
    mut rx: mpsc::Receiver<EventCleanupJob>,
    tx: mpsc::Sender<EventCleanupJob>,
) {
    while let Some(job) = rx.recv().await {
        let config = match EventCleanupConfig::from_env() {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!(
                    user_id = job.user_id,
                    ontology_message_id = job.ontology_message_id,
                    service = %job.service,
                    room_id = %job.room_id,
                    event_id = %job.event_id,
                    error = %e,
                    "Tuwunel event cleanup config became unavailable; dropping queued cleanup job"
                );
                continue;
            }
        };

        match send_cleanup_commands(&config, &job).await {
            Ok(sent_commands) => {
                for sent_command in &sent_commands {
                    tracing::info!(
                        user_id = job.user_id,
                        ontology_message_id = job.ontology_message_id,
                        service = %job.service,
                        room_id = %job.room_id,
                        source_event_id = %job.event_id,
                        cleanup_command_kind = sent_command.kind,
                        admin_room_id = %config.admin_room_id,
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
    admin_command_event_id: String,
}

async fn send_cleanup_commands(
    config: &EventCleanupConfig,
    job: &EventCleanupJob,
) -> Result<Vec<SentAdminCommand>> {
    let mut sent_commands = Vec::with_capacity(if job.delete_media { 2 } else { 1 });

    if job.delete_media {
        sent_commands.push(
            send_admin_room_command(
                config,
                job,
                "media_delete_by_event",
                &build_delete_media_by_event_command(&job.event_id),
            )
            .await
            .map_err(|e| anyhow!("media delete-by-event command failed: {}", e))?,
        );
    }

    sent_commands.push(
        send_admin_room_command(
            config,
            job,
            "redact_event",
            &build_redact_event_command(&job.event_id),
        )
        .await
        .map_err(|e| anyhow!("redact-event command failed: {}", e))?,
    );

    Ok(sent_commands)
}

async fn send_admin_room_command(
    config: &EventCleanupConfig,
    job: &EventCleanupJob,
    kind: &'static str,
    command: &str,
) -> Result<SentAdminCommand> {
    let room_id = urlencoding::encode(&config.admin_room_id);
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
        admin_room_id = %config.admin_room_id,
        homeserver_url = %config.homeserver_url,
        txn_id = %txn_id,
        attempt = job.attempt,
        "Sending Tuwunel cleanup admin command message"
    );

    let response = reqwest::Client::new()
        .put(url)
        .bearer_auth(&config.admin_access_token)
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
            config.admin_room_id
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
    fn from_env() -> Result<Self> {
        let homeserver_url =
            std::env::var("MATRIX_HOMESERVER").unwrap_or_else(|_| DEFAULT_HOMESERVER_URL.into());
        let admin_room_id = std::env::var("TUWUNEL_ADMIN_ROOM_ID")
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        let admin_access_token = std::env::var("TUWUNEL_ADMIN_ACCESS_TOKEN")
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        let max_attempts = std::env::var("TUWUNEL_EVENT_CLEANUP_MAX_ATTEMPTS")
            .ok()
            .and_then(|value| value.parse::<u8>().ok())
            .filter(|attempts| *attempts > 0)
            .unwrap_or(DEFAULT_MAX_ATTEMPTS);

        let mut missing = Vec::new();
        if admin_room_id.is_empty() {
            missing.push("TUWUNEL_ADMIN_ROOM_ID");
        }
        if admin_access_token.is_empty() {
            missing.push("TUWUNEL_ADMIN_ACCESS_TOKEN");
        }
        if !missing.is_empty() {
            return Err(anyhow!("missing required env vars: {}", missing.join(", ")));
        }

        Ok(Self {
            homeserver_url,
            admin_room_id,
            admin_access_token,
            max_attempts,
        })
    }
}
