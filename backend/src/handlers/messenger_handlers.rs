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
    let now = Utc::now().naive_utc();
    let start_time = (now - chrono::Duration::hours(24)).timestamp();
    let end_time = now.timestamp()+1000000;
    tracing::info!("Fetching messages from {} to {}", start_time, end_time);
    match crate::utils::bridge::fetch_bridge_messages("messenger", &state, auth_user.user_id, start_time, false).await {
        Ok(messages) => {
            tracing::info!("Found {} messages", messages.len());
          
            // Print message details in a readable format for testing
            println!("\n📱 Messenger Messages Summary:");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
          
            for msg in messages.iter() {
                let datetime = chrono::DateTime::<chrono::Utc>::from_timestamp(msg.timestamp, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "unknown time".to_string());
                  
                let message_type_icon = match msg.message_type.as_str() {
                    "text" => "💬",
                    "notice" => "📢",
                    "image" => "🖼️",
                    "video" => "🎥",
                    "file" => "📎",
                    "audio" => "🔊",
                    "audio" => "🔊",
                    "location" => "📍",
                    "emote" => "🎭",
                    _ => "📝",
                };
              
                println!("\n{} Room: {}", message_type_icon, msg.room_name);
                println!("👤 {}", msg.sender_display_name);
                println!("🕒 {}", msg.formatted_timestamp);
                println!("📄 {}", msg.content);
                println!("─────────────────────────────────────");
            }
          
            println!("\nTotal messages: {}\n", messages.len());
          
            // Also keep the debug logging for the first 5 messages
            for (i, msg) in messages.iter().enumerate().take(5) {
                tracing::info!(
                    "Message {}: room={}, sender={}, content={}",
                    i,
                    msg.room_name,
                    msg.sender,
                    if msg.content.len() > 30 {
                        format!("{}...", &msg.content[..30])
                    } else {
                        msg.content.clone()
                    }
                );
            }
          
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
            println!("Found {} matching Messenger rooms", rooms.len());
          
            // Print detailed information about each matching room
            for (i, room) in rooms.iter().enumerate() {
                println!(
                    "Room {}: ID='{}', Name='{}'",
                    i + 1,
                    room.room_id,
                    room.display_name
                );
            }
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
use axum::{
    extract::Query,
    http::StatusCode,
};
#[derive(Deserialize)]
pub struct SearchQuery {
    search: String,
}
pub async fn search_messenger_rooms_handler_query(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<BridgeRoom>>, StatusCode> {
    match crate::utils::bridge::search_bridge_rooms("messenger", &state, auth_user.user_id, &params.search).await {
        Ok(rooms) => Ok(Json(rooms)),
        Err(e) => {
            tracing::error!("Failed to search Messenger rooms for user {}: {}", auth_user.user_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
