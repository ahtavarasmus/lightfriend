use std::sync::Arc;
use axum::{
    extract::{State},
    response::Json,
    http::{StatusCode, HeaderMap},
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde_json::json;

use crate::{
    AppState,
    handlers::auth_dtos::Claims,
};

pub async fn gmail_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Checking Gmail connection status");

    // Extract and validate token
    let auth_header = headers.get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token,
        None => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "No authorization token provided"}))
        )),
    };

    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET_KEY must be set in environment")
            .as_bytes()),
        &Validation::new(Algorithm::HS256)
    ) {
        Ok(token_data) => {
            tracing::info!("JWT token decoded successfully");
            token_data.claims
        },
        Err(e) => {
            tracing::error!("Invalid token: {}", e);
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"}))
            ));
        },
    };

    // Check if user has active Gmail connection
    match state.user_repository.has_active_gmail(claims.sub) {
        Ok(has_connection) => {
            tracing::info!("Successfully checked Gmail connection status for user {}: {}", claims.sub, has_connection);
            Ok(Json(json!({
                "connected": has_connection,
                "user_id": claims.sub
            })))
        },
        Err(e) => {
            tracing::error!("Failed to check Gmail connection status: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to check Gmail connection status",
                    "details": e.to_string()
                }))
            ))
        }
    }
}

