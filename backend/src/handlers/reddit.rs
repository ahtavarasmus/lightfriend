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
pub struct RedditResolveRequest {
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct RedditEmbedResponse {
    pub post_id: String,
    pub embed_url: String,
    pub subreddit: Option<String>,
}

/// Resolves a Reddit URL and returns embed information
pub async fn resolve_reddit_url(
    State(_state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Json(request): Json<RedditResolveRequest>,
) -> Result<Json<RedditEmbedResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Resolving Reddit URL: {}", request.url);

    let url = request.url.trim();

    // Handle redd.it short URLs by following redirects
    let resolved_url = if url.contains("redd.it/") {
        resolve_short_url(url).await.unwrap_or_else(|| url.to_string())
    } else {
        url.to_string()
    };

    // Extract post info from URL
    let (post_id, subreddit) = match extract_reddit_info(&resolved_url) {
        Some(info) => info,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Could not extract post ID from Reddit URL"}))
            ));
        }
    };

    // Reddit embed URL - uses redditmedia.com for iframe embeds
    // Format: https://www.redditmedia.com/r/{subreddit}/comments/{post_id}/?embed=true&theme=dark
    let embed_url = if let Some(ref sub) = subreddit {
        format!(
            "https://www.redditmedia.com/r/{}/comments/{}/?embed=true&theme=dark",
            sub, post_id
        )
    } else {
        format!(
            "https://www.redditmedia.com/comments/{}/?embed=true&theme=dark",
            post_id
        )
    };

    Ok(Json(RedditEmbedResponse {
        post_id,
        embed_url,
        subreddit,
    }))
}

/// Extracts post ID and subreddit from Reddit URL
/// Supports formats:
/// - https://www.reddit.com/r/subreddit/comments/postid/title/
/// - https://reddit.com/r/subreddit/comments/postid/
/// - https://old.reddit.com/r/subreddit/comments/postid/
fn extract_reddit_info(url: &str) -> Option<(String, Option<String>)> {
    // Pattern for Reddit post URLs
    let re = Regex::new(r"reddit\.com/r/([^/]+)/comments/([a-zA-Z0-9]+)").ok()?;

    if let Some(caps) = re.captures(url) {
        let subreddit = caps.get(1).map(|m| m.as_str().to_string());
        let post_id = caps.get(2).map(|m| m.as_str().to_string())?;
        return Some((post_id, subreddit));
    }

    // Pattern for direct post links without subreddit context
    let re_direct = Regex::new(r"reddit\.com/comments/([a-zA-Z0-9]+)").ok()?;
    if let Some(caps) = re_direct.captures(url) {
        let post_id = caps.get(1).map(|m| m.as_str().to_string())?;
        return Some((post_id, None));
    }

    None
}

/// Resolves short Reddit URLs (redd.it) by following redirects
async fn resolve_short_url(short_url: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .ok()?;

    let response = client.get(short_url).send().await.ok()?;
    Some(response.url().to_string())
}
