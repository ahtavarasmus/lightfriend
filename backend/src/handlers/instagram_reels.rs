use crate::handlers::auth_middleware::AuthUser;
use axum::{extract::State, http::StatusCode, response::Json};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct InstagramResolveRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct InstagramEmbedResponse {
    pub reel_id: String,
    pub embed_url: String,
}

/// Resolves an Instagram Reel URL and returns embed information
pub async fn resolve_instagram_reel(
    State(_state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Json(request): Json<InstagramResolveRequest>,
) -> Result<Json<InstagramEmbedResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Resolving Instagram URL: {}", request.url);

    let url = request.url.trim();

    // Extract reel/post ID from URL
    let reel_id = match extract_instagram_id(url) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Could not extract reel ID from Instagram URL"})),
            ));
        }
    };

    // Construct embed URL - Instagram uses /p/ format for embeds
    // The /p/ route works for both posts and reels
    let embed_url = format!("https://www.instagram.com/p/{}/embed/", reel_id);

    Ok(Json(InstagramEmbedResponse { reel_id, embed_url }))
}

/// Extracts reel/post ID from Instagram URL
/// Supports formats:
/// - https://www.instagram.com/reel/ABC123xyz/
/// - https://www.instagram.com/reels/ABC123xyz/  (plural form)
/// - https://instagram.com/reel/ABC123xyz/
/// - https://www.instagram.com/p/ABC123xyz/
/// - https://instagram.com/p/ABC123xyz/
fn extract_instagram_id(url: &str) -> Option<String> {
    // Pattern for reel URLs (supports both /reel/ and /reels/)
    let re_reel = Regex::new(r"instagram\.com/reels?/([A-Za-z0-9_-]+)").ok()?;
    if let Some(caps) = re_reel.captures(url) {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    // Pattern for post URLs (some videos are shared as /p/)
    let re_post = Regex::new(r"instagram\.com/p/([A-Za-z0-9_-]+)").ok()?;
    if let Some(caps) = re_post.captures(url) {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    None
}
