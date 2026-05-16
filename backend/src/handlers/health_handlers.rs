use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use diesel::prelude::*;
use diesel::sql_types::{Integer, Nullable, Text};
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;

use crate::AppState;

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

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
