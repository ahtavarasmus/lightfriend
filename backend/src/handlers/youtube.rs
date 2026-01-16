use crate::handlers::auth_middleware::AuthUser;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use oauth2::TokenResponse;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::AppState;

/// Refresh the YouTube access token using the refresh token
async fn refresh_youtube_token(
    state: &AppState,
    user_id: i32,
    refresh_token: &str,
) -> Result<String, String> {
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    tracing::info!("Attempting to refresh YouTube token for user {}", user_id);

    let token_result = state
        .youtube_oauth_client
        .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token.to_string()))
        .request_async(&http_client)
        .await
        .map_err(|e| format!("Failed to refresh token: {}", e))?;

    let new_access_token = token_result.access_token().secret().to_string();
    let expires_in = token_result.expires_in().unwrap_or_default().as_secs() as i32;

    // Update the access token in the database
    state
        .user_repository
        .update_youtube_access_token(user_id, &new_access_token, expires_in)
        .map_err(|e| format!("Failed to store refreshed token: {}", e))?;

    tracing::info!("Successfully refreshed YouTube token for user {}", user_id);
    Ok(new_access_token)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub channel_id: String,
    pub thumbnail: String,
    pub duration: String,
    pub published_at: String,
    pub view_count: String,
}

#[derive(Debug, Serialize)]
pub struct SubscriptionFeedResponse {
    pub videos: Vec<Video>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub videos: Vec<Video>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<Vec<Channel>>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(rename = "type")]
    pub search_type: Option<String>, // "video", "channel", or "all"
    pub channel_id: Option<String>, // Filter videos by channel ID
}

