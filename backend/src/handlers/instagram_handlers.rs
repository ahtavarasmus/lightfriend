use crate::handlers::auth_middleware::AuthUser;
use crate::{
    utils::bridge::{fetch_bridge_messages, BridgeMessage, BridgeRoom},
    AppState,
};
use axum::extract::{Json, State};
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
#[derive(Serialize)]
pub struct InstagramMessagesResponse {
    messages: Vec<BridgeMessage>,
}
#[derive(Deserialize)]
pub struct SearchInstagramRoomsRequest {
    search_term: String,
}
#[derive(Serialize)]
pub struct SearchInstagramRoomsResponse {
    rooms: Vec<BridgeRoom>,
}
#[derive(Deserialize)]
pub struct SendInstagramMessageRequest {
    chat_name: String,
    message: String,
    image_url: Option<String>,
}
#[derive(Serialize)]
pub struct SendInstagramMessageResponse {
    message: BridgeMessage,
}
pub async fn send_instagram_message(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<SendInstagramMessageRequest>,
) -> Result<Json<SendInstagramMessageResponse>, String> {
    // Get bridge info first to verify Instagram is connected
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "instagram")
        .map_err(|e| format!("Failed to get bridge info: {}", e))?
        .ok_or_else(|| "Instagram bridge not found".to_string())?;
    tracing::info!(
        "Found Instagram bridge: status={}, room_id={:?}",
        bridge.status,
        bridge.room_id
    );
    if bridge.status != "connected" {
        return Err("Instagram is not connected".to_string());
    }
    match crate::utils::bridge::send_bridge_message(
        "instagram",
        &state,
        auth_user.user_id,
        &request.chat_name,
        &request.message,
        request.image_url,
    )
    .await
    {
        Ok(message) => {
            tracing::info!(
                "Successfully sent Instagram message to {}",
                request.chat_name
            );
            Ok(Json(SendInstagramMessageResponse { message }))
        }
        Err(e) => {
            tracing::error!("Failed to send Instagram message: {}", e);
            Err(format!("Failed to send Instagram message: {}", e))
        }
    }
}
pub async fn test_fetch_instagram_messages(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<InstagramMessagesResponse>, String> {
    // Get bridge info first
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "instagram")
        .map_err(|e| format!("Failed to get bridge info: {}", e))?
        .ok_or_else(|| "Instagram bridge not found".to_string())?;
    tracing::info!(
        "Found Instagram bridge: status={}, room_id={:?}",
        bridge.status,
        bridge.room_id
    );
    if bridge.status != "connected" {
        return Err("Instagram is not connected".to_string());
    }
    // Get a wider time range - last 24 hours
    let now = Utc::now();
    let start_time = (now - chrono::Duration::hours(24)).timestamp();
    let end_time = now.timestamp() + 1000000;
    tracing::info!("Fetching messages from {} to {}", start_time, end_time);
    match crate::utils::bridge::fetch_bridge_messages(
        "instagram",
        &state,
        auth_user.user_id,
        start_time,
        false,
    )
    .await
    {
        Ok(messages) => {
            tracing::info!("Found {} messages", messages.len());
            Ok(Json(InstagramMessagesResponse { messages }))
        }
        Err(e) => {
            tracing::error!("Error fetching messages: {}", e);

            // Try to fall back to the older fetch_instagram_messages method
            match fetch_bridge_messages("instagram", &state, auth_user.user_id, start_time, false)
                .await
            {
                Ok(fallback_messages) => {
                    tracing::info!(
                        "Fallback successful, found {} messages",
                        fallback_messages.len()
                    );
                    Ok(Json(InstagramMessagesResponse {
                        messages: fallback_messages,
                    }))
                }
                Err(fallback_err) => {
                    tracing::error!("Fallback also failed: {}", fallback_err);
                    // Return a proper error response with status code
                    Err(format!(
                        "Failed to fetch messages: {}. Fallback also failed: {}",
                        e, fallback_err
                    ))
                }
            }
        }
    }
}
/// Handler that specifically fetches only Instagram rooms for the user
pub async fn search_instagram_rooms_handler(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<SearchInstagramRoomsRequest>,
) -> Result<Json<SearchInstagramRoomsResponse>, String> {
    // Get bridge info first to verify Instagram is connected
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "instagram")
        .map_err(|e| format!("Failed to get bridge info: {}", e))?
        .ok_or_else(|| "Instagram bridge not found".to_string())?;
    if bridge.status != "connected" {
        return Err("Instagram is not connected".to_string());
    }
    match crate::utils::bridge::search_bridge_rooms(
        "instagram",
        &state,
        auth_user.user_id,
        &request.search_term,
    )
    .await
    {
        Ok(rooms) => {
            if rooms.is_empty() {
                tracing::error!(
                    "No rooms found matching search term: '{}'",
                    request.search_term
                );
            }
            Ok(Json(SearchInstagramRoomsResponse { rooms }))
        }
        Err(e) => {
            tracing::error!("Failed to search Instagram rooms: {}", e);
            Err(format!("Failed to search Instagram rooms: {}", e))
        }
    }
}
