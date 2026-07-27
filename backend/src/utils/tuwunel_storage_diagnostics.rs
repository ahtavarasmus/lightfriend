use crate::AppState;
use anyhow::{anyhow, Context, Result};
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use matrix_sdk::ruma::{OwnedRoomAliasId, UInt};
use matrix_sdk::Room;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

const COMMAND: &str = "!admin debug database-files";
const DEFAULT_ADMIN_USER_ID: i32 = 1;
const DEFAULT_INITIAL_DELAY_SECS: u64 = 20;
const DEFAULT_REFRESH_SECS: u64 = 300;
const DEFAULT_RESPONSE_TIMEOUT_SECS: u64 = 20;
const DEFAULT_SNAPSHOT_FILE: &str = "/tmp/tuwunel-rocksdb-database-files.md";
const MESSAGE_PAGE_SIZE: u64 = 100;
const RESPONSE_SETTLE_TIME: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
struct Config {
    enabled: bool,
    admin_user_id: i32,
    initial_delay: Duration,
    refresh_interval: Duration,
    response_timeout: Duration,
    snapshot_file: PathBuf,
}

#[derive(Debug)]
struct ReplyEvent {
    event_id: String,
    origin_server_ts: u64,
    body: String,
    relation_target: String,
}

pub async fn start_tuwunel_storage_diagnostics_worker(state: Arc<AppState>) {
    let config = Config::from_env();
    if !config.enabled {
        tracing::info!("Tuwunel RocksDB diagnostics worker disabled");
        return;
    }

    tracing::info!(
        admin_user_id = config.admin_user_id,
        initial_delay_secs = config.initial_delay.as_secs(),
        refresh_secs = config.refresh_interval.as_secs(),
        response_timeout_secs = config.response_timeout.as_secs(),
        snapshot_file = %config.snapshot_file.display(),
        "Tuwunel RocksDB diagnostics worker enabled"
    );
    tokio::time::sleep(config.initial_delay).await;

    loop {
        if let Err(error) = refresh_snapshot(&state, &config).await {
            tracing::warn!(
                error = %error,
                snapshot_file = %config.snapshot_file.display(),
                "Tuwunel RocksDB diagnostics refresh failed; retaining last valid snapshot"
            );
        }
        tokio::time::sleep(config.refresh_interval).await;
    }
}

async fn refresh_snapshot(state: &Arc<AppState>, config: &Config) -> Result<()> {
    let client = crate::utils::matrix_auth::get_cached_client(config.admin_user_id, state).await?;
    let admin_user = client
        .user_id()
        .ok_or_else(|| anyhow!("configured admin client has no authenticated user"))?;
    let server_name = admin_user.server_name();
    let server_user = format!("@conduit:{server_name}");
    let alias = OwnedRoomAliasId::try_from(format!("#admins:{server_name}"))
        .context("invalid Tuwunel admin room alias")?;
    let resolved = client
        .resolve_room_alias(&alias)
        .await
        .context("could not resolve Tuwunel admin room")?;
    let room = client
        .get_room(&resolved.room_id)
        .ok_or_else(|| anyhow!("Tuwunel admin room is not visible to configured admin user"))?;

    let sent = room
        .send(RoomMessageEventContent::text_plain(COMMAND))
        .await
        .context("could not send Tuwunel database-files command")?;
    let report = await_database_files_report(
        &room,
        sent.event_id.as_str(),
        &server_user,
        config.response_timeout,
    )
    .await?;
    write_snapshot_atomically(&config.snapshot_file, &report).await?;

    tracing::info!(
        room_id = %resolved.room_id,
        command_event_id = %sent.event_id,
        report_bytes = report.len(),
        snapshot_file = %config.snapshot_file.display(),
        "Refreshed verified Tuwunel RocksDB database-files snapshot"
    );
    Ok(())
}

