use axum::{
    extract::State,
    http::{header::CACHE_CONTROL, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use diesel::prelude::*;
use diesel::sql_types::{Integer, Nullable, Text};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use crate::AppState;

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const STORAGE_HEALTH_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_STORAGE_HEALTH_SCRIPT: &str = "/app/storage-health.sh";
const DEFAULT_BACKUP_STATUS_FILE: &str = "/data/seed/export-status.json";
const BACKUP_STALE_AFTER_SECONDS: u64 = 2 * 60 * 60;

#[derive(Serialize)]
pub struct DeepHealthResponse {
    pub db: String,
    pub matrix: String,
    pub twilio: String,
    pub telnyx: String,
    pub resend: String,
    pub overall: String,
    pub checked_at: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StorageHealthResponse {
    pub timestamp: String,
    pub filesystems: StorageFilesystems,
    pub reserve: StorageReserve,
    pub tuwunel: TuwunelStorage,
    pub postgres: StorageBytes,
    pub tuwunel_backup_engine: StorageBytes,
    pub supervisor_logs: StorageBytes,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hourly_backup: Option<HourlyBackupHealth>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StorageFilesystems {
    pub root: FilesystemMetrics,
    pub tmp: FilesystemMetrics,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FilesystemMetrics {
    pub size_kib: u64,
    pub used_kib: u64,
    pub avail_kib: u64,
    pub use_pct: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StorageReserve {
    #[serde(rename = "path", skip_serializing)]
    _path: Option<String>,
    pub present: bool,
    pub bytes: u64,
    pub projected_root_avail_kib: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TuwunelStorage {
    pub total_bytes: u64,
    pub media: StorageBucket,
    pub rocksdb_sst: StorageBucket,
    pub rocksdb_archive_log: StorageBucket,
    pub rocksdb_meta_logs: StorageBucket,
    pub other: StorageBucket,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StorageBucket {
    pub count: u64,
    pub bytes: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StorageBytes {
    pub bytes: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HourlyBackupHealth {
    pub status: String,
    pub last_attempt_at: Option<String>,
    pub age_seconds: Option<u64>,
    pub stale: bool,
    pub failed_step: Option<String>,
}

#[derive(Deserialize)]
struct ExportStatusRecord {
    status: Option<String>,
    timestamp: Option<String>,
    step: Option<String>,
}

/// Deep health probe. Protected by `X-Maintenance-Secret` header (same secret as
/// the maintenance toggle endpoints). Returns JSON with the status of each
/// external dependency. The hourly external watchdog polls this.
pub async fn deep_health(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !crate::handlers::maintenance_handlers::check_secret(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "forbidden"})),
        )
            .into_response();
    }

    let http = match reqwest::Client::builder().timeout(PROBE_TIMEOUT).build() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("http client: {}", e)})),
            )
                .into_response();
        }
    };

    let (db, matrix, twilio, telnyx, resend) = tokio::join!(
        probe_db(state.clone()),
        probe_matrix(&http),
        probe_twilio(&http),
        probe_telnyx(&http),
        probe_resend(),
    );

    let components = [&db, &matrix, &twilio, &resend];
    let overall = if components.iter().all(|s| s.starts_with("ok")) {
        if telnyx.starts_with("ok") || telnyx == "not_configured" {
            "ok"
        } else {
            "degraded"
        }
    } else {
        "degraded"
    };

    let resp = DeepHealthResponse {
        db,
        matrix,
        twilio,
        telnyx,
        resend,
        overall: overall.to_string(),
        checked_at: chrono::Utc::now().timestamp(),
    };

    let status = if overall == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(resp)).into_response()
}

