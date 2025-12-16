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
pub struct SpotifyResolveRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct SpotifyEmbedResponse {
    pub content_id: String,
    pub content_type: String, // track, episode, playlist, album, artist
    pub embed_url: String,
}

/// Resolves a Spotify URL and returns embed information
pub async fn resolve_spotify_url(
    State(_state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Json(request): Json<SpotifyResolveRequest>,
) -> Result<Json<SpotifyEmbedResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Resolving Spotify URL: {}", request.url);

    let url = request.url.trim();

    // Extract content info from URL
    let (content_type, content_id) = match extract_spotify_info(url) {
        Some(info) => info,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Could not extract content from Spotify URL"}))
            ));
        }
    };

    // Spotify embed URL format
    let embed_url = format!(
        "https://open.spotify.com/embed/{}/{}?theme=0",
        content_type, content_id
    );

    Ok(Json(SpotifyEmbedResponse {
        content_id,
        content_type,
        embed_url,
    }))
}

/// Extracts content type and ID from Spotify URL
/// Supports formats:
/// - https://open.spotify.com/track/4iV5W9uYEdYUVa79Axb7Rh
/// - https://open.spotify.com/episode/1234567890
/// - https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M
/// - https://open.spotify.com/album/1234567890
/// - https://open.spotify.com/artist/1234567890
/// - spotify:track:4iV5W9uYEdYUVa79Axb7Rh (URI format)
fn extract_spotify_info(url: &str) -> Option<(String, String)> {
    // URL format: open.spotify.com/{type}/{id}
    let re_url = Regex::new(r"open\.spotify\.com/(track|episode|playlist|album|artist|show)/([a-zA-Z0-9]+)").ok()?;

    if let Some(caps) = re_url.captures(url) {
        let content_type = caps.get(1).map(|m| m.as_str().to_string())?;
        let content_id = caps.get(2).map(|m| m.as_str().to_string())?;
        return Some((content_type, content_id));
    }

    // URI format: spotify:{type}:{id}
    let re_uri = Regex::new(r"spotify:(track|episode|playlist|album|artist|show):([a-zA-Z0-9]+)").ok()?;

    if let Some(caps) = re_uri.captures(url) {
        let content_type = caps.get(1).map(|m| m.as_str().to_string())?;
        let content_id = caps.get(2).map(|m| m.as_str().to_string())?;
        return Some((content_type, content_id));
    }

    None
}
