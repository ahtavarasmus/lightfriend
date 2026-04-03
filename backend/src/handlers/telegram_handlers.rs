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
pub struct TelegramMessagesResponse {
    messages: Vec<BridgeMessage>,
}

#[derive(Deserialize)]
pub struct SearchTelegramRoomsRequest {
    search_term: String,
}

#[derive(Serialize)]
pub struct SearchTelegramRoomsResponse {
    rooms: Vec<BridgeRoom>,
}

#[derive(Deserialize)]
pub struct SendTelegramMessageRequest {
    chat_name: String,
    message: String,
    image_url: Option<String>,
}

#[derive(Serialize)]
pub struct SendTelegramMessageResponse {
    message: BridgeMessage,
}

pub async fn send_message(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<SendTelegramMessageRequest>,
) -> Result<Json<SendTelegramMessageResponse>, String> {
    // Get bridge info first to verify Telegram is connected
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "telegram")
        .map_err(|e| format!("Failed to get bridge info: {}", e))?
        .ok_or_else(|| "Telegram bridge not found".to_string())?;

    tracing::info!(
        "Found Telegram bridge: status={}, room_id={:?}",
        bridge.status,
        bridge.room_id
    );

    if bridge.status != "connected" {
        return Err("Telegram is not connected".to_string());
    }

    match crate::utils::bridge::send_bridge_message(
        "telegram",
        &state,
        auth_user.user_id,
        &request.chat_name,
        &request.message,
        request.image_url,
        None,
    )
    .await
    {
        Ok(message) => {
            tracing::info!(
                "Successfully sent Telegram message to {}",
                request.chat_name
            );
            Ok(Json(SendTelegramMessageResponse { message }))
        }
        Err(e) => {
            tracing::error!("Failed to send Telegram message: {}", e);
            Err(format!("Failed to send Telegram message: {}", e))
        }
    }
}

pub async fn test_fetch_messages(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<TelegramMessagesResponse>, String> {
    // Get bridge info first
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "telegram")
        .map_err(|e| format!("Failed to get bridge info: {}", e))?
        .ok_or_else(|| "Telegram bridge not found".to_string())?;

    tracing::info!(
        "Found Telegram bridge: status={}, room_id={:?}",
        bridge.status,
        bridge.room_id
    );

    if bridge.status != "connected" {
        return Err("Telegram is not connected".to_string());
    }

    // Get a wider time range - last 24 hours
    let now = Utc::now();
    let start_time = (now - chrono::Duration::hours(24)).timestamp();
    let end_time = now.timestamp() + 1000000;

    tracing::info!("Fetching messages from {} to {}", start_time, end_time);

    match crate::utils::bridge::fetch_bridge_messages(
        "telegram",
        &state,
        auth_user.user_id,
        start_time,
        false,
    )
    .await
    {
        Ok(messages) => {
            tracing::info!("Found {} messages", messages.len());
            Ok(Json(TelegramMessagesResponse { messages }))
        }
        Err(e) => {
            tracing::error!("Error fetching messages: {}", e);

            // Try to fall back to the older fetch_telegram_messages method
            match fetch_bridge_messages("telegram", &state, auth_user.user_id, start_time, false)
                .await
            {
                Ok(fallback_messages) => {
                    tracing::info!(
                        "Fallback successful, found {} messages",
                        fallback_messages.len()
                    );
                    Ok(Json(TelegramMessagesResponse {
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

/// Handler that specifically fetches only Telegram rooms for the user
pub async fn search_telegram_rooms_handler(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<SearchTelegramRoomsRequest>,
) -> Result<Json<SearchTelegramRoomsResponse>, String> {
    // Get bridge info first to verify Telegram is connected
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "telegram")
        .map_err(|e| format!("Failed to get bridge info: {}", e))?
        .ok_or_else(|| "Telegram bridge not found".to_string())?;

    if bridge.status != "connected" {
        return Err("Telegram is not connected".to_string());
    }

    match crate::utils::bridge::search_bridge_rooms(
        "telegram",
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

            Ok(Json(SearchTelegramRoomsResponse { rooms }))
        }
        Err(e) => {
            tracing::error!("Failed to search Telegram rooms: {}", e);
            Err(format!("Failed to search Telegram rooms: {}", e))
        }
    }
}

use axum::{extract::Query, http::StatusCode};

#[derive(Deserialize)]
pub struct SearchQuery {
    search: String,
}

pub async fn search_rooms_handler(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<BridgeRoom>>, StatusCode> {
    match crate::utils::bridge::search_bridge_rooms(
        "telegram",
        &state,
        auth_user.user_id,
        &params.search,
    )
    .await
    {
        Ok(rooms) => Ok(Json(rooms)),
        Err(e) => {
            tracing::error!(
                "Failed to search Telegram rooms for user {}: {}",
                auth_user.user_id,
                e
            );
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
