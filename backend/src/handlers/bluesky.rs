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
pub struct BlueskyResolveRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct BlueskyEmbedResponse {
    pub post_id: String,
    pub handle: String,
    pub embed_url: String,
}

/// Resolves a Bluesky URL and returns embed information
pub async fn resolve_bluesky_url(
    State(_state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Json(request): Json<BlueskyResolveRequest>,
) -> Result<Json<BlueskyEmbedResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Resolving Bluesky URL: {}", request.url);

    let url = request.url.trim();

    // Extract post info from URL
    let (handle, post_id) = match extract_bluesky_info(url) {
        Some(info) => info,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Could not extract post from Bluesky URL"}))
            ));
        }
    };

    // Bluesky iframe embed URL - pass the full post URL as a parameter
    let post_url = format!("https://bsky.app/profile/{}/post/{}", handle, post_id);
    let embed_url = format!("https://embed.bsky.app/embed?url={}", urlencoding::encode(&post_url));

    Ok(Json(BlueskyEmbedResponse {
        post_id,
        handle,
        embed_url,
    }))
}

/// Extracts handle and post ID from Bluesky URL
/// Supports formats:
/// - https://bsky.app/profile/handle.bsky.social/post/3abc123xyz
/// - https://bsky.app/profile/custom.domain/post/3abc123xyz
fn extract_bluesky_info(url: &str) -> Option<(String, String)> {
    // Pattern for Bluesky post URLs
    let re = Regex::new(r"bsky\.app/profile/([^/]+)/post/([a-zA-Z0-9]+)").ok()?;

    if let Some(caps) = re.captures(url) {
        let handle = caps.get(1).map(|m| m.as_str().to_string())?;
        let post_id = caps.get(2).map(|m| m.as_str().to_string())?;
        return Some((handle, post_id));
    }

    None
}