#[derive(Debug, Deserialize)]
pub struct VideoQuery {
    pub id: String, // Video ID or URL
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Channel {
    pub id: String,
    pub title: String,
    pub description: String,
    pub thumbnail: String,
    pub subscriber_count: String,
    pub is_subscribed: bool,
}

#[derive(Debug, Deserialize)]
pub struct SubscribeRequest {
    pub channel_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Comment {
    pub id: String,
    pub author: String,
    pub author_profile_image: String,
    pub author_channel_id: String,
    pub text: String,
    pub like_count: u64,
    pub published_at: String,
    pub reply_count: u32,
}

#[derive(Debug, Serialize)]
pub struct CommentsResponse {
    pub comments: Vec<Comment>,
    pub next_page_token: Option<String>,
    pub total_results: u32,
}

#[derive(Debug, Deserialize)]
pub struct CommentRequest {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct RateVideoRequest {
    pub rating: String, // "like", "dislike", or "none"
}

/// Helper to fetch subscription feed with a given token
async fn fetch_subscription_feed_with_token(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<SubscriptionFeedResponse, (u16, String)> {
    // First, get the user's subscriptions
    let subscriptions_response = client
        .get("https://www.googleapis.com/youtube/v3/subscriptions")
        .query(&[("part", "snippet"), ("mine", "true"), ("maxResults", "20")])
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to fetch subscriptions: {}", e)))?;

    if !subscriptions_response.status().is_success() {
        let status = subscriptions_response.status().as_u16();
        let error_text = subscriptions_response.text().await.unwrap_or_default();
        return Err((status, error_text));
    }

    let subscriptions_data: serde_json::Value = subscriptions_response
        .json()
        .await
        .map_err(|e| (500, format!("Failed to parse subscriptions: {}", e)))?;

    // Extract channel IDs from subscriptions
    let channel_ids: Vec<String> = subscriptions_data["items"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|item| {
            item["snippet"]["resourceId"]["channelId"]
                .as_str()
                .map(|s| s.to_string())
        })
        .collect();

    if channel_ids.is_empty() {
        return Ok(SubscriptionFeedResponse { videos: vec![] });
    }

    // For each channel, get their recent uploads
    let mut all_videos: Vec<Video> = Vec::new();

    // Fetch videos from multiple channels (batch by taking first 10 channels to avoid too many requests)
    for channel_id in channel_ids.iter().take(10) {
        let search_response = client
            .get("https://www.googleapis.com/youtube/v3/search")
            .query(&[
                ("part", "snippet"),
                ("channelId", channel_id.as_str()),
                ("order", "date"),
                ("type", "video"),
                ("maxResults", "3"),
            ])
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await;

        if let Ok(resp) = search_response {
            if resp.status().is_success() {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if let Some(items) = data["items"].as_array() {
                        for item in items {
                            let video_id = item["id"]["videoId"].as_str().unwrap_or_default();
                            let snippet = &item["snippet"];

                            all_videos.push(Video {
                                id: video_id.to_string(),
                                title: snippet["title"].as_str().unwrap_or_default().to_string(),
                                channel: snippet["channelTitle"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                channel_id: snippet["channelId"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                thumbnail: snippet["thumbnails"]["medium"]["url"]
                                    .as_str()
                                    .or_else(|| snippet["thumbnails"]["default"]["url"].as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                duration: "".to_string(), // Would need additional API call to get duration
                                published_at: snippet["publishedAt"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                view_count: "".to_string(), // Would need additional API call
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort by published date (newest first)
    all_videos.sort_by(|a, b| b.published_at.cmp(&a.published_at));

    // Limit to 20 most recent
    all_videos.truncate(20);

    Ok(SubscriptionFeedResponse { videos: all_videos })
}

/// Fetches recent videos from the user's YouTube subscriptions
pub async fn get_subscription_feed(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<SubscriptionFeedResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(
        "Fetching YouTube subscription feed for user {}",
        auth_user.user_id
    );

    // Get YouTube tokens
    let (access_token, refresh_token) =
        match state.user_repository.get_youtube_tokens(auth_user.user_id) {
            Ok(Some(tokens)) => tokens,
            Ok(None) => {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "YouTube not connected", "youtube_auth_error": true})),
                ));
            }
            Err(e) => {
                tracing::error!("Failed to get YouTube tokens: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to get YouTube tokens"})),
                ));
            }
        };

    let client = reqwest::Client::new();

    // Try with current token first
    match fetch_subscription_feed_with_token(&client, &access_token).await {
        Ok(response) => Ok(Json(response)),
        Err((status, error_text)) => {
            // If 401, try to refresh the token and retry
            if status == 401 {
                tracing::info!(
                    "YouTube token expired, attempting refresh for user {}",
                    auth_user.user_id
                );

                match refresh_youtube_token(&state, auth_user.user_id, &refresh_token).await {
                    Ok(new_token) => {
                        // Retry with new token
                        match fetch_subscription_feed_with_token(&client, &new_token).await {
                            Ok(response) => Ok(Json(response)),
                            Err((_, retry_error)) => {
                                tracing::error!(
                                    "YouTube API error after token refresh: {}",
                                    retry_error
                                );
                                Err((
                                    StatusCode::FORBIDDEN,
                                    Json(
                                        json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                                    ),
                                ))
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to refresh YouTube token: {}", e);
                        Err((
                            StatusCode::FORBIDDEN,
                            Json(
                                json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                            ),
                        ))
                    }
                }
            } else {
                tracing::error!("YouTube API error {}: {}", status, error_text);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "YouTube API error"})),
                ))
            }
        }
    }
}

/// Helper to perform YouTube search with a given token
async fn perform_youtube_search(
    client: &reqwest::Client,
    access_token: &str,
    query: &str,
    type_param: &str,
    channel_id: Option<&str>,
) -> Result<SearchResponse, (u16, String)> {
    let mut query_params = vec![
        ("part", "snippet"),
        ("q", query),
        ("type", type_param),
        ("maxResults", "20"),
    ];

    // Add channel filter if provided
    let channel_id_owned: String;
    if let Some(ch_id) = channel_id {
        channel_id_owned = ch_id.to_string();
        query_params.push(("channelId", &channel_id_owned));
    }

    let search_response = client
        .get("https://www.googleapis.com/youtube/v3/search")
        .query(&query_params)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to search YouTube: {}", e)))?;

    if !search_response.status().is_success() {
        let status = search_response.status().as_u16();
        let error_text = search_response.text().await.unwrap_or_default();
        return Err((status, error_text));
    }

    let search_data: serde_json::Value = search_response
        .json()
        .await
        .map_err(|e| (500, format!("Failed to parse search results: {}", e)))?;

    // Get user's current subscriptions to check if channels are subscribed
    let subscribed_channel_ids = get_user_subscription_ids(client, access_token).await;

    let mut videos: Vec<Video> = Vec::new();
    let mut channels: Vec<Channel> = Vec::new();

    for item in search_data["items"].as_array().unwrap_or(&vec![]) {
        let kind = item["id"]["kind"].as_str().unwrap_or_default();
        let snippet = &item["snippet"];

        if kind == "youtube#video" {
            let video_id = item["id"]["videoId"].as_str().unwrap_or_default();
            videos.push(Video {
                id: video_id.to_string(),
                title: snippet["title"].as_str().unwrap_or_default().to_string(),
                channel: snippet["channelTitle"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                channel_id: snippet["channelId"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                thumbnail: snippet["thumbnails"]["medium"]["url"]
                    .as_str()
                    .or_else(|| snippet["thumbnails"]["default"]["url"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                duration: "".to_string(),
                published_at: snippet["publishedAt"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                view_count: "".to_string(),
            });
        } else if kind == "youtube#channel" {
            let channel_id = item["id"]["channelId"].as_str().unwrap_or_default();
            channels.push(Channel {
                id: channel_id.to_string(),
                title: snippet["title"].as_str().unwrap_or_default().to_string(),
                description: snippet["description"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                thumbnail: snippet["thumbnails"]["medium"]["url"]
                    .as_str()
                    .or_else(|| snippet["thumbnails"]["default"]["url"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                subscriber_count: "".to_string(), // Would need additional API call
                is_subscribed: subscribed_channel_ids.contains(&channel_id.to_string()),
            });
        }
    }

    Ok(SearchResponse {
        videos,
        channels: if channels.is_empty() {
            None
        } else {
            Some(channels)
        },
    })
}

/// Searches YouTube for videos
pub async fn search_youtube(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(
        "Searching YouTube for '{}' for user {}",
        query.q,
        auth_user.user_id
    );

    // Get YouTube tokens
    let (access_token, refresh_token) =
        match state.user_repository.get_youtube_tokens(auth_user.user_id) {
            Ok(Some(tokens)) => tokens,
            Ok(None) => {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "YouTube not connected", "youtube_auth_error": true})),
                ));
            }
            Err(e) => {
                tracing::error!("Failed to get YouTube tokens: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to get YouTube tokens"})),
                ));
            }
        };

