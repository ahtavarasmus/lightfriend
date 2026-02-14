use openai_api_rs::v1::{chat_completion, types};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::AppState;

/// Tool definition for YouTube operations
pub fn get_youtube_tool() -> chat_completion::Tool {
    let mut properties = HashMap::new();
    properties.insert(
        "query".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "The YouTube operation to perform. Use 'search:keywords' to search for videos, \
                'subscriptions' to get the user's recent subscription videos, or \
                'trending' to get trending videos. Examples: 'search:rust programming', \
                'search:cooking recipes', 'subscriptions', 'trending'."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("youtube"),
            description: Some(String::from(
                "Search YouTube videos, get user's subscription feed, or find trending content. \
                Use 'search:query' for video searches (e.g., 'search:cooking tutorials'), \
                'subscriptions' for the user's subscription feed, or 'trending' for trending videos. \
                Returns video titles, thumbnails, and IDs that can be displayed to the user.",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![String::from("query")]),
            },
        },
    }
}

/// Result from YouTube tool operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeToolResult {
    pub videos: Vec<VideoResult>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoResult {
    pub video_id: String,
    pub title: String,
    pub channel: String,
    pub thumbnail: String,
    pub duration: Option<String>,
}

/// Handle YouTube tool calls
pub async fn handle_youtube_tool(state: &Arc<AppState>, user_id: i32, query: &str) -> String {
    // Parse the query to determine operation
    let query = query.trim();

    if query.starts_with("search:") {
        let search_query = query.strip_prefix("search:").unwrap_or("").trim();
        if search_query.is_empty() {
            return "Please provide a search query. Example: 'search:rust programming'".to_string();
        }
        handle_youtube_search(state, user_id, search_query).await
    } else if query == "subscriptions" {
        handle_youtube_subscriptions(state, user_id).await
    } else if query == "trending" {
        // For trending, we can search for popular content
        handle_youtube_search(state, user_id, "trending videos today").await
    } else {
        // Treat as a search query if no prefix
        handle_youtube_search(state, user_id, query).await
    }
}