/// Current aggregate enclave storage metrics. This intentionally exposes only
/// the sanitized JSON counters from storage-health.sh, not the full diagnostic
/// report containing internal paths, room IDs, or logs.
pub async fn storage_health(headers: HeaderMap) -> Response {
    if !crate::handlers::maintenance_handlers::check_secret(&headers) {
        return no_store_json(
            StatusCode::FORBIDDEN,
            serde_json::json!({"error": "forbidden"}),
        );
    }

    let script = std::env::var("STORAGE_HEALTH_SCRIPT")
        .unwrap_or_else(|_| DEFAULT_STORAGE_HEALTH_SCRIPT.to_string());
    let mut command = tokio::process::Command::new(&script);
    command.arg("json").kill_on_drop(true);

    let output = match tokio::time::timeout(STORAGE_HEALTH_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            tracing::warn!(script = %script, %error, "storage health command failed to start");
            return no_store_json(
                StatusCode::SERVICE_UNAVAILABLE,
                serde_json::json!({"error": "storage_metrics_unavailable"}),
            );
        }
        Err(_) => {
            tracing::warn!(script = %script, "storage health command timed out");
            return no_store_json(
                StatusCode::GATEWAY_TIMEOUT,
                serde_json::json!({"error": "storage_metrics_timeout"}),
            );
        }
    };

    if !output.status.success() {
        tracing::warn!(
            script = %script,
            exit_code = output.status.code(),
            "storage health command returned a failure"
        );
        return no_store_json(
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({"error": "storage_metrics_unavailable"}),
        );
    }

    match parse_storage_health_json(&output.stdout) {
        Ok(mut metrics) => {
            metrics.hourly_backup = Some(read_hourly_backup_health().await);
            no_store_json(StatusCode::OK, metrics)
        }
        Err(error) => {
            tracing::warn!(%error, "storage health command returned invalid JSON");
            no_store_json(
                StatusCode::SERVICE_UNAVAILABLE,
                serde_json::json!({"error": "storage_metrics_invalid"}),
            )
        }
    }
}

pub fn parse_storage_health_json(stdout: &[u8]) -> Result<StorageHealthResponse, String> {
    serde_json::from_slice(stdout).map_err(|error| error.to_string())
}

pub fn parse_hourly_backup_status_json(input: &[u8], now: DateTime<Utc>) -> HourlyBackupHealth {
    let record = match serde_json::from_slice::<ExportStatusRecord>(input) {
        Ok(record) => record,
        Err(_) => return unknown_hourly_backup_health(),
    };

    let status = match record.status.as_deref() {
        Some(value) if value.eq_ignore_ascii_case("SUCCESS") => "success",
        Some(value) if value.eq_ignore_ascii_case("FAILED") => "failed",
        _ => "unknown",
    };
    let attempted_at = record.timestamp.as_deref().and_then(parse_export_timestamp);
    let age_seconds = attempted_at
        .map(|timestamp| now.signed_duration_since(timestamp).num_seconds().max(0) as u64);
    let stale = status == "unknown"
        || age_seconds
            .map(|age| age > BACKUP_STALE_AFTER_SECONDS)
            .unwrap_or(true);
    let failed_step = if status == "failed" {
        record.step.as_deref().and_then(sanitize_backup_step)
    } else {
        None
    };

    HourlyBackupHealth {
        status: status.to_string(),
        last_attempt_at: attempted_at
            .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)),
        age_seconds,
        stale,
        failed_step,
    }
}

async fn read_hourly_backup_health() -> HourlyBackupHealth {
    let path = std::env::var("LIGHTFRIEND_BACKUP_STATUS_FILE")
        .unwrap_or_else(|_| DEFAULT_BACKUP_STATUS_FILE.to_string());
    match tokio::fs::read(path).await {
        Ok(input) => parse_hourly_backup_status_json(&input, Utc::now()),
        Err(_) => unknown_hourly_backup_health(),
    }
}

fn parse_export_timestamp(value: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ")
        .ok()
        .map(|timestamp| timestamp.and_utc())
}

fn sanitize_backup_step(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return None;
    }
    Some(value.to_string())
}

fn unknown_hourly_backup_health() -> HourlyBackupHealth {
    HourlyBackupHealth {
        status: "unknown".to_string(),
        last_attempt_at: None,
        age_seconds: None,
        stale: true,
        failed_step: None,
    }
}

fn no_store_json<T: Serialize>(status: StatusCode, body: T) -> Response {
    let mut response = (status, Json(body)).into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn probe_db(state: Arc<AppState>) -> String {
    let result = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut conn = state.pg_pool.get().map_err(|e| format!("pool: {}", e))?;
        diesel::sql_query("SELECT 1")
            .execute(&mut conn)
            .map_err(|e| format!("query: {}", e))?;
        Ok(())
    })
    .await;

    match result {
        Ok(Ok(())) => "ok".to_string(),
        Ok(Err(e)) => format!("fail: {}", e),
        Err(e) => format!("fail: join {}", e),
    }
}

async fn probe_matrix(http: &reqwest::Client) -> String {
    let url = match std::env::var("MATRIX_HOMESERVER") {
        Ok(u) if !u.is_empty() => u,
        _ => return "fail: MATRIX_HOMESERVER not set".to_string(),
    };
    let url = format!("{}/_matrix/client/versions", url.trim_end_matches('/'));
    match http.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => "ok".to_string(),
        Ok(resp) => format!("fail: status {}", resp.status()),
        Err(e) => format!("fail: {}", short_err(&e)),
    }
}