async fn await_database_files_report(
    room: &Room,
    command_event_id: &str,
    server_user: &str,
    response_timeout: Duration,
) -> Result<String> {
    let deadline = Instant::now() + response_timeout;
    let mut candidate: Option<String> = None;
    let mut candidate_seen_at = None;

    while Instant::now() < deadline {
        let mut options = matrix_sdk::room::MessagesOptions::backward();
        options.limit = UInt::new(MESSAGE_PAGE_SIZE).expect("valid Matrix message page size");
        let messages = tokio::time::timeout(REQUEST_TIMEOUT, room.messages(options))
            .await
            .map_err(|_| anyhow!("Tuwunel admin room history request timed out"))?
            .context("could not read Tuwunel admin room history")?;
        let events = messages
            .chunk
            .iter()
            .filter_map(|event| event.raw().deserialize_as::<Value>().ok())
            .collect::<Vec<_>>();

        if let Some(report) =
            extract_correlated_database_files_report(&events, command_event_id, server_user)
        {
            if candidate.as_ref() != Some(&report) {
                candidate = Some(report);
                candidate_seen_at = Some(Instant::now());
            } else if candidate_seen_at.is_some_and(|seen| seen.elapsed() >= RESPONSE_SETTLE_TIME) {
                return Ok(candidate.expect("candidate exists after stability check"));
            }
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }

    candidate.ok_or_else(|| anyhow!("no valid correlated database-files response before timeout"))
}

pub fn extract_correlated_database_files_report(
    events: &[Value],
    command_event_id: &str,
    server_user: &str,
) -> Option<String> {
    let mut replies = HashMap::new();
    for event in events {
        let Some(reply) = parse_reply_event(event, server_user) else {
            continue;
        };
        replies.entry(reply.event_id.clone()).or_insert(reply);
    }

    let mut reachable = HashSet::from([command_event_id.to_string()]);
    let mut selected = HashSet::new();
    loop {
        let mut changed = false;
        for reply in replies.values() {
            if !selected.contains(&reply.event_id) && reachable.contains(&reply.relation_target) {
                reachable.insert(reply.event_id.clone());
                selected.insert(reply.event_id.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut selected_replies = replies
        .values()
        .filter(|reply| selected.contains(&reply.event_id))
        .collect::<Vec<_>>();
    selected_replies.sort_by(|left, right| {
        left.origin_server_ts
            .cmp(&right.origin_server_ts)
            .then_with(|| left.event_id.cmp(&right.event_id))
    });
    let combined = selected_replies
        .into_iter()
        .map(|reply| reply.body.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    extract_database_files_table(&combined)
}

fn parse_reply_event(event: &Value, server_user: &str) -> Option<ReplyEvent> {
    if event.get("type")?.as_str()? != "m.room.message"
        || event.get("sender")?.as_str()? != server_user
    {
        return None;
    }
    let content = event.get("content")?;
    if !matches!(content.get("msgtype")?.as_str()?, "m.notice" | "m.text") {
        return None;
    }
    let relates_to = content.get("m.relates_to")?;
    let relation_target = relates_to
        .get("m.in_reply_to")
        .and_then(|reply| reply.get("event_id"))
        .and_then(Value::as_str)
        .or_else(|| {
            (relates_to.get("rel_type").and_then(Value::as_str) == Some("m.thread"))
                .then(|| relates_to.get("event_id").and_then(Value::as_str))
                .flatten()
        })?;

    Some(ReplyEvent {
        event_id: event.get("event_id")?.as_str()?.to_string(),
        origin_server_ts: event.get("origin_server_ts")?.as_u64()?,
        body: content.get("body")?.as_str()?.to_string(),
        relation_target: relation_target.to_string(),
    })
}

fn extract_database_files_table(text: &str) -> Option<String> {
    let lines = text.lines().collect::<Vec<_>>();
    for (header_index, line) in lines.iter().enumerate() {
        if markdown_fields(line) != ["lev", "sst", "keys", "dels", "size", "column"] {
            continue;
        }

        let mut report = vec![line.trim().to_string()];
        let mut data_rows = 0;
        for line in lines.iter().skip(header_index + 1) {
            let fields = markdown_fields(line);
            if is_markdown_separator(&fields) && data_rows == 0 {
                report.push(line.trim().to_string());
                continue;
            }
            if is_database_file_row(&fields) {
                report.push(line.trim().to_string());
                data_rows += 1;
                continue;
            }
            if data_rows > 0 {
                break;
            }
        }
        if data_rows > 0 {
            return Some(format!("{}\n", report.join("\n")));
        }
    }
    None
}

fn markdown_fields(line: &str) -> Vec<&str> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect()
}

fn is_markdown_separator(fields: &[&str]) -> bool {
    fields.len() == 6
        && fields.iter().all(|field| {
            let field = field.trim_matches(':');
            field.len() >= 3 && field.chars().all(|character| character == '-')
        })
}

fn is_database_file_row(fields: &[&str]) -> bool {
    fields.len() == 6
        && fields[0].parse::<u32>().is_ok()
        && !fields[1].is_empty()
        && fields[2]
            .strip_suffix('+')
            .is_some_and(|value| value.trim().parse::<u64>().is_ok())
        && fields[3]
            .strip_suffix('-')
            .is_some_and(|value| value.trim().parse::<u64>().is_ok())
        && fields[4].parse::<u64>().is_ok()
        && !fields[5].is_empty()
}

async fn write_snapshot_atomically(path: &Path, report: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("snapshot path has no parent"))?;
    tokio::fs::create_dir_all(parent)
        .await
        .with_context(|| format!("could not create snapshot directory {}", parent.display()))?;
    let temp_path = path.with_extension(format!("tmp.{}", std::process::id()));
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .with_context(|| format!("could not create {}", temp_path.display()))?;
    file.write_all(report.as_bytes())
        .await
        .context("could not write Tuwunel diagnostics snapshot")?;
    file.sync_all()
        .await
        .context("could not sync Tuwunel diagnostics snapshot")?;
    drop(file);
    tokio::fs::rename(&temp_path, path)
        .await
        .with_context(|| format!("could not publish snapshot {}", path.display()))?;
    Ok(())
}

impl Config {
    fn from_env() -> Self {
        Self {
            enabled: env_bool("TUWUNEL_ROCKSDB_DIAGNOSTICS_ENABLED", false),
            admin_user_id: env_i32("TUWUNEL_ADMIN_USER_ID", DEFAULT_ADMIN_USER_ID, 1),
            initial_delay: Duration::from_secs(env_u64(
                "TUWUNEL_ROCKSDB_DIAGNOSTICS_INITIAL_DELAY_SECS",
                DEFAULT_INITIAL_DELAY_SECS,
                1,
            )),
            refresh_interval: Duration::from_secs(env_u64(
                "TUWUNEL_ROCKSDB_DIAGNOSTICS_REFRESH_SECS",
                DEFAULT_REFRESH_SECS,
                30,
            )),
            response_timeout: Duration::from_secs(env_u64(
                "TUWUNEL_ROCKSDB_DIAGNOSTICS_RESPONSE_TIMEOUT_SECS",
                DEFAULT_RESPONSE_TIMEOUT_SECS,
                5,
            )),
            snapshot_file: std::env::var("TUWUNEL_ROCKSDB_DIAGNOSTICS_SNAPSHOT_FILE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(DEFAULT_SNAPSHOT_FILE)),
        }
    }
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

fn env_i32(name: &str, default: i32, minimum: i32) -> i32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|value| *value >= minimum)
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64, minimum: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value >= minimum)
        .unwrap_or(default)
}
