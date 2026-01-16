use crate::handlers::auth_middleware::AuthUser;
use axum::{extract::State, http::StatusCode, response::Json};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct RumbleResolveRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct RumbleEmbedResponse {
    pub video_id: String,
    pub embed_url: String,
}

#[derive(Debug, Deserialize)]
struct RumbleOEmbed {
    html: Option<String>,
}

/// Resolves a Rumble URL and returns embed information
pub async fn resolve_rumble_url(
    State(_state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Json(request): Json<RumbleResolveRequest>,
) -> Result<Json<RumbleEmbedResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Resolving Rumble URL: {}", request.url);

    let url = request.url.trim();

    // Use Rumble's oEmbed API to get the proper embed URL
    match fetch_rumble_oembed(url).await {
        Some((video_id, embed_url)) => Ok(Json(RumbleEmbedResponse {
            video_id,
            embed_url,
        })),
        None => Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({"error": "Could not fetch Rumble video embed. Make sure the URL is valid."}),
            ),
        )),
    }
}

/// Fetches oEmbed data from Rumble and extracts the embed URL
async fn fetch_rumble_oembed(url: &str) -> Option<(String, String)> {
    let client = reqwest::Client::new();
    let oembed_url = format!(
        "https://rumble.com/api/Media/oembed.json?url={}",
        urlencoding::encode(url)
    );

    tracing::info!("Fetching Rumble oEmbed: {}", oembed_url);

    let response = client.get(&oembed_url).send().await.ok()?;
    let oembed: RumbleOEmbed = response.json().await.ok()?;

    // Extract embed URL from the HTML iframe
    // Format: <iframe ... src="https://rumble.com/embed/v1abc123/?pub=4" ...>
    if let Some(html) = oembed.html {
        let re = Regex::new(r#"src="(https://rumble\.com/embed/([^/]+)/[^"]*)""#).ok()?;
        if let Some(caps) = re.captures(&html) {
            let embed_url = caps.get(1).map(|m| m.as_str().to_string())?;
            let video_id = caps.get(2).map(|m| m.as_str().to_string())?;
            return Some((video_id, embed_url));
        }
    }

    None
}