async fn probe_twilio(http: &reqwest::Client) -> String {
    let sid = match std::env::var("TWILIO_ACCOUNT_SID") {
        Ok(s) if !s.is_empty() => s,
        _ => return "fail: TWILIO_ACCOUNT_SID not set".to_string(),
    };
    let token = match std::env::var("TWILIO_AUTH_TOKEN") {
        Ok(s) if !s.is_empty() => s,
        _ => return "fail: TWILIO_AUTH_TOKEN not set".to_string(),
    };
    let url = format!("https://api.twilio.com/2010-04-01/Accounts/{}.json", sid);
    match http.get(&url).basic_auth(&sid, Some(&token)).send().await {
        Ok(resp) if resp.status().is_success() => "ok".to_string(),
        Ok(resp) => format!("fail: status {}", resp.status()),
        Err(e) => format!("fail: {}", short_err(&e)),
    }
}

async fn probe_telnyx(http: &reqwest::Client) -> String {
    let key = match std::env::var("TELNYX_API_KEY") {
        Ok(s) if !s.is_empty() => s,
        _ => return "not_configured".to_string(),
    };
    let url = "https://api.telnyx.com/v2/messaging_profiles?page[size]=1";
    match http.get(url).bearer_auth(&key).send().await {
        Ok(resp) if resp.status().is_success() => "ok".to_string(),
        Ok(resp) => format!("fail: status {}", resp.status()),
        Err(e) => format!("fail: {}", short_err(&e)),
    }
}

async fn probe_resend() -> String {
    match std::env::var("RESEND_API_KEY") {
        Ok(s) if !s.is_empty() => "ok".to_string(),
        _ => "fail: RESEND_API_KEY not set".to_string(),
    }
}

fn short_err(e: &reqwest::Error) -> String {
    let msg = e.to_string();
    if msg.len() > 120 {
        format!("{}...", &msg[..120])
    } else {
        msg
    }
}

// ============================================================================
// Daily alert digest
// ============================================================================

#[derive(Serialize)]
pub struct AlertGroup {
    pub alert_type: String,
    pub severity: String,
    pub count: i64,
    pub last_at: i32,
}

#[derive(Serialize)]
pub struct FailedSmsGroup {
    pub error_code: String,
    pub decoded_title: String,
    pub count: i64,
    pub last_at: i32,
}

#[derive(Serialize)]
pub struct DigestResponse {
    pub period_hours: i32,
    pub generated_at: i64,
    pub admin_alerts_total: i64,
    pub admin_alerts: Vec<AlertGroup>,
    pub failed_sms_total: i64,
    pub failed_sms: Vec<FailedSmsGroup>,
}

#[derive(QueryableByName)]
struct AlertGroupRow {
    #[diesel(sql_type = Text)]
    alert_type: String,
    #[diesel(sql_type = Text)]
    severity: String,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    count: i64,
    #[diesel(sql_type = Integer)]
    last_at: i32,
}

#[derive(QueryableByName)]
struct FailedSmsRow {
    #[diesel(sql_type = Nullable<Text>)]
    error_code: Option<String>,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    count: i64,
    #[diesel(sql_type = Integer)]
    last_at: i32,
}

/// Digest of the last N hours of admin_alerts and failed SMS, grouped by type.
/// Used by the daily digest GitHub Action to produce a single rollup email
/// instead of one alert email per failure event.
pub async fn alerts_digest(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !crate::handlers::maintenance_handlers::check_secret(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "forbidden"})),
        )
            .into_response();
    }

    let period_hours: i32 = headers
        .get("X-Period-Hours")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(24);
    let cutoff = chrono::Utc::now().timestamp() as i32 - period_hours * 3600;

    let result = tokio::task::spawn_blocking(move || -> Result<DigestResponse, String> {
        let mut conn = state.pg_pool.get().map_err(|e| format!("pool: {}", e))?;

        let alert_rows: Vec<AlertGroupRow> = diesel::sql_query(
            "SELECT alert_type, severity, COUNT(*) AS count, MAX(created_at) AS last_at \
             FROM admin_alerts WHERE created_at >= $1 \
             GROUP BY alert_type, severity \
             ORDER BY count DESC, last_at DESC",
        )
        .bind::<Integer, _>(cutoff)
        .load(&mut conn)
        .map_err(|e| format!("admin_alerts query: {}", e))?;

        let sms_rows: Vec<FailedSmsRow> = diesel::sql_query(
            "SELECT error_code, COUNT(*) AS count, MAX(created_at) AS last_at \
             FROM message_status_log \
             WHERE created_at >= $1 AND status IN ('failed', 'undelivered') \
             GROUP BY error_code \
             ORDER BY count DESC",
        )
        .bind::<Integer, _>(cutoff)
        .load(&mut conn)
        .map_err(|e| format!("message_status_log query: {}", e))?;

        let admin_alerts_total: i64 = alert_rows.iter().map(|r| r.count).sum();
        let failed_sms_total: i64 = sms_rows.iter().map(|r| r.count).sum();

        let admin_alerts = alert_rows
            .into_iter()
            .map(|r| AlertGroup {
                alert_type: r.alert_type,
                severity: r.severity,
                count: r.count,
                last_at: r.last_at,
            })
            .collect();

        let failed_sms = sms_rows
            .into_iter()
            .map(|r| {
                let code = r.error_code.unwrap_or_else(|| "unknown".to_string());
                let decoded_title = crate::utils::twilio_error_codes::decode(&code)
                    .map(|c| c.title.to_string())
                    .unwrap_or_else(|| "Unknown error code".to_string());
                FailedSmsGroup {
                    error_code: code,
                    decoded_title,
                    count: r.count,
                    last_at: r.last_at,
                }
            })
            .collect();

        Ok(DigestResponse {
            period_hours,
            generated_at: chrono::Utc::now().timestamp(),
            admin_alerts_total,
            admin_alerts,
            failed_sms_total,
            failed_sms,
        })
    })
    .await;

    match result {
        Ok(Ok(resp)) => (StatusCode::OK, Json(resp)).into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("join: {}", e)})),
        )
            .into_response(),
    }
}