    let client = reqwest::Client::new();
    let search_type = query.search_type.as_deref().unwrap_or("video");

    // Determine what to search for
    let type_param = match search_type {
        "channel" => "channel",
        "all" => "video,channel",
        _ => "video",
    };

    let channel_id = query.channel_id.as_deref();

    // Try with current token first
    match perform_youtube_search(&client, &access_token, &query.q, type_param, channel_id).await {
        Ok(response) => Ok(Json(response)),
        Err((status, error_text)) => {
            // If 401, try to refresh the token and retry
            if status == 401 {
                tracing::info!(
                    "YouTube token expired during search, attempting refresh for user {}",
                    auth_user.user_id
                );

                match refresh_youtube_token(&state, auth_user.user_id, &refresh_token).await {
                    Ok(new_token) => {
                        // Retry with new token
                        match perform_youtube_search(
                            &client, &new_token, &query.q, type_param, channel_id,
                        )
                        .await
                        {
                            Ok(response) => Ok(Json(response)),
                            Err((_, retry_error)) => {
                                tracing::error!(
                                    "YouTube search API error after token refresh: {}",
                                    retry_error
                                );
                                Err((
                                    StatusCode::FORBIDDEN,
                                    Json(
                                        json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                                    ),
                                ))
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to refresh YouTube token: {}", e);
                        Err((
                            StatusCode::FORBIDDEN,
                            Json(
                                json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                            ),
                        ))
                    }
                }
            } else {
                tracing::error!("YouTube search API error {}: {}", status, error_text);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "YouTube API error"})),
                ))
            }
        }
    }
}

/// Extracts a YouTube video ID from various URL formats or returns the input if it's already an ID
pub fn extract_video_id(input: &str) -> Option<String> {
    let input = input.trim();

    // If it's already just an ID (11 characters, alphanumeric with - and _)
    if input.len() == 11
        && input
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Some(input.to_string());
    }

    // Try various YouTube URL patterns
    // youtube.com/watch?v=VIDEO_ID
    // youtu.be/VIDEO_ID
    // youtube.com/embed/VIDEO_ID
    // youtube.com/v/VIDEO_ID
    // youtube.com/shorts/VIDEO_ID

    let patterns = [
        r"(?:youtube\.com/watch\?v=|youtu\.be/|youtube\.com/embed/|youtube\.com/v/|youtube\.com/shorts/)([a-zA-Z0-9_-]{11})",
        r"[?&]v=([a-zA-Z0-9_-]{11})",
    ];

    for pattern in patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(caps) = re.captures(input) {
                if let Some(id) = caps.get(1) {
                    return Some(id.as_str().to_string());
                }
            }
        }
    }

    None
}

#[derive(Debug, Serialize)]
pub struct VideoDetailsResponse {
    pub video: Video,
    pub embed_url: String,
}

