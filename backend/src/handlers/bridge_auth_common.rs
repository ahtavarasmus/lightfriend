use crate::UserCoreOps;
use anyhow::{anyhow, Result};
use axum::{extract::State, http::StatusCode, response::Json as AxumJson};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;

use crate::handlers::auth_middleware::AuthUser;
use crate::repositories::user_core::UserCore;
use crate::repositories::user_repository::UserRepository;
use crate::AppState;

/// Helper function to get the store path for a Matrix user
pub fn get_store_path(username: &str) -> Result<String> {
    let persistent_store_path = std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
        .map_err(|_| anyhow!("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"))?;
    Ok(format!("{}/{}", persistent_store_path, username))
}

/// Handler to reset/clear all Matrix credentials for a user.
/// This is useful when Matrix auth fails and the user needs to re-register.
pub async fn reset_matrix_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, String)> {
    let user_id = auth_user.user_id;

    tracing::info!("User {} requested Matrix connection reset", user_id);

    // Get the user's Matrix username before clearing (if it exists)
    let user_core = UserCore::new(state.db_pool.clone());
    let matrix_username = user_core
        .find_by_id(user_id)
        .map_err(|e| {
            tracing::error!("Failed to get user {}: {}", user_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get user".to_string(),
            )
        })?
        .and_then(|u| u.matrix_username);

    // Clear the Matrix credentials from the database
    let user_repo = UserRepository::new(state.pg_pool.clone(), state.db_pool.clone());
    user_repo.clear_matrix_credentials(user_id).map_err(|e| {
        tracing::error!(
            "Failed to clear Matrix credentials for user {}: {}",
            user_id,
            e
        );
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to clear Matrix credentials".to_string(),
        )
    })?;

    // Also clear the local store directory if it exists
    if let Some(ref username) = matrix_username {
        if let Ok(store_path) = get_store_path(username) {
            if Path::new(&store_path).exists() {
                if let Err(e) = fs::remove_dir_all(&store_path).await {
                    tracing::warn!("Failed to remove store directory {}: {}", store_path, e);
                    // Don't fail the request, just log the warning
                } else {
                    tracing::info!("Removed Matrix store directory: {}", store_path);
                }
            }
        }
    }

    tracing::info!("Successfully reset Matrix connection for user {}", user_id);

    Ok(AxumJson(json!({
        "success": true,
        "message": "Matrix connection has been reset. You can now reconnect your messaging bridges."
    })))
}
