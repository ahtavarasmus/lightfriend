use crate::UserCoreOps;
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::repositories::user_repository::UsageDataPoint;
use crate::{handlers::auth_middleware::AuthUser, AppState};

#[derive(Deserialize)]
pub struct AutoTopupSettings {
    pub active: bool,
    pub amount: Option<f32>,
}

#[derive(Deserialize)]
pub struct UsageDataRequest {
    pub from: i32,
}

pub async fn get_usage_data(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<UsageDataRequest>,
) -> Result<Json<Vec<UsageDataPoint>>, (StatusCode, Json<serde_json::Value>)> {
    println!("in get_usage_data route");

    // Get usage data using the provided 'from' timestamp
    let usage_data = state
        .user_repository
        .get_usage_data(auth_user.user_id, request.from)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    Ok(Json(usage_data))
}

pub async fn reset_credits(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check if user is an admin
    if !auth_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Only admins can reset credits"})),
        ));
    }

    // Reset user's credits to zero in database
    state
        .user_repository
        .update_user_credits(user_id, 0.00)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    Ok(Json(json!({
        "message": "credits reset successfully"
    })))
}

pub async fn increase_credits(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check if user is modifying their own credits or is an admin
    if auth_user.user_id != user_id && !auth_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "You can only modify your own credits unless you're an admin"})),
        ));
    }

    // Update user's credits in database
    state
        .user_repository
        .increase_credits(user_id, 1.00)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    Ok(Json(json!({
        "message": "credits increased successfully"
    })))
}

#[derive(Serialize)]
pub struct UsageLogEntry {
    pub activity_type: String,
    pub credits: Option<f32>,
    pub created_at: i32,
    pub call_duration: Option<i32>,
}

pub async fn get_recent_usage(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<UsageLogEntry>>, (StatusCode, Json<serde_json::Value>)> {
    // Get billing period start from user's next_billing_date_timestamp
    let user = state
        .user_core
        .find_by_id(auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        ))?;

    // Calculate billing period start (30 days before next billing, or 30 days ago)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    let billing_period_start = if let Some(next_billing) = user.next_billing_date_timestamp {
        next_billing - 30 * 86400
    } else {
        now - 30 * 86400
    };

    let logs = state
        .user_repository
        .get_recent_usage_logs(auth_user.user_id, billing_period_start, 50)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    let entries: Vec<UsageLogEntry> = logs
        .into_iter()
        .map(|log| UsageLogEntry {
            activity_type: log.activity_type,
            credits: log.credits,
            created_at: log.created_at,
            call_duration: log.call_duration,
        })
        .collect();

    Ok(Json(entries))
}

pub async fn update_topup(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(settings): Json<AutoTopupSettings>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Update the user's auto-topup settings with fixed threshold of 3.00
    match state
        .user_core
        .update_auto_topup(auth_user.user_id, settings.active, settings.amount)
    {
        Ok(_) => Ok(Json(json!({
            "success": true,
            "message": "Auto top-up settings updated successfully"
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                json!({"success": false, "message": format!("Failed to update auto top-up settings: {}", e)}),
            ),
        )),
    }
}