/// Helper to fetch video details with a given token
async fn fetch_video_details_with_token(
    client: &reqwest::Client,
    access_token: &str,
    video_id: &str,
) -> Result<VideoDetailsResponse, (u16, String)> {
    let video_response = client
        .get("https://www.googleapis.com/youtube/v3/videos")
        .query(&[
            ("part", "snippet,contentDetails,statistics"),
            ("id", video_id),
        ])
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to fetch video details: {}", e)))?;

    if !video_response.status().is_success() {
        let status = video_response.status().as_u16();
        let error_text = video_response.text().await.unwrap_or_default();
        return Err((status, error_text));
    }

    let video_data: serde_json::Value = video_response
        .json()
        .await
        .map_err(|e| (500, format!("Failed to parse video details: {}", e)))?;

    let items = video_data["items"].as_array();
    if items.is_none() || items.unwrap().is_empty() {
        return Err((404, "Video not found".to_string()));
    }

    let item = &items.unwrap()[0];
    let snippet = &item["snippet"];
    let content_details = &item["contentDetails"];
    let statistics = &item["statistics"];

    // Parse ISO 8601 duration (PT1H2M3S) to human readable format
    let duration_iso = content_details["duration"].as_str().unwrap_or("");
    let duration = parse_iso_duration(duration_iso);

    let view_count = statistics["viewCount"]
        .as_str()
        .unwrap_or("0")
        .parse::<u64>()
        .unwrap_or(0);
    let view_count_formatted = format_view_count(view_count);

    let video = Video {
        id: video_id.to_string(),
        title: snippet["title"].as_str().unwrap_or_default().to_string(),
        channel: snippet["channelTitle"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        channel_id: snippet["channelId"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        thumbnail: snippet["thumbnails"]["high"]["url"]
            .as_str()
            .or_else(|| snippet["thumbnails"]["medium"]["url"].as_str())
            .unwrap_or_default()
            .to_string(),
        duration,
        published_at: snippet["publishedAt"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        view_count: view_count_formatted,
    };

    Ok(VideoDetailsResponse {
        video,
        // Parameters:
        // rel=0 - Don't show related videos from other channels at the end
        // modestbranding=1 - Reduce YouTube branding
        // iv_load_policy=3 - Hide video annotations
        // disablekb=0 - Keep keyboard controls enabled
        embed_url: format!(
            "https://www.youtube.com/embed/{}?rel=0&modestbranding=1&iv_load_policy=3",
            video_id
        ),
    })
}

/// Gets details for a specific video by ID or URL
pub async fn get_video_details(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(query): Query<VideoQuery>,
) -> Result<Json<VideoDetailsResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Extract video ID from URL if needed
    let video_id = match extract_video_id(&query.id) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid YouTube video ID or URL"})),
            ));
        }
    };

    tracing::info!(
        "Getting video details for {} for user {}",
        video_id,
        auth_user.user_id
    );

    // Get YouTube tokens
    let (access_token, refresh_token) =
        match state.user_repository.get_youtube_tokens(auth_user.user_id) {
            Ok(Some(tokens)) => tokens,
            Ok(None) => {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "YouTube not connected", "youtube_auth_error": true})),
                ));
            }
            Err(e) => {
                tracing::error!("Failed to get YouTube tokens: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to get YouTube tokens"})),
                ));
            }
        };

    let client = reqwest::Client::new();

    // Try with current token first
    match fetch_video_details_with_token(&client, &access_token, &video_id).await {
        Ok(response) => Ok(Json(response)),
        Err((status, error_text)) => {
            // If 401, try to refresh the token and retry
            if status == 401 {
                tracing::info!(
                    "YouTube token expired for video details, attempting refresh for user {}",
                    auth_user.user_id
                );

                match refresh_youtube_token(&state, auth_user.user_id, &refresh_token).await {
                    Ok(new_token) => {
                        // Retry with new token
                        match fetch_video_details_with_token(&client, &new_token, &video_id).await {
                            Ok(response) => Ok(Json(response)),
                            Err((retry_status, retry_error)) => {
                                if retry_status == 404 {
                                    Err((
                                        StatusCode::NOT_FOUND,
                                        Json(json!({"error": "Video not found"})),
                                    ))
                                } else {
                                    tracing::error!(
                                        "YouTube video API error after token refresh: {}",
                                        retry_error
                                    );
                                    Err((
                                        StatusCode::FORBIDDEN,
                                        Json(
                                            json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                                        ),
                                    ))
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to refresh YouTube token: {}", e);
                        Err((
                            StatusCode::FORBIDDEN,
                            Json(
                                json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                            ),
                        ))
                    }
                }
            } else if status == 404 {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "Video not found"})),
                ))
            } else {
                tracing::error!("YouTube video API error {}: {}", status, error_text);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "YouTube API error"})),
                ))
            }
        }
    }
}

/// Parse ISO 8601 duration (e.g., PT1H2M3S) to human readable format
fn parse_iso_duration(iso: &str) -> String {
    let iso = iso.trim_start_matches("PT");
    let mut hours = 0u32;
    let mut minutes = 0u32;
    let mut seconds = 0u32;

    let mut num_str = String::new();
    for c in iso.chars() {
        if c.is_ascii_digit() {
            num_str.push(c);
        } else {
            let num: u32 = num_str.parse().unwrap_or(0);
            match c {
                'H' => hours = num,
                'M' => minutes = num,
                'S' => seconds = num,
                _ => {}
            }
            num_str.clear();
        }
    }

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

/// Format view count to human readable (e.g., 1.2M, 45K)
fn format_view_count(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M views", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{}K views", count / 1_000)
    } else {
        format!("{} views", count)
    }
}

/// Helper function to get the user's subscribed channel IDs
async fn get_user_subscription_ids(client: &reqwest::Client, access_token: &str) -> Vec<String> {
    let mut channel_ids = Vec::new();

    // Fetch up to 50 subscriptions (one page)
    let response = client
        .get("https://www.googleapis.com/youtube/v3/subscriptions")
        .query(&[("part", "snippet"), ("mine", "true"), ("maxResults", "50")])
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await;

    if let Ok(resp) = response {
        if resp.status().is_success() {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                if let Some(items) = data["items"].as_array() {
                    for item in items {
                        if let Some(channel_id) =
                            item["snippet"]["resourceId"]["channelId"].as_str()
                        {
                            channel_ids.push(channel_id.to_string());
                        }
                    }
                }
            }
        }
    }

    channel_ids
}

