use axum::{
    extract::{State, Json},
};
use serde::Deserialize;
use crate::{AppState, utils::whatsapp_utils::{fetch_whatsapp_messages, WhatsAppMessage}};
use serde::Serialize;
use chrono::{Utc, NaiveDateTime};
use crate::handlers::auth_middleware::AuthUser;

#[derive(Serialize)]
pub struct WhatsAppMessagesResponse {
    messages: Vec<crate::utils::whatsapp_utils::WhatsAppMessage>,
}

#[derive(Deserialize)]
pub struct SearchWhatsAppRoomsRequest {
    search_term: String,
}

#[derive(Serialize)]
pub struct SearchWhatsAppRoomsResponse {
    rooms: Vec<crate::utils::whatsapp_utils::WhatsAppRoom>,
}

#[derive(Deserialize)]
pub struct SendWhatsAppMessageRequest {
    chat_name: String,
    message: String,
}

#[derive(Serialize)]
pub struct SendWhatsAppMessageResponse {
    message: WhatsAppMessage,
}

pub async fn send_message(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<SendWhatsAppMessageRequest>,
) -> Result<Json<SendWhatsAppMessageResponse>, String> {
    // Get bridge info first to verify WhatsApp is connected
    let bridge = state.user_repository.get_whatsapp_bridge(auth_user.user_id)
        .map_err(|e| format!("Failed to get bridge info: {}", e))?
        .ok_or_else(|| "WhatsApp bridge not found".to_string())?;

    tracing::info!("Found WhatsApp bridge: status={}, room_id={:?}", bridge.status, bridge.room_id);

    if bridge.status != "connected" {
        return Err("WhatsApp is not connected".to_string());
    }

    // Send the message using the whatsapp_utils function
    match crate::utils::whatsapp_utils::send_whatsapp_message(
        &state,
        auth_user.user_id,
        &request.chat_name,
        &request.message,
    ).await {
        Ok(message) => {
            tracing::info!("Successfully sent WhatsApp message to {}", request.chat_name);
            Ok(Json(SendWhatsAppMessageResponse { message }))
        }
        Err(e) => {
            tracing::error!("Failed to send WhatsApp message: {}", e);
            Err(format!("Failed to send WhatsApp message: {}", e))
        }
    }
}

pub async fn test_fetch_messages(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<WhatsAppMessagesResponse>, String> {

    // Get bridge info first
    let bridge = state.user_repository.get_whatsapp_bridge(auth_user.user_id)
        .map_err(|e| format!("Failed to get bridge info: {}", e))?
        .ok_or_else(|| "WhatsApp bridge not found".to_string())?;

    tracing::info!("Found WhatsApp bridge: status={}, room_id={:?}", bridge.status, bridge.room_id);

    if bridge.status != "connected" {
        return Err("WhatsApp is not connected".to_string());
    }

    // Get a wider time range - last 24 hours
    let now = Utc::now().naive_utc();
    let start_time = (now - chrono::Duration::hours(24)).timestamp();
    let end_time = now.timestamp()+1000000;

    tracing::info!("Fetching messages from {} to {}", start_time, end_time);

    match crate::utils::whatsapp_utils::fetch_whatsapp_messages(&state, auth_user.user_id, start_time, end_time).await {
        Ok(messages) => {
            tracing::info!("Found {} messages", messages.len());
            
            // Log some details about the messages to help debug
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
            
            Ok(Json(WhatsAppMessagesResponse { messages }))
        }
        Err(e) => {
            tracing::error!("Error fetching messages: {}", e);
            
            // Try to fall back to the older fetch_whatsapp_messages method
            tracing::info!("Attempting fallback to fetch_whatsapp_messages method");
            match fetch_whatsapp_messages(&state, auth_user.user_id, start_time, end_time).await {
                Ok(fallback_messages) => {
                    tracing::info!("Fallback successful, found {} messages", fallback_messages.len());
                    Ok(Json(WhatsAppMessagesResponse { messages: fallback_messages }))
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


/// Handler that specifically fetches only WhatsApp rooms for the user
pub async fn search_whatsapp_rooms_handler(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<SearchWhatsAppRoomsRequest>,
) -> Result<Json<SearchWhatsAppRoomsResponse>, String> {
    // Get bridge info first to verify WhatsApp is connected
    let bridge = state.user_repository.get_whatsapp_bridge(auth_user.user_id)
        .map_err(|e| format!("Failed to get bridge info: {}", e))?
        .ok_or_else(|| "WhatsApp bridge not found".to_string())?;

    if bridge.status != "connected" {
        return Err("WhatsApp is not connected".to_string());
    }

    match crate::utils::whatsapp_utils::search_whatsapp_rooms(
        &state,
        auth_user.user_id,
        &request.search_term,
    ).await {
        Ok(rooms) => {
            println!("Found {} matching WhatsApp rooms", rooms.len());
            
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

            Ok(Json(SearchWhatsAppRoomsResponse { rooms }))
        }
        Err(e) => {
            tracing::error!("Failed to search WhatsApp rooms: {}", e);
            Err(format!("Failed to search WhatsApp rooms: {}", e))
        }
    }
}