// ============================================================================
// 6h SMS carrier-noise digest
// ============================================================================

#[derive(QueryableByName)]
struct CarrierNoiseRow {
    #[diesel(sql_type = Nullable<Text>)]
    error_code: Option<String>,
    #[diesel(sql_type = Text)]
    to_number: String,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    count: i64,
    #[diesel(sql_type = Integer)]
    last_at: i32,
}

/// Send the 6h carrier-noise digest email. Triggered by the cron GitHub Action.
///
/// Queries `message_status_log` for failed/undelivered rows in the last N hours
/// (default 6, override via `X-Period-Hours`), filters to CarrierNoise codes,
/// groups by (error_code, to_number), and ships a single email via Resend with
/// the affected masked numbers in the subject line.
pub async fn sms_failures_digest_send(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !crate::handlers::maintenance_handlers::check_secret(&headers) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "forbidden"})),
        )
            .into_response();
    }

    let period_hours: i32 = headers
        .get("X-Period-Hours")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    let cutoff = chrono::Utc::now().timestamp() as i32 - period_hours * 3600;

    // Pull all failed/undelivered rows in the window, grouped by (code, to).
    let state_for_query = state.clone();
    let rows_result =
        tokio::task::spawn_blocking(move || -> Result<Vec<CarrierNoiseRow>, String> {
            let mut conn = state_for_query
                .pg_pool
                .get()
                .map_err(|e| format!("pool: {}", e))?;
            let rows: Vec<CarrierNoiseRow> = diesel::sql_query(
                "SELECT error_code, to_number, COUNT(*) AS count, MAX(created_at) AS last_at \
             FROM message_status_log \
             WHERE created_at >= $1 AND status IN ('failed', 'undelivered') \
             GROUP BY error_code, to_number \
             ORDER BY count DESC, last_at DESC",
            )
            .bind::<Integer, _>(cutoff)
            .load(&mut conn)
            .map_err(|e| format!("message_status_log query: {}", e))?;
            Ok(rows)
        })
        .await;

    let rows = match rows_result {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("join: {}", e)})),
            )
                .into_response();
        }
    };

    // Filter to CarrierNoise codes only. Actionable codes are already SMS-pushed
    // per-event; including them here would be double-reporting.
    let digest_rows: Vec<crate::utils::email::DigestRow> = rows
        .into_iter()
        .filter(|r| !crate::utils::twilio_error_codes::is_actionable(r.error_code.as_deref()))
        .map(|r| crate::utils::email::DigestRow {
            error_code: r.error_code,
            to_number: r.to_number,
            count: r.count,
            last_at: r.last_at,
        })
        .collect();

    let row_count = digest_rows.len();
    let total: i64 = digest_rows.iter().map(|r| r.count).sum();

    if digest_rows.is_empty() {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "sent": false,
                "reason": "no carrier-noise failures in window",
                "period_hours": period_hours,
            })),
        )
            .into_response();
    }

    match crate::utils::email::send_sms_failure_digest_email(&digest_rows, period_hours).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "sent": true,
                "period_hours": period_hours,
                "rows": row_count,
                "total_failures": total,
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("send digest: {}", e)})),
        )
            .into_response(),
    }
}