/// Helper to perform subscribe with a given token
async fn perform_subscribe(
    client: &reqwest::Client,
    access_token: &str,
    channel_id: &str,
) -> Result<serde_json::Value, (u16, String)> {
    let subscribe_body = json!({
        "snippet": {
            "resourceId": {
                "kind": "youtube#channel",
                "channelId": channel_id
            }
        }
    });

    let response = client
        .post("https://www.googleapis.com/youtube/v3/subscriptions")
        .query(&[("part", "snippet")])
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .json(&subscribe_body)
        .send()
        .await
        .map_err(|e| (500, format!("Failed to subscribe: {}", e)))?;

    if response.status().is_success() {
        Ok(json!({
            "success": true,
            "message": "Successfully subscribed to channel"
        }))
    } else {
        let status = response.status().as_u16();
        let error_text = response.text().await.unwrap_or_default();

        // Check if already subscribed (409 conflict)
        if status == 409 {
            return Ok(json!({
                "success": true,
                "message": "Already subscribed to this channel"
            }));
        }

        Err((status, error_text))
    }
}

/// Subscribe to a YouTube channel
pub async fn subscribe_to_channel(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<SubscribeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(
        "Subscribing to channel {} for user {}",
        request.channel_id,
        auth_user.user_id
    );

    // Get YouTube tokens
    let (access_token, refresh_token) =
        match state.user_repository.get_youtube_tokens(auth_user.user_id) {
            Ok(Some(tokens)) => tokens,
            Ok(None) => {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "YouTube not connected", "youtube_auth_error": true})),
                ));
            }
            Err(e) => {
                tracing::error!("Failed to get YouTube tokens: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to get YouTube tokens"})),
                ));
            }
        };

    let client = reqwest::Client::new();

    // Try with current token first
    match perform_subscribe(&client, &access_token, &request.channel_id).await {
        Ok(response) => Ok(Json(response)),
        Err((status, error_text)) => {
            // If 401, try to refresh the token and retry
            if status == 401 {
                tracing::info!(
                    "YouTube token expired for subscribe, attempting refresh for user {}",
                    auth_user.user_id
                );

                match refresh_youtube_token(&state, auth_user.user_id, &refresh_token).await {
                    Ok(new_token) => {
                        // Retry with new token
                        match perform_subscribe(&client, &new_token, &request.channel_id).await {
                            Ok(response) => Ok(Json(response)),
                            Err((_, retry_error)) => {
                                tracing::error!(
                                    "YouTube subscribe API error after token refresh: {}",
                                    retry_error
                                );
                                Err((
                                    StatusCode::FORBIDDEN,
                                    Json(
                                        json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                                    ),
                                ))
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to refresh YouTube token: {}", e);
                        Err((
                            StatusCode::FORBIDDEN,
                            Json(
                                json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                            ),
                        ))
                    }
                }
            } else if status == 403 {
                // Insufficient scope - need write access
                tracing::error!(
                    "YouTube subscribe failed due to insufficient scope: {}",
                    error_text
                );
                Err((
                    StatusCode::FORBIDDEN,
                    Json(
                        json!({"error": "Insufficient permissions. Please grant subscribe permission first.", "insufficient_scope": true}),
                    ),
                ))
            } else {
                tracing::error!("YouTube subscribe API error {}: {}", status, error_text);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to subscribe to channel"})),
                ))
            }
        }
    }
}

/// Helper to perform unsubscribe with a given token
async fn perform_unsubscribe(
    client: &reqwest::Client,
    access_token: &str,
    channel_id: &str,
) -> Result<serde_json::Value, (u16, String)> {
    // First, we need to find the subscription ID for this channel
    let subscriptions_response = client
        .get("https://www.googleapis.com/youtube/v3/subscriptions")
        .query(&[
            ("part", "id,snippet"),
            ("mine", "true"),
            ("forChannelId", channel_id),
        ])
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to find subscription: {}", e)))?;

    if !subscriptions_response.status().is_success() {
        let status = subscriptions_response.status().as_u16();
        let error_text = subscriptions_response.text().await.unwrap_or_default();
        return Err((status, error_text));
    }

    let data: serde_json::Value = subscriptions_response
        .json()
        .await
        .map_err(|e| (500, format!("Failed to parse subscriptions: {}", e)))?;

    let subscription_id = data["items"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item["id"].as_str())
        .map(|s| s.to_string());

    let subscription_id = match subscription_id {
        Some(id) => id,
        None => {
            return Ok(json!({
                "success": true,
                "message": "Not subscribed to this channel"
            }));
        }
    };

    // Delete the subscription
    let delete_response = client
        .delete("https://www.googleapis.com/youtube/v3/subscriptions")
        .query(&[("id", subscription_id.as_str())])
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to unsubscribe: {}", e)))?;

    if delete_response.status().is_success() || delete_response.status() == 204 {
        Ok(json!({
            "success": true,
            "message": "Successfully unsubscribed from channel"
        }))
    } else {
        let status = delete_response.status().as_u16();
        let error_text = delete_response.text().await.unwrap_or_default();
        Err((status, error_text))
    }
}

