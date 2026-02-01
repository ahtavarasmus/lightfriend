//! Backup key establishment endpoints for encrypted backup system.
//!
//! These endpoints allow a browser to establish a session key for encrypting
//! the user's bridge data. The key is generated in the browser and sent
//! over HTTPS to the enclave.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::handlers::auth_middleware::AuthUser;
use crate::AppState;

/// Request body for establishing a backup session key
#[derive(Debug, Deserialize)]
pub struct EstablishKeyRequest {
    /// Base64-encoded 256-bit session key from browser
    pub session_key: String,
}

/// Response for successful key establishment
#[derive(Debug, Serialize)]
pub struct EstablishKeyResponse {
    pub success: bool,
    pub session_id: String,
}

/// Response for session status check
#[derive(Debug, Serialize)]
pub struct SessionStatusResponse {
    pub active: bool,
    pub last_backup: Option<i64>,
    pub session_id: Option<String>,
    pub established_at: Option<String>,
}

/// POST /api/backup/establish-key
///
/// Establishes a backup session key for the authenticated user.
/// The key is held in memory only and used to encrypt backup data.
pub async fn establish_key(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<EstablishKeyRequest>,
) -> impl IntoResponse {
    let user_id = auth_user.user_id;

    // Decode the base64 session key
    let key_bytes = match BASE64.decode(&payload.session_key) {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!("Invalid base64 session key from user {}: {}", user_id, e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "success": false,
                    "error": "Invalid session key encoding"
                })),
            );
        }
    };

    // Verify key length (must be 256 bits / 32 bytes)
    if key_bytes.len() != 32 {
        tracing::warn!(
            "Invalid session key length from user {}: {} bytes",
            user_id,
            key_bytes.len()
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "Session key must be 256 bits"
            })),
        );
    }

    // Convert to fixed-size array
    let mut key: [u8; 32] = [0u8; 32];
    key.copy_from_slice(&key_bytes);

    // Generate a session ID for tracking
    let session_id = Uuid::new_v4().to_string();

    // Store the key in memory
    state
        .session_key_store
        .set(user_id, key, session_id.clone())
        .await;

    // Update the user's backup_session_active flag in the database
    if let Err(e) = state
        .user_repository
        .set_backup_session_active(user_id, true)
    {
        tracing::error!(
            "Failed to update backup_session_active for user {}: {}",
            user_id,
            e
        );
        // Continue anyway - the in-memory key is established
    }

    tracing::info!(
        "Backup session key established for user {} with session {}",
        user_id,
        session_id
    );

    (
        StatusCode::OK,
        Json(serde_json::json!(EstablishKeyResponse {
            success: true,
            session_id,
        })),
    )
}

/// GET /api/backup/session-status
///
/// Returns the status of the user's backup session.
pub async fn session_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> impl IntoResponse {
    let user_id = auth_user.user_id;

    // Check if session key exists in memory
    let session_info = state.session_key_store.get_session_info(user_id).await;

    // Get last backup timestamp from database
    let last_backup = state
        .user_repository
        .get_last_backup_at(user_id)
        .ok()
        .flatten();

    let response = if let Some(info) = session_info {
        SessionStatusResponse {
            active: true,
            last_backup: last_backup.map(|t| t as i64),
            session_id: Some(info.session_id),
            established_at: Some(info.established_at.to_rfc3339()),
        }
    } else {
        SessionStatusResponse {
            active: false,
            last_backup: last_backup.map(|t| t as i64),
            session_id: None,
            established_at: None,
        }
    };

    (StatusCode::OK, Json(response))
}

/// DELETE /api/backup/session
///
/// Clears the backup session key for the authenticated user.
pub async fn clear_session(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> impl IntoResponse {
    let user_id = auth_user.user_id;

    // Remove the key from memory
    state.session_key_store.remove(user_id).await;

    // Update the database flag
    if let Err(e) = state
        .user_repository
        .set_backup_session_active(user_id, false)
    {
        tracing::error!(
            "Failed to clear backup_session_active for user {}: {}",
            user_id,
            e
        );
    }

    tracing::info!("Backup session cleared for user {}", user_id);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true
        })),
    )
}
