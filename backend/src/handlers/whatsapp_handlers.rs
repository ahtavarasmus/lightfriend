use axum::{
    extract::State,
    Json,
};
use crate::{AppState, utils::whatsapp_utils::fetch_whatsapp_messages};
use serde::Serialize;
use chrono::{Utc, NaiveDateTime};
use crate::handlers::auth_middleware::AuthUser;

#[derive(Serialize)]
pub struct WhatsAppMessagesResponse {
    messages: Vec<crate::utils::whatsapp_utils::WhatsAppMessage>,
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
    let end_time = now.timestamp();

    tracing::info!("Fetching messages from {} to {}", start_time, end_time);

    match fetch_whatsapp_messages(&state, auth_user.user_id, start_time, end_time).await {
        Ok(messages) => {
            tracing::info!("Found {} messages", messages.len());
            tracing::debug!("Messages: {:?}", messages);
            Ok(Json(WhatsAppMessagesResponse { messages }))
        }
        Err(e) => {
            tracing::error!("Error fetching messages: {}", e);
            // Return a proper error response with status code
            Err(format!("Failed to fetch messages: {}", e))
        }
    }
}