/// Unsubscribe from a YouTube channel
pub async fn unsubscribe_from_channel(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(channel_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(
        "Unsubscribing from channel {} for user {}",
        channel_id,
        auth_user.user_id
    );

    // Get YouTube tokens
    let (access_token, refresh_token) =
        match state.user_repository.get_youtube_tokens(auth_user.user_id) {
            Ok(Some(tokens)) => tokens,
            Ok(None) => {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "YouTube not connected", "youtube_auth_error": true})),
                ));
            }
            Err(e) => {
                tracing::error!("Failed to get YouTube tokens: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to get YouTube tokens"})),
                ));
            }
        };

    let client = reqwest::Client::new();

    // Try with current token first
    match perform_unsubscribe(&client, &access_token, &channel_id).await {
        Ok(response) => Ok(Json(response)),
        Err((status, error_text)) => {
            // If 401, try to refresh the token and retry
            if status == 401 {
                tracing::info!(
                    "YouTube token expired for unsubscribe, attempting refresh for user {}",
                    auth_user.user_id
                );

                match refresh_youtube_token(&state, auth_user.user_id, &refresh_token).await {
                    Ok(new_token) => {
                        // Retry with new token
                        match perform_unsubscribe(&client, &new_token, &channel_id).await {
                            Ok(response) => Ok(Json(response)),
                            Err((_, retry_error)) => {
                                tracing::error!(
                                    "YouTube unsubscribe API error after token refresh: {}",
                                    retry_error
                                );
                                Err((
                                    StatusCode::FORBIDDEN,
                                    Json(
                                        json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                                    ),
                                ))
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to refresh YouTube token: {}", e);
                        Err((
                            StatusCode::FORBIDDEN,
                            Json(
                                json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                            ),
                        ))
                    }
                }
            } else if status == 403 {
                // Insufficient scope - need write access
                tracing::error!(
                    "YouTube unsubscribe failed due to insufficient scope: {}",
                    error_text
                );
                Err((
                    StatusCode::FORBIDDEN,
                    Json(
                        json!({"error": "Insufficient permissions. Please grant subscribe permission first.", "insufficient_scope": true}),
                    ),
                ))
            } else {
                tracing::error!("YouTube unsubscribe API error {}: {}", status, error_text);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to unsubscribe from channel"})),
                ))
            }
        }
    }
}

/// Helper to fetch video comments with a given token
async fn fetch_comments_with_token(
    client: &reqwest::Client,
    access_token: &str,
    video_id: &str,
) -> Result<CommentsResponse, (u16, String)> {
    let response = client
        .get("https://www.googleapis.com/youtube/v3/commentThreads")
        .query(&[
            ("part", "snippet"),
            ("videoId", video_id),
            ("maxResults", "20"),
            ("order", "relevance"),
            ("textFormat", "plainText"),
        ])
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to fetch comments: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let error_text = response.text().await.unwrap_or_default();
        return Err((status, error_text));
    }

    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| (500, format!("Failed to parse comments: {}", e)))?;

    let mut comments = Vec::new();

    if let Some(items) = data["items"].as_array() {
        for item in items {
            let snippet = &item["snippet"]["topLevelComment"]["snippet"];
            comments.push(Comment {
                id: item["id"].as_str().unwrap_or_default().to_string(),
                author: snippet["authorDisplayName"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                author_profile_image: snippet["authorProfileImageUrl"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                author_channel_id: snippet["authorChannelId"]["value"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                text: snippet["textDisplay"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                like_count: snippet["likeCount"].as_u64().unwrap_or(0),
                published_at: snippet["publishedAt"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                reply_count: item["snippet"]["totalReplyCount"].as_u64().unwrap_or(0) as u32,
            });
        }
    }

    let total_results = data["pageInfo"]["totalResults"].as_u64().unwrap_or(0) as u32;
    let next_page_token = data["nextPageToken"].as_str().map(|s| s.to_string());

    Ok(CommentsResponse {
        comments,
        next_page_token,
        total_results,
    })
}

/// Get comments for a video
pub async fn get_video_comments(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(video_id): Path<String>,
) -> Result<Json<CommentsResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(
        "Fetching comments for video {} for user {}",
        video_id,
        auth_user.user_id
    );

    // Get YouTube tokens
    let (access_token, refresh_token) =
        match state.user_repository.get_youtube_tokens(auth_user.user_id) {
            Ok(Some(tokens)) => tokens,
            Ok(None) => {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "YouTube not connected", "youtube_auth_error": true})),
                ));
            }
            Err(e) => {
                tracing::error!("Failed to get YouTube tokens: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to get YouTube tokens"})),
                ));
            }
        };

    let client = reqwest::Client::new();

    // Try with current token first
    match fetch_comments_with_token(&client, &access_token, &video_id).await {
        Ok(response) => Ok(Json(response)),
        Err((status, error_text)) => {
            if status == 401 {
                tracing::info!(
                    "YouTube token expired for comments, attempting refresh for user {}",
                    auth_user.user_id
                );

                match refresh_youtube_token(&state, auth_user.user_id, &refresh_token).await {
                    Ok(new_token) => {
                        match fetch_comments_with_token(&client, &new_token, &video_id).await {
                            Ok(response) => Ok(Json(response)),
                            Err((_, retry_error)) => {
                                tracing::error!(
                                    "YouTube comments API error after token refresh: {}",
                                    retry_error
                                );
                                Err((
                                    StatusCode::FORBIDDEN,
                                    Json(
                                        json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                                    ),
                                ))
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to refresh YouTube token: {}", e);
                        Err((
                            StatusCode::FORBIDDEN,
                            Json(
                                json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                            ),
                        ))
                    }
                }
            } else if status == 403 && error_text.contains("commentsDisabled") {
                // Comments are disabled on this video
                Ok(Json(CommentsResponse {
                    comments: vec![],
                    next_page_token: None,
                    total_results: 0,
                }))
            } else {
                tracing::error!("YouTube comments API error {}: {}", status, error_text);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to fetch comments"})),
                ))
            }
        }
    }
}

