use std::sync::Arc;
use crate::handlers::auth_middleware::AuthUser;
use axum::{
    extract::State,
    response::Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use regex::Regex;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct StreamableResolveRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct StreamableEmbedResponse {
    pub video_id: String,
    pub embed_url: String,
}

/// Resolves a Streamable URL and returns embed information
pub async fn resolve_streamable_url(
    State(_state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Json(request): Json<StreamableResolveRequest>,
) -> Result<Json<StreamableEmbedResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Resolving Streamable URL: {}", request.url);

    let url = request.url.trim();

    // Extract video ID from URL
    let video_id = match extract_streamable_video_id(url) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Could not extract video ID from Streamable URL"}))
            ));
        }
    };

    // Streamable embed URL format
    let embed_url = format!("https://streamable.com/e/{}", video_id);

    Ok(Json(StreamableEmbedResponse {
        video_id,
        embed_url,
    }))
}

/// Extracts video ID from Streamable URL
/// Supports formats:
/// - https://streamable.com/abc123
/// - https://streamable.com/e/abc123
fn extract_streamable_video_id(url: &str) -> Option<String> {
    // Pattern for Streamable URLs
    let re = Regex::new(r"streamable\.com/(?:e/)?([a-zA-Z0-9]+)").ok()?;

    if let Some(caps) = re.captures(url) {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    None
}
