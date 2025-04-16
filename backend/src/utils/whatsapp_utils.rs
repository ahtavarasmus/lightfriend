use std::sync::Arc;
use anyhow::{anyhow, Result};
use matrix_sdk::{
    Client as MatrixClient,
    ruma::{
        events::room::message::{RoomMessageEventContent, SyncRoomMessageEvent, MessageType},
        events::AnySyncTimelineEvent,
        OwnedRoomId, OwnedUserId, OwnedDeviceId,
    },
};
use serde::{Deserialize, Serialize};
use crate::{AppState, models::user_models::Bridge};

#[derive(Debug, Serialize, Deserialize)]
pub struct WhatsAppMessage {
    pub sender: String,
    pub sender_display_name: String,
    pub content: String,
    pub timestamp: i64,
    pub message_type: String,
    pub room_name: String,
}


pub async fn fetch_whatsapp_messages(
    state: &AppState,
    user_id: i32,
    start_time: i64,
    end_time: i64,
) -> Result<Vec<WhatsAppMessage>> {
    tracing::info!("Fetching messages for user {}", user_id);
    
    // Get user's Matrix credentials
    let (username, access_token, device_id) = state.user_repository.get_matrix_credentials(user_id)?
        .ok_or_else(|| anyhow!("Matrix credentials not found"))?;

    // Get homeserver URL
    let homeserver_url = std::env::var("MATRIX_HOMESERVER")
        .map_err(|_| anyhow!("MATRIX_HOMESERVER not set"))?;

    // Parse the homeserver URL to extract the domain
    let parsed_url = url::Url::parse(&homeserver_url)?;
    let domain = parsed_url.host_str()
        .ok_or_else(|| anyhow!("Invalid homeserver URL: missing host"))?;

    let full_user_id = format!("@{}:{}", username, domain);

    // Initialize HTTP client for direct Matrix API access
    let http_client = reqwest::Client::new();
    
    // Step 1: Get the list of all joined rooms using the /sync API
    let sync_url = format!("{}/_matrix/client/v3/sync?timeout=10000", homeserver_url);
    tracing::info!("Syncing with Matrix server to get room list: {}", sync_url);
    
    let response = http_client.get(&sync_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    
    // Extract the joined rooms from the sync response
    let joined_rooms = match response.get("rooms").and_then(|r| r.get("join")) {
        Some(rooms) => rooms.as_object().ok_or_else(|| anyhow!("Invalid rooms format"))?,
        None => {
            tracing::warn!("No joined rooms found in sync response");
            return Ok(Vec::new());
        }
    };
    
    tracing::info!("Found {} joined rooms via direct API", joined_rooms.len());
    let mut room_ids_and_names = Vec::new();
    
    // Get the basic info for each room
    for (room_id, room_data) in joined_rooms {
        let room_name = room_data
            .get("state")
            .and_then(|state| state.get("events"))
            .and_then(|events| {
                events.as_array().and_then(|arr| {
                    arr.iter()
                        .find(|event| {
                            event.get("type").map_or(false, |t| t == "m.room.name")
                        })
                        .and_then(|event| event.get("content").and_then(|c| c.get("name")).and_then(|n| n.as_str()))
                })
            })
            .unwrap_or_else(|| room_id.strip_prefix("!").unwrap_or(room_id));
        
        room_ids_and_names.push((room_id.to_string(), room_name.to_string()));
        tracing::info!("Room: {} ({})", room_name, room_id);
    }
    
    // Sort rooms by name for nicer output
    room_ids_and_names.sort_by(|a, b| a.1.cmp(&b.1));
    
    let mut all_messages = Vec::new();
    
    // Step 2: For each room, fetch messages within the time range
    for (room_id, room_name) in room_ids_and_names {
        tracing::info!("Processing room: {} ({})", room_name, room_id);
        
        // URL for room messages
        let messages_url = format!(
            "{}/_matrix/client/v3/rooms/{}/messages?dir=b&limit=100",
            homeserver_url,
            urlencoding::encode(&room_id)
        );
        
        tracing::debug!("Fetching messages from: {}", messages_url);
        
        let mut from_token: Option<String> = None;
        
        // Loop to paginate through messages
        loop {
            let mut url = messages_url.clone();
            if let Some(token) = &from_token {
                url = format!("{}&from={}", url, token);
            }
            
            let response = match http_client.get(&url)
                .header("Authorization", format!("Bearer {}", access_token))
                .send()
                .await
            {
                Ok(resp) => {
                    if resp.status().is_success() {
                        match resp.json::<serde_json::Value>().await {
                            Ok(json) => json,
                            Err(err) => {
                                tracing::error!("Failed to parse message response: {}", err);
                                break;
                            }
                        }
                    } else {
                        tracing::error!("Error response: {} {}", resp.status(), resp.text().await.unwrap_or_default());
                        break;
                    }
                },
                Err(err) => {
                    tracing::error!("Failed to fetch messages: {}", err);
                    break;
                }
            };
            
            // Extract messages
            let chunk = response.get("chunk").and_then(|c| c.as_array());
            let chunk_size = chunk.map(|c| c.len()).unwrap_or(0);
            tracing::debug!("Received {} messages from room {}", chunk_size, room_id);
            
            if let Some(chunk) = chunk {
                let mut found_older_messages = false;
                
                for event in chunk {
                    // Get basic event info
                    let event_type = event.get("type").and_then(|t| t.as_str());
                    if event_type != Some("m.room.message") {
                        continue;
                    }
                    
                    let timestamp = event.get("origin_server_ts")
                        .and_then(|ts| ts.as_u64())
                        .map(|ts| ts as i64 / 1000)
                        .unwrap_or(0);

                    
                    // Get message content
                    let content = event.get("content");
                    let msgtype = content.and_then(|c| c.get("msgtype")).and_then(|t| t.as_str());
                    let body = content.and_then(|c| c.get("body")).and_then(|b| b.as_str());
                    
                    if let (Some(msgtype), Some(body)) = (msgtype, body) {
                        let message_type = match msgtype {
                            "m.text" => "text",
                            "m.image" => "image",
                            "m.video" => "video",
                            "m.audio" => "audio",
                            "m.file" => "file",
                            "m.notice" => "notice",
                            _ => continue,
                        };
                        
                        let message_content = if message_type == "text" || message_type == "notice" {
                            body.to_string()
                        } else {
                            format!("📎 {}", message_type.to_uppercase())
                        };
                        
                        // Get sender info
                        let sender_id = event.get("sender").and_then(|s| s.as_str()).unwrap_or("unknown");
                        let sender_display_name = sender_id.strip_prefix("@").and_then(|s| s.split(':').next()).unwrap_or(sender_id);
                        
                        tracing::debug!("Found message in room {} from {}: {}", 
                            room_name, sender_display_name, message_content.chars().take(30).collect::<String>());
                        
                        all_messages.push(WhatsAppMessage {
                            sender: sender_id.to_string(),
                            sender_display_name: sender_display_name.to_string(),
                            content: message_content,
                            timestamp,
                            message_type: message_type.to_string(),
                            room_name: room_name.clone(),
                        });
                    }
                }
                
                // Break conditions
                if found_older_messages {
                    break;
                }
            }
            
            // Check for end of pagination
            from_token = response.get("end").and_then(|e| e.as_str()).map(|s| s.to_string());
            if from_token.is_none() {
                break;
            }
        }
    }
    
    tracing::info!("Found {} messages across all rooms", all_messages.len());
    
    // Sort messages by timestamp (newest first)
    all_messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    
    Ok(all_messages)
}