/// Helper to post a comment with a given token
async fn post_comment_with_token(
    client: &reqwest::Client,
    access_token: &str,
    video_id: &str,
    text: &str,
) -> Result<Comment, (u16, String)> {
    let comment_body = json!({
        "snippet": {
            "videoId": video_id,
            "topLevelComment": {
                "snippet": {
                    "textOriginal": text
                }
            }
        }
    });

    let response = client
        .post("https://www.googleapis.com/youtube/v3/commentThreads")
        .query(&[("part", "snippet")])
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .json(&comment_body)
        .send()
        .await
        .map_err(|e| (500, format!("Failed to post comment: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let error_text = response.text().await.unwrap_or_default();
        return Err((status, error_text));
    }

    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| (500, format!("Failed to parse comment response: {}", e)))?;

    let snippet = &data["snippet"]["topLevelComment"]["snippet"];
    Ok(Comment {
        id: data["id"].as_str().unwrap_or_default().to_string(),
        author: snippet["authorDisplayName"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        author_profile_image: snippet["authorProfileImageUrl"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        author_channel_id: snippet["authorChannelId"]["value"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        text: snippet["textDisplay"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        like_count: 0,
        published_at: snippet["publishedAt"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        reply_count: 0,
    })
}

/// Post a comment on a video
pub async fn post_video_comment(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(video_id): Path<String>,
    Json(request): Json<CommentRequest>,
) -> Result<Json<Comment>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(
        "Posting comment on video {} for user {}",
        video_id,
        auth_user.user_id
    );

    if request.text.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Comment text cannot be empty"})),
        ));
    }

    // Get YouTube tokens
    let (access_token, refresh_token) =
        match state.user_repository.get_youtube_tokens(auth_user.user_id) {
            Ok(Some(tokens)) => tokens,
            Ok(None) => {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "YouTube not connected", "youtube_auth_error": true})),
                ));
            }
            Err(e) => {
                tracing::error!("Failed to get YouTube tokens: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to get YouTube tokens"})),
                ));
            }
        };

    let client = reqwest::Client::new();

    // Try with current token first
    match post_comment_with_token(&client, &access_token, &video_id, &request.text).await {
        Ok(comment) => Ok(Json(comment)),
        Err((status, error_text)) => {
            if status == 401 {
                tracing::info!(
                    "YouTube token expired for posting comment, attempting refresh for user {}",
                    auth_user.user_id
                );

                match refresh_youtube_token(&state, auth_user.user_id, &refresh_token).await {
                    Ok(new_token) => {
                        match post_comment_with_token(&client, &new_token, &video_id, &request.text)
                            .await
                        {
                            Ok(comment) => Ok(Json(comment)),
                            Err((retry_status, retry_error)) => {
                                if retry_status == 403 {
                                    Err((
                                        StatusCode::FORBIDDEN,
                                        Json(
                                            json!({"error": "Insufficient permissions. Please grant comment permission first.", "insufficient_scope": true}),
                                        ),
                                    ))
                                } else {
                                    tracing::error!(
                                        "YouTube post comment API error after token refresh: {}",
                                        retry_error
                                    );
                                    Err((
                                        StatusCode::FORBIDDEN,
                                        Json(
                                            json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                                        ),
                                    ))
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to refresh YouTube token: {}", e);
                        Err((
                            StatusCode::FORBIDDEN,
                            Json(
                                json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                            ),
                        ))
                    }
                }
            } else if status == 403 {
                tracing::error!("YouTube post comment 403 error: {}", error_text);
                // Check if it's a scope issue or something else
                if error_text.contains("insufficientPermissions")
                    || error_text.contains("ACCESS_TOKEN_SCOPE_INSUFFICIENT")
                {
                    Err((
                        StatusCode::FORBIDDEN,
                        Json(
                            json!({"error": "Insufficient permissions. Please grant comment permission first.", "insufficient_scope": true}),
                        ),
                    ))
                } else if error_text.contains("ineligibleAccount")
                    || error_text.contains("not connected to Google+")
                {
                    // YouTube account restriction - some accounts can't comment via API
                    Err((
                        StatusCode::FORBIDDEN,
                        Json(
                            json!({"error": "Your YouTube account cannot post comments via third-party apps. Please comment directly on YouTube."}),
                        ),
                    ))
                } else if error_text.contains("commentsDisabled") {
                    Err((
                        StatusCode::FORBIDDEN,
                        Json(json!({"error": "Comments are disabled on this video"})),
                    ))
                } else if error_text.contains("forbidden")
                    || error_text.contains("processingFailure")
                {
                    Err((
                        StatusCode::FORBIDDEN,
                        Json(
                            json!({"error": "Unable to post comment. The video may have restricted comments."}),
                        ),
                    ))
                } else {
                    Err((
                        StatusCode::FORBIDDEN,
                        Json(json!({"error": "Unable to post comment on this video"})),
                    ))
                }
            } else {
                tracing::error!("YouTube post comment API error {}: {}", status, error_text);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to post comment"})),
                ))
            }
        }
    }
}