/// Search YouTube videos
async fn handle_youtube_search(state: &Arc<AppState>, user_id: i32, query: &str) -> String {
    // Get YouTube tokens
    let (access_token, refresh_token) = match state.user_repository.get_youtube_tokens(user_id) {
        Ok(Some(tokens)) => tokens,
        Ok(None) => {
            return "YouTube is not connected. Please connect your YouTube account in the connections settings.".to_string();
        }
        Err(e) => {
            tracing::error!("Failed to get YouTube tokens: {}", e);
            return "Failed to access YouTube. Please try again later.".to_string();
        }
    };

    let client = reqwest::Client::new();

    // Try search with current token
    match perform_search(&client, &access_token, query).await {
        Ok(result) => result,
        Err((401, _)) => {
            // Try to refresh token
            match refresh_youtube_token(state, user_id, &refresh_token).await {
                Ok(new_token) => match perform_search(&client, &new_token, query).await {
                    Ok(result) => result,
                    Err((_, err)) => {
                        tracing::error!("YouTube search failed after token refresh: {}", err);
                        "YouTube search failed. Please try reconnecting your account.".to_string()
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to refresh YouTube token: {}", e);
                    "YouTube session expired. Please reconnect your account.".to_string()
                }
            }
        }
        Err((_, err)) => {
            tracing::error!("YouTube search error: {}", err);
            "Failed to search YouTube. Please try again later.".to_string()
        }
    }
}

/// Get user's subscription feed
async fn handle_youtube_subscriptions(state: &Arc<AppState>, user_id: i32) -> String {
    // Get YouTube tokens
    let (access_token, refresh_token) = match state.user_repository.get_youtube_tokens(user_id) {
        Ok(Some(tokens)) => tokens,
        Ok(None) => {
            return "YouTube is not connected. Please connect your YouTube account in the connections settings.".to_string();
        }
        Err(e) => {
            tracing::error!("Failed to get YouTube tokens: {}", e);
            return "Failed to access YouTube. Please try again later.".to_string();
        }
    };

    let client = reqwest::Client::new();

    // Try to get subscriptions with current token
    match fetch_subscription_videos(&client, &access_token).await {
        Ok(result) => result,
        Err((401, _)) => {
            // Try to refresh token
            match refresh_youtube_token(state, user_id, &refresh_token).await {
                Ok(new_token) => match fetch_subscription_videos(&client, &new_token).await {
                    Ok(result) => result,
                    Err((_, err)) => {
                        tracing::error!(
                            "YouTube subscriptions failed after token refresh: {}",
                            err
                        );
                        "Failed to fetch subscriptions. Please try reconnecting your account."
                            .to_string()
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to refresh YouTube token: {}", e);
                    "YouTube session expired. Please reconnect your account.".to_string()
                }
            }
        }
        Err((_, err)) => {
            tracing::error!("YouTube subscriptions error: {}", err);
            "Failed to fetch subscriptions. Please try again later.".to_string()
        }
    }
}

/// Perform YouTube search API call
async fn perform_search(
    client: &reqwest::Client,
    access_token: &str,
    query: &str,
) -> Result<String, (u16, String)> {
    let search_response = client
        .get("https://www.googleapis.com/youtube/v3/search")
        .query(&[
            ("part", "snippet"),
            ("q", query),
            ("type", "video"),
            ("maxResults", "5"),
        ])
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

    let items = search_data["items"].as_array();
    if items.is_none() || items.unwrap().is_empty() {
        return Ok("No videos found for your search.".to_string());
    }

    let videos: Vec<VideoResult> = items
        .unwrap()
        .iter()
        .filter_map(|item| {
            let video_id = item["id"]["videoId"].as_str()?;
            let snippet = &item["snippet"];
            Some(VideoResult {
                video_id: video_id.to_string(),
                title: snippet["title"].as_str().unwrap_or_default().to_string(),
                channel: snippet["channelTitle"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                thumbnail: snippet["thumbnails"]["medium"]["url"]
                    .as_str()
                    .or_else(|| snippet["thumbnails"]["default"]["url"].as_str())
                    .unwrap_or_default()
                    .to_string(),
                duration: None,
            })
        })
        .collect();

    if videos.is_empty() {
        return Ok("No videos found for your search.".to_string());
    }

    // Format results as JSON that can be parsed by the frontend
    let result = YouTubeToolResult {
        videos: videos.clone(),
        message: format!("Found {} videos:", videos.len()),
    };

    // Return formatted text for the AI to use, plus structured JSON
    let video_list = videos
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{}. {} by {}", i + 1, v.title, v.channel))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        "Found {} videos:\n{}\n\n[MEDIA_RESULTS]{}[/MEDIA_RESULTS]",
        videos.len(),
        video_list,
        serde_json::to_string(&result).unwrap_or_default()
    ))
}

/// Fetch videos from user's subscriptions
async fn fetch_subscription_videos(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<String, (u16, String)> {
    // Get subscriptions
    let subs_response = client
        .get("https://www.googleapis.com/youtube/v3/subscriptions")
        .query(&[("part", "snippet"), ("mine", "true"), ("maxResults", "10")])
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to fetch subscriptions: {}", e)))?;

    if !subs_response.status().is_success() {
        let status = subs_response.status().as_u16();
        let error_text = subs_response.text().await.unwrap_or_default();
        return Err((status, error_text));
    }

    let subs_data: serde_json::Value = subs_response
        .json()
        .await
        .map_err(|e| (500, format!("Failed to parse subscriptions: {}", e)))?;

    let channel_ids: Vec<String> = subs_data["items"]
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
        return Ok("You don't have any YouTube subscriptions.".to_string());
    }

    // Get recent videos from each channel (limit to first 5 channels)
    let mut all_videos: Vec<VideoResult> = Vec::new();

    for channel_id in channel_ids.iter().take(5) {
        if let Ok(resp) = client
            .get("https://www.googleapis.com/youtube/v3/search")
            .query(&[
                ("part", "snippet"),
                ("channelId", channel_id),
                ("order", "date"),
                ("type", "video"),
                ("maxResults", "2"),
            ])
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if let Some(items) = data["items"].as_array() {
                        for item in items {
                            if let Some(video_id) = item["id"]["videoId"].as_str() {
                                let snippet = &item["snippet"];
                                all_videos.push(VideoResult {
                                    video_id: video_id.to_string(),
                                    title: snippet["title"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string(),
                                    channel: snippet["channelTitle"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string(),
                                    thumbnail: snippet["thumbnails"]["medium"]["url"]
                                        .as_str()
                                        .or_else(|| {
                                            snippet["thumbnails"]["default"]["url"].as_str()
                                        })
                                        .unwrap_or_default()
                                        .to_string(),
                                    duration: None,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    if all_videos.is_empty() {
        return Ok("No recent videos from your subscriptions.".to_string());
    }

    // Sort by most recent (we don't have exact dates, but the API returns newest first)
    all_videos.truncate(10);

    let result = YouTubeToolResult {
        videos: all_videos.clone(),
        message: format!(
            "Found {} recent videos from your subscriptions:",
            all_videos.len()
        ),
    };

    let video_list = all_videos
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{}. {} by {}", i + 1, v.title, v.channel))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        "Recent videos from your subscriptions:\n{}\n\n[MEDIA_RESULTS]{}[/MEDIA_RESULTS]",
        video_list,
        serde_json::to_string(&result).unwrap_or_default()
    ))
}

/// Refresh YouTube token
async fn refresh_youtube_token(
    state: &Arc<AppState>,
    user_id: i32,
    refresh_token: &str,
) -> Result<String, String> {
    use oauth2::TokenResponse;

    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    let token_result = state
        .youtube_oauth_client
        .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token.to_string()))
        .request_async(&http_client)
        .await
        .map_err(|e| format!("Failed to refresh token: {}", e))?;

    let new_access_token = token_result.access_token().secret().to_string();
    let expires_in = token_result.expires_in().unwrap_or_default().as_secs() as i32;

    state
        .user_repository
        .update_youtube_access_token(user_id, &new_access_token, expires_in)
        .map_err(|e| format!("Failed to store refreshed token: {}", e))?;

    Ok(new_access_token)
}
