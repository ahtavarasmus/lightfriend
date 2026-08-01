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

const ADMIN_HISTORY_PURGE_SERVICE: &str = "admin_room_manual";
const DEFAULT_ADMIN_HISTORY_RETENTION_SECS: u64 = 60;

/// Validates the X-Maintenance-Secret header against the MAINTENANCE_SECRET env var.
pub fn check_secret(headers: &HeaderMap) -> bool {
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

pub async fn compact_tuwunel(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !check_secret(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }

    let admin_user_id = std::env::var("TUWUNEL_ADMIN_USER_ID")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(1);
    let client = match crate::utils::matrix_auth::get_cached_client(admin_user_id, &state).await {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(admin_user_id, error = %error, "Tuwunel compaction admin client unavailable");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": format!("admin client unavailable: {error}")})),
            )
                .into_response();
        }
    };
    let Some(admin_user) = client.user_id() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "admin client has no authenticated user"})),
        )
            .into_response();
    };
    let alias = format!("#admins:{}", admin_user.server_name());
    let alias = match matrix_sdk::ruma::OwnedRoomAliasId::try_from(alias) {
        Ok(alias) => alias,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("invalid admin room alias: {error}")})),
            )
                .into_response();
        }
    };
    let resolved = match client.resolve_room_alias(&alias).await {
        Ok(resolved) => resolved,
        Err(error) => {
            tracing::error!(alias = %alias, error = %error, "Could not resolve Tuwunel admin room");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": format!("admin room unavailable: {error}")})),
            )
                .into_response();
        }
    };
    let Some(room) = client.get_room(&resolved.room_id) else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "admin room is not visible to the configured admin user"})),
        )
            .into_response();
    };
    let command = "!admin query raw compact --parallelism 1 --exhaustive";
    let sent = match room
        .send(matrix_sdk::ruma::events::room::message::RoomMessageEventContent::text_plain(command))
        .await
    {
        Ok(sent) => sent,
        Err(error) => {
            tracing::error!(room_id = %resolved.room_id, error = %error, "Failed to submit Tuwunel compaction command");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": format!("compaction command failed: {error}")})),
            )
                .into_response();
        }
    };

    tracing::warn!(
        room_id = %resolved.room_id,
        event_id = %sent.event_id,
        command,
        "Submitted guarded Tuwunel RocksDB compaction after verified backup"
    );
    (
        StatusCode::ACCEPTED,
        Json(json!({
            "status": "accepted",
            "room_id": resolved.room_id,
            "event_id": sent.event_id,
            "command": command
        })),
    )
        .into_response()
}

pub async fn purge_tuwunel_admin_history(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !check_secret(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }

    let admin_user_id = std::env::var("TUWUNEL_ADMIN_USER_ID")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(1);
    let retention_secs = std::env::var("TUWUNEL_ADMIN_HISTORY_RETENTION_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_ADMIN_HISTORY_RETENTION_SECS);
    let client = match crate::utils::matrix_auth::get_cached_client(admin_user_id, &state).await {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(admin_user_id, error = %error, "Tuwunel admin history purge client unavailable");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": format!("admin client unavailable: {error}")})),
            )
                .into_response();
        }
    };
    let Some(admin_user) = client.user_id() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "admin client has no authenticated user"})),
        )
            .into_response();
    };
    let alias = match matrix_sdk::ruma::OwnedRoomAliasId::try_from(format!(
        "#admins:{}",
        admin_user.server_name()
    )) {
        Ok(alias) => alias,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("invalid admin room alias: {error}")})),
            )
                .into_response();
        }
    };
    let resolved = match client.resolve_room_alias(&alias).await {
        Ok(resolved) => resolved,
        Err(error) => {
            tracing::error!(alias = %alias, error = %error, "Could not resolve Tuwunel admin room for history purge");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": format!("admin room unavailable: {error}")})),
            )
                .into_response();
        }
    };

    let now = crate::repositories::tuwunel_cleanup_repository::now_timestamp();
    let cutoff_ts = now.saturating_sub(retention_secs.min(i32::MAX as u64) as i32);
    let room_id = resolved.room_id.to_string();
    let queued = match state.tuwunel_cleanup_repository.record_portal_census_rooms(
        admin_user_id,
        ADMIN_HISTORY_PURGE_SERVICE,
        std::slice::from_ref(&room_id),
        cutoff_ts,
        now,
    ) {
        Ok(queued) => queued,
        Err(error) => {
            tracing::error!(room_id, error = %error, "Could not queue Tuwunel admin room history purge");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "admin history purge queue failed"})),
            )
                .into_response();
        }
    };

    tracing::warn!(
        room_id,
        cutoff_ts,
        retention_secs,
        queued,
        "Queued state-preserving Tuwunel admin room history purge"
    );
    (
        StatusCode::ACCEPTED,
        Json(json!({
            "status": "queued",
            "room_id": room_id,
            "cutoff_ts": cutoff_ts,
            "retention_secs": retention_secs,
            "queued_rows": queued,
            "preserves_state": true
        })),
    )
        .into_response()
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

    // Always allow internal endpoints (maintenance + recovery)
    if path.starts_with("/api/internal/") {
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