/// Helper to rate a video with a given token
async fn rate_video_with_token(
    client: &reqwest::Client,
    access_token: &str,
    video_id: &str,
    rating: &str,
) -> Result<(), (u16, String)> {
    let response = client
        .post("https://www.googleapis.com/youtube/v3/videos/rate")
        .query(&[("id", video_id), ("rating", rating)])
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Length", "0")
        .send()
        .await
        .map_err(|e| (500, format!("Failed to rate video: {}", e)))?;

    if response.status().is_success() || response.status() == 204 {
        Ok(())
    } else {
        let status = response.status().as_u16();
        let error_text = response.text().await.unwrap_or_default();
        Err((status, error_text))
    }
}

/// Rate a video (like, dislike, or remove rating)
pub async fn rate_video(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(video_id): Path<String>,
    Json(request): Json<RateVideoRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Validate rating
    let rating = request.rating.to_lowercase();
    if !["like", "dislike", "none"].contains(&rating.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid rating. Must be 'like', 'dislike', or 'none'"})),
        ));
    }

    tracing::info!(
        "Rating video {} as {} for user {}",
        video_id,
        rating,
        auth_user.user_id
    );

    // Get YouTube tokens
    let (access_token, refresh_token) =
        match state.user_repository.get_youtube_tokens(auth_user.user_id) {
            Ok(Some(tokens)) => tokens,
            Ok(None) => {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "YouTube not connected", "youtube_auth_error": true})),
                ));
            }
            Err(e) => {
                tracing::error!("Failed to get YouTube tokens: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to get YouTube tokens"})),
                ));
            }
        };

    let client = reqwest::Client::new();

    // Try with current token first
    match rate_video_with_token(&client, &access_token, &video_id, &rating).await {
        Ok(()) => Ok(Json(json!({"success": true, "rating": rating}))),
        Err((status, error_text)) => {
            if status == 401 {
                tracing::info!(
                    "YouTube token expired for rating, attempting refresh for user {}",
                    auth_user.user_id
                );

                match refresh_youtube_token(&state, auth_user.user_id, &refresh_token).await {
                    Ok(new_token) => {
                        match rate_video_with_token(&client, &new_token, &video_id, &rating).await {
                            Ok(()) => Ok(Json(json!({"success": true, "rating": rating}))),
                            Err((retry_status, retry_error)) => {
                                if retry_status == 403 {
                                    Err((
                                        StatusCode::FORBIDDEN,
                                        Json(
                                            json!({"error": "Insufficient permissions. Please grant extended access first.", "insufficient_scope": true}),
                                        ),
                                    ))
                                } else {
                                    tracing::error!(
                                        "YouTube rate video API error after token refresh: {}",
                                        retry_error
                                    );
                                    Err((
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(json!({"error": "Failed to rate video"})),
                                    ))
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to refresh YouTube token: {}", e);
                        Err((
                            StatusCode::FORBIDDEN,
                            Json(
                                json!({"error": "YouTube token expired, please reconnect", "youtube_auth_error": true}),
                            ),
                        ))
                    }
                }
            } else if status == 403 {
                tracing::error!("YouTube rate video 403 error: {}", error_text);
                if error_text.contains("insufficientPermissions")
                    || error_text.contains("ACCESS_TOKEN_SCOPE_INSUFFICIENT")
                {
                    Err((
                        StatusCode::FORBIDDEN,
                        Json(
                            json!({"error": "Insufficient permissions. Please grant extended access first.", "insufficient_scope": true}),
                        ),
                    ))
                } else {
                    Err((
                        StatusCode::FORBIDDEN,
                        Json(json!({"error": "Unable to rate this video"})),
                    ))
                }
            } else {
                tracing::error!("YouTube rate video API error {}: {}", status, error_text);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to rate video"})),
                ))
            }
        }
    }
}
