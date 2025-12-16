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
pub struct TwitterResolveRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct TwitterEmbedResponse {
    pub tweet_id: String,
    pub embed_url: String,
    pub author: Option<String>,
}

/// Resolves a Twitter/X URL and returns embed information
pub async fn resolve_twitter_url(
    State(_state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Json(request): Json<TwitterResolveRequest>,
) -> Result<Json<TwitterEmbedResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Resolving Twitter/X URL: {}", request.url);

    let url = request.url.trim();

    // Handle t.co short URLs by following redirects
    let resolved_url = if url.contains("t.co/") {
        resolve_short_url(url).await.unwrap_or_else(|| url.to_string())
    } else {
        url.to_string()
    };

    // Extract tweet ID and author from URL
    let (tweet_id, author) = match extract_tweet_info(&resolved_url) {
        Some(info) => info,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Could not extract tweet ID from Twitter/X URL"}))
            ));
        }
    };

    // Construct embed URL using Twitter's platform embed
    let embed_url = format!(
        "https://platform.twitter.com/embed/Tweet.html?id={}&theme=dark",
        tweet_id
    );

    Ok(Json(TwitterEmbedResponse {
        tweet_id,
        embed_url,
        author,
    }))
}

/// Extracts tweet ID and author from Twitter/X URL
/// Supports formats:
/// - https://twitter.com/username/status/1234567890123456789
/// - https://x.com/username/status/1234567890123456789
/// - https://twitter.com/username/status/1234567890123456789?s=20
/// - https://x.com/i/status/1234567890123456789 (no username)
fn extract_tweet_info(url: &str) -> Option<(String, Option<String>)> {
    // Pattern for Twitter/X URLs with username
    let re = Regex::new(r"(?:twitter\.com|x\.com)/([^/]+)/status/(\d+)").ok()?;

    if let Some(caps) = re.captures(url) {
        let username = caps.get(1).map(|m| m.as_str().to_string());
        let tweet_id = caps.get(2).map(|m| m.as_str().to_string())?;

        // Filter out "i" as it's not a real username (used in some URL formats)
        let author = username.filter(|u| u != "i");

        return Some((tweet_id, author));
    }

    None
}

/// Resolves short Twitter URLs (t.co) by following redirects
async fn resolve_short_url(short_url: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;

    // Follow redirect manually to get the full URL
    let response = client.head(short_url).send().await.ok()?;

    if let Some(location) = response.headers().get("location") {
        location.to_str().ok().map(|s| s.to_string())
    } else {
        // Try GET request if HEAD doesn't return location
        let response = client.get(short_url).send().await.ok()?;
        response.headers().get("location").and_then(|l| l.to_str().ok()).map(|s| s.to_string())
    }
}
