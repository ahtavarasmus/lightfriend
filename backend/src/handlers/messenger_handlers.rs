use axum::{
    extract::{State, Json},
};
use serde::Deserialize;
use crate::{AppState, utils::bridge::{fetch_bridge_messages, BridgeMessage, BridgeRoom}};
use serde::Serialize;
use chrono::Utc;
use crate::handlers::auth_middleware::AuthUser;
#[derive(Serialize)]
pub struct MessengerMessagesResponse {
    messages: Vec<BridgeMessage>,
}
#[derive(Deserialize)]
pub struct SearchMessengerRoomsRequest {
    search_term: String,
}
#[derive(Serialize)]
pub struct SearchMessengerRoomsResponse {
    rooms: Vec<BridgeRoom>,
}
#[derive(Deserialize)]
pub struct SendMessengerMessageRequest {
    chat_name: String,
    message: String,
    image_url: Option<String>,
}
#[derive(Serialize)]
pub struct SendMessengerMessageResponse {
    message: BridgeMessage,
}
pub async fn send_messenger_message(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<SendMessengerMessageRequest>,
) -> Result<Json<SendMessengerMessageResponse>, String> {
    // Get bridge info first to verify Messenger is connected
    let bridge = state.user_repository.get_bridge(auth_user.user_id, "messenger")
        .map_err(|e| format!("Failed to get bridge info: {}", e))?
        .ok_or_else(|| "Messenger bridge not found".to_string())?;
    tracing::info!("Found Messenger bridge: status={}, room_id={:?}", bridge.status, bridge.room_id);
    if bridge.status != "connected" {
        return Err("Messenger is not connected".to_string());
    }
    match crate::utils::bridge::send_bridge_message(
        "messenger",
        &state,
        auth_user.user_id,
        &request.chat_name,
        &request.message,
        request.image_url,
    ).await {
        Ok(message) => {
            tracing::info!("Successfully sent Messenger message to {}", request.chat_name);
            Ok(Json(SendMessengerMessageResponse { message }))
        }
        Err(e) => {
            tracing::error!("Failed to send Messenger message: {}", e);
            Err(format!("Failed to send Messenger message: {}", e))
        }
    }
}
pub async fn test_fetch_messenger_messages(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<MessengerMessagesResponse>, String> {
    // Get bridge info first
    let bridge = state.user_repository.get_bridge(auth_user.user_id, "messenger")
        .map_err(|e| format!("Failed to get bridge info: {}", e))?
        .ok_or_else(|| "Messenger bridge not found".to_string())?;
    tracing::info!("Found Messenger bridge: status={}, room_id={:?}", bridge.status, bridge.room_id);
    if bridge.status != "connected" {
        return Err("Messenger is not connected".to_string());
    }
    // Get a wider time range - last 24 hours
    let now = Utc::now();
    let start_time = (now - chrono::Duration::hours(24)).timestamp();
    let end_time = now.timestamp()+1000000;
    tracing::info!("Fetching messages from {} to {}", start_time, end_time);
    match crate::utils::bridge::fetch_bridge_messages("messenger", &state, auth_user.user_id, start_time, false).await {
        Ok(messages) => {
            tracing::info!("Found {} messages", messages.len());
            Ok(Json(MessengerMessagesResponse { messages }))
        }
        Err(e) => {
            tracing::error!("Error fetching messages: {}", e);
          
            // Try to fall back to the older fetch_messenger_messages method
            match fetch_bridge_messages("messenger", &state, auth_user.user_id, start_time, false).await {
                Ok(fallback_messages) => {
                    tracing::info!("Fallback successful, found {} messages", fallback_messages.len());
                    Ok(Json(MessengerMessagesResponse { messages: fallback_messages }))
                },
                Err(fallback_err) => {
                    tracing::error!("Fallback also failed: {}", fallback_err);
                    // Return a proper error response with status code
                    Err(format!("Failed to fetch messages: {}. Fallback also failed: {}", e, fallback_err))
                }
            }
        }
    }
}
/// Handler that specifically fetches only Messenger rooms for the user
pub async fn search_messenger_rooms_handler(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<SearchMessengerRoomsRequest>,
) -> Result<Json<SearchMessengerRoomsResponse>, String> {
    // Get bridge info first to verify Messenger is connected
    let bridge = state.user_repository.get_bridge(auth_user.user_id, "messenger")
        .map_err(|e| format!("Failed to get bridge info: {}", e))?
        .ok_or_else(|| "Messenger bridge not found".to_string())?;
    if bridge.status != "connected" {
        return Err("Messenger is not connected".to_string());
    }
    match crate::utils::bridge::search_bridge_rooms(
        "messenger",
        &state,
        auth_user.user_id,
        &request.search_term,
    ).await {
        Ok(rooms) => {
            if rooms.is_empty() {
                tracing::error!("No rooms found matching search term: '{}'", request.search_term);
            }
            Ok(Json(SearchMessengerRoomsResponse { rooms }))
        }
        Err(e) => {
            tracing::error!("Failed to search Messenger rooms: {}", e);
            Err(format!("Failed to search Messenger rooms: {}", e))
        }
    }
}
