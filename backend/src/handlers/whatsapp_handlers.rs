use axum::{
    extract::State,
    Json,
};
use crate::{AppState, utils::whatsapp_utils::{fetch_whatsapp_messages, WhatsAppMessage}};
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
pub async fn fetch_whatsapp_rooms(
    State(state): State<std::sync::Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<WhatsAppMessagesResponse>, String> {
    // Get bridge info first to verify WhatsApp is connected
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

    tracing::info!("Fetching WhatsApp rooms from {} to {}", start_time, end_time);

    // First try to get the client from the state
    let client = {
        let clients = state.matrix_user_clients.lock().await;
        match clients.get(&auth_user.user_id) {
            Some(client) => client.clone(),
            None => {
                tracing::error!("Matrix client not found in state for user {}", auth_user.user_id);
                return Err("Matrix client not initialized".to_string());
            }
        }
    };
    
    // Get all rooms the user has joined
    let rooms = client.joined_rooms();
    tracing::info!("Found {} total joined rooms for user {}", rooms.len(), auth_user.user_id);
    
    if rooms.is_empty() {
        return Ok(Json(WhatsAppMessagesResponse { messages: Vec::new() }));
    }

    // Filter for WhatsApp rooms only
    // WhatsApp rooms typically have specific patterns in their names or IDs
    let whatsapp_rooms: Vec<_> = rooms.into_iter()
        .filter(|room| {
            // Get room name asynchronously but in a blocking way for filtering
            let room_name = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    room.display_name().await.unwrap_or_else(|_| {
                        matrix_sdk::RoomDisplayName::Named(room.room_id().to_string())
                    }).to_string()
                })
            });
            
            // Filter criteria for WhatsApp rooms - adjust these patterns based on your setup
            room_name.contains("WhatsApp") || 
            room_name.contains("WA:") || 
            room_name.contains("+") ||  // Phone numbers often start with +
            room_name.contains("@whatsapp.com")
        })
        .collect();
    
    tracing::info!("Filtered down to {} WhatsApp rooms", whatsapp_rooms.len());

    // Collect messages from WhatsApp rooms
    let mut all_whatsapp_messages = Vec::new();
    
    for room in whatsapp_rooms {
        let room_id = room.room_id();
        let room_name = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                room.display_name().await.unwrap_or_else(|_| {
                    matrix_sdk::RoomDisplayName::Named(room_id.to_string())
                }).to_string()
            })
        });
        
        tracing::info!("Processing WhatsApp room: {}", room_name);
        
        // Fetch messages from this room
        let mut options = matrix_sdk::room::MessagesOptions::backward();
        options.limit = matrix_sdk::ruma::UInt::new(50).unwrap(); // Fetch up to 50 messages per room
        
        match room.messages(options).await {
            Ok(response) => {
                for event in response.chunk {
                    if let Ok(any_sync_event) = event.raw().deserialize() {
                        if let matrix_sdk::ruma::events::AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
                        ) = any_sync_event {
                            match msg {
                                matrix_sdk::ruma::events::room::message::SyncRoomMessageEvent::Original(e) => {
                                    let timestamp = i64::from(e.origin_server_ts.0) / 1000;
                                    
                                    // Skip messages outside the time range
                                    if timestamp < start_time || timestamp > end_time {
                                        continue;
                                    }
                                    
                                    let (msgtype, body) = match &e.content.msgtype {
                                        matrix_sdk::ruma::events::room::message::MessageType::Text(t) => 
                                            ("text", t.body.clone()),
                                        matrix_sdk::ruma::events::room::message::MessageType::Notice(n) => 
                                            ("notice", n.body.clone()),
                                        matrix_sdk::ruma::events::room::message::MessageType::Image(_) => 
                                            ("image", "📎 IMAGE".into()),
                                        matrix_sdk::ruma::events::room::message::MessageType::Video(_) => 
                                            ("video", "📎 VIDEO".into()),
                                        matrix_sdk::ruma::events::room::message::MessageType::File(_) => 
                                            ("file", "📎 FILE".into()),
                                        matrix_sdk::ruma::events::room::message::MessageType::Audio(_) => 
                                            ("audio", "📎 AUDIO".into()),
                                        matrix_sdk::ruma::events::room::message::MessageType::Location(_) => 
                                            ("location", "📍 LOCATION".into()),
                                        matrix_sdk::ruma::events::room::message::MessageType::Emote(t) => 
                                            ("emote", t.body.clone()),
                                        _ => continue,
                                    };
                                    
                                    // Filter out bridge bot messages
                                    if e.sender.localpart().contains("whatsappbot") || 
                                       e.sender.localpart().contains("whatsapp-bridge") {
                                        continue;
                                    }
                                    
                                    all_whatsapp_messages.push(WhatsAppMessage {
                                        sender: e.sender.to_string(),
                                        sender_display_name: e.sender.localpart().to_string(),
                                        content: body,
                                        timestamp,
                                        message_type: msgtype.to_string(),
                                        room_name: room_name.clone(),
                                    });
                                },
                                _ => continue,
                            }
                        }
                    }
                }
            },
            Err(e) => {
                tracing::error!("Failed to fetch messages for room {}: {}", room_id, e);
                // Continue with other rooms even if one fails
            }
        }
    }
    
    // Sort messages by timestamp (most recent first)
    all_whatsapp_messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    
    tracing::info!("Found {} WhatsApp messages across all rooms", all_whatsapp_messages.len());
    
    // Log some details about the messages to help debug
    for (i, msg) in all_whatsapp_messages.iter().enumerate().take(5) {
        tracing::info!(
            "WhatsApp message {}: room={}, sender={}, content={}",
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
    
    Ok(Json(WhatsAppMessagesResponse { messages: all_whatsapp_messages }))
}

