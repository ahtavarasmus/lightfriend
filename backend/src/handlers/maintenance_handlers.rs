use axum::http::HeaderMap;
use axum::{
    body::Body,
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};

use crate::AppState;

/// Validates the X-Maintenance-Secret header against the MAINTENANCE_SECRET env var.
fn check_secret(headers: &HeaderMap) -> bool {
    let expected = match std::env::var("MAINTENANCE_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => return false,
    };
    headers
        .get("X-Maintenance-Secret")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == expected)
}

pub async fn enable_maintenance(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !check_secret(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    state.maintenance_mode.store(true, Ordering::SeqCst);
    tracing::warn!("Maintenance mode ENABLED - write operations will return 503");

    // Auto-disable after 30 minutes in case CI crashes and never sends disable
    let flag = state.maintenance_mode.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
        if flag.load(Ordering::SeqCst) {
            flag.store(false, Ordering::SeqCst);
            tracing::warn!("Maintenance mode AUTO-DISABLED after 30 minute timeout");
        }
    });

    Json(json!({"status": "maintenance_enabled"})).into_response()
}

pub async fn disable_maintenance(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !check_secret(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    state.maintenance_mode.store(false, Ordering::SeqCst);
    tracing::warn!("Maintenance mode DISABLED - normal operation resumed");
    Json(json!({"status": "maintenance_disabled"})).into_response()
}

pub async fn maintenance_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !check_secret(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }
    let enabled = state.maintenance_mode.load(Ordering::SeqCst);
    Json(json!({"maintenance_mode": enabled})).into_response()
}

/// Middleware that returns 503 for write operations when maintenance mode is active.
/// GET/HEAD/OPTIONS always pass through. Internal maintenance endpoints always pass through.
pub async fn maintenance_guard(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    // Always allow read-only methods
    if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
        return next.run(request).await;
    }

    // Always allow maintenance endpoints themselves
    if path.starts_with("/api/internal/maintenance") {
        return next.run(request).await;
    }

    // Always allow health check
    if path == "/api/health" {
        return next.run(request).await;
    }

    // Check maintenance mode for all other write operations
    if state.maintenance_mode.load(Ordering::SeqCst) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "error": "System update in progress. Please try again in 5-10 minutes.",
                "maintenance": true
            })),
        )
            .into_response();
    }

    next.run(request).await
}
