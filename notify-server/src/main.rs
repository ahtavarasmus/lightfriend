use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

mod config;
mod sms;
mod email;

use config::Config;

#[derive(Clone)]
struct AppState {
    cfg: Arc<Config>,
    http: reqwest::Client,
    dedup: Arc<DashMap<String, Instant>>,
}

#[derive(Debug, Deserialize)]
struct AlertRequest {
    severity: String,
    title: String,
    body: String,
    #[serde(default)]
    dedup_key: Option<String>,
}

#[derive(Debug, Serialize)]
struct AlertResponse {
    status: &'static str,
    sms_sent: bool,
    deduped: bool,
}

#[derive(Debug, Deserialize)]
struct DigestRequest {
    subject: String,
    plain_body: String,
    #[serde(default)]
    html_body: Option<String>,
}

#[derive(Debug, Serialize)]
struct DigestResponse {
    status: &'static str,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = Arc::new(Config::from_env()?);
    let bind_addr: SocketAddr = cfg
        .bind_addr
        .parse()
        .with_context(|| format!("invalid BIND_ADDR: {}", cfg.bind_addr))?;

    let state = AppState {
        cfg: cfg.clone(),
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .context("build http client")?,
        dedup: Arc::new(DashMap::new()),
    };

    spawn_dedup_gc(state.dedup.clone());

    let app = Router::new()
        .route("/health", get(health))
        .route("/alert", post(alert))
        .route("/digest", post(digest))
        .with_state(state);

    tracing::info!("notify-server listening on {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
}

async fn health() -> &'static str {
    "OK"
}

async fn alert(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AlertRequest>,
) -> impl IntoResponse {
    if let Err(e) = check_auth(&state, &headers) {
        return (StatusCode::UNAUTHORIZED, Json(json_err(&e))).into_response();
    }

    let severity = req.severity.to_ascii_lowercase();
    let dedup_key = req.dedup_key.unwrap_or_else(|| req.title.clone());
    let dedup_ttl = match severity.as_str() {
        "critical" => state.cfg.dedup_ttl_critical,
        "error" => state.cfg.dedup_ttl_error,
        _ => state.cfg.dedup_ttl_warning,
    };

    if is_deduped(&state.dedup, &dedup_key, dedup_ttl) {
        tracing::info!(
            severity = %severity,
            dedup_key = %dedup_key,
            "alert deduped (within TTL)"
        );
        return Json(AlertResponse {
            status: "deduped",
            sms_sent: false,
            deduped: true,
        })
        .into_response();
    }
    state.dedup.insert(dedup_key.clone(), Instant::now());

    let sms_eligible = severity == "critical";
    let mut sms_sent = false;

    if sms_eligible {
        let sms_text = format_sms_text(&severity, &req.title, &req.body);
        match sms::send(&state.http, &state.cfg, &sms_text).await {
            Ok(_) => {
                sms_sent = true;
                tracing::info!(title = %req.title, "SMS alert sent");
            }
            Err(e) => {
                tracing::error!(error = ?e, "SMS send failed");
            }
        }
    } else {
        tracing::info!(severity = %severity, title = %req.title, "alert received but severity below SMS threshold");
    }

    Json(AlertResponse {
        status: "ok",
        sms_sent,
        deduped: false,
    })
    .into_response()
}

async fn digest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DigestRequest>,
) -> impl IntoResponse {
    if let Err(e) = check_auth(&state, &headers) {
        return (StatusCode::UNAUTHORIZED, Json(json_err(&e))).into_response();
    }

    match email::send_digest(&state.http, &state.cfg, &req.subject, &req.plain_body, req.html_body.as_deref()).await {
        Ok(_) => Json(DigestResponse { status: "ok" }).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "digest email failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json_err(&format!("digest failed: {}", e))),
            )
                .into_response()
        }
    }
}

fn check_auth(state: &AppState, headers: &HeaderMap) -> Result<(), String> {
    let auth = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| "missing Authorization header".to_string())?;
    let expected = format!("Bearer {}", state.cfg.bearer_token);
    if !constant_time_eq(auth.as_bytes(), expected.as_bytes()) {
        return Err("invalid bearer token".to_string());
    }
    Ok(())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn is_deduped(map: &DashMap<String, Instant>, key: &str, ttl: Duration) -> bool {
    if let Some(entry) = map.get(key) {
        if entry.elapsed() < ttl {
            return true;
        }
    }
    false
}

fn spawn_dedup_gc(map: Arc<DashMap<String, Instant>>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            let cutoff = Duration::from_secs(48 * 3600);
            map.retain(|_, ts| ts.elapsed() < cutoff);
        }
    });
}

fn format_sms_text(severity: &str, title: &str, body: &str) -> String {
    let max_len = 1200;
    let prefix = match severity {
        "critical" => "[CRIT]",
        "error" => "[ERR]",
        _ => "[WARN]",
    };
    let combined = format!("{} {}\n{}", prefix, title, body);
    if combined.len() <= max_len {
        combined
    } else {
        let mut truncated = combined;
        truncated.truncate(max_len - 3);
        truncated.push_str("...");
        truncated
    }
}

fn json_err(msg: &str) -> serde_json::Value {
    serde_json::json!({ "error": msg })
}
