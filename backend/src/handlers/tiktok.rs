use crate::handlers::auth_middleware::AuthUser;
use axum::{extract::State, http::StatusCode, response::Json};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct TikTokResolveRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct TikTokEmbedResponse {
    pub video_id: String,
    pub embed_url: String,
    pub title: String,
    pub author: String,
    pub thumbnail_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TikTokOEmbedResponse {
    title: Option<String>,
    author_name: Option<String>,
    thumbnail_url: Option<String>,
}

/// Resolves a TikTok URL and returns embed information
pub async fn resolve_tiktok_url(
    State(_state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Json(request): Json<TikTokResolveRequest>,
) -> Result<Json<TikTokEmbedResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Resolving TikTok URL: {}", request.url);

    let url = request.url.trim();

    // Handle short URLs (vm.tiktok.com) by following redirects
    let resolved_url = if url.contains("vm.tiktok.com") {
        resolve_short_url(url)
            .await
            .unwrap_or_else(|| url.to_string())
    } else {
        url.to_string()
    };

    // Extract video ID from URL
    let video_id = match extract_tiktok_video_id(&resolved_url) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Could not extract video ID from TikTok URL"})),
            ));
        }
    };

    // Call TikTok oEmbed API to get metadata
    let client = reqwest::Client::new();
    let oembed_url = format!(
        "https://www.tiktok.com/oembed?url={}",
        urlencoding::encode(&resolved_url)
    );

    let (title, author, thumbnail_url) = match client.get(&oembed_url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<TikTokOEmbedResponse>().await {
                Ok(oembed) => (
                    oembed.title.unwrap_or_else(|| "TikTok Video".to_string()),
                    oembed.author_name.unwrap_or_else(|| "Unknown".to_string()),
                    oembed.thumbnail_url,
                ),
                Err(_) => ("TikTok Video".to_string(), "Unknown".to_string(), None),
            }
        }
        _ => ("TikTok Video".to_string(), "Unknown".to_string(), None),
    };

    // Construct embed URL
    let embed_url = format!("https://www.tiktok.com/embed/v2/{}", video_id);

    Ok(Json(TikTokEmbedResponse {
        video_id,
        embed_url,
        title,
        author,
        thumbnail_url,
    }))
}

/// Extracts video ID from TikTok URL
/// Supports formats:
/// - https://www.tiktok.com/@username/video/1234567890123456789
/// - https://tiktok.com/@username/video/1234567890123456789
fn extract_tiktok_video_id(url: &str) -> Option<String> {
    // Pattern for full TikTok URLs with video ID
    let re = Regex::new(r"tiktok\.com/@[^/]+/video/(\d+)").ok()?;

    if let Some(caps) = re.captures(url) {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    // Pattern for URLs with /v/ format
    let re_v = Regex::new(r"tiktok\.com/v/(\d+)").ok()?;
    if let Some(caps) = re_v.captures(url) {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    None
}

/// Resolves short TikTok URLs (vm.tiktok.com) by following redirects
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
        response
            .headers()
            .get("location")
            .and_then(|l| l.to_str().ok())
            .map(|s| s.to_string())
    }
}
