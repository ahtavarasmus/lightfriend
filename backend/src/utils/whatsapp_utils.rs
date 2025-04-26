use std::sync::Arc;
use anyhow::{anyhow, Result};
use matrix_sdk::{
    Client as MatrixClient,
    room::Room,
    ruma::{
        events::room::message::{RoomMessageEventContent, SyncRoomMessageEvent, MessageType},
        events::{AnyTimelineEvent, AnySyncTimelineEvent},
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

use std::time::Duration;
use tokio::time::sleep;

pub async fn fetch_whatsapp_messages(
    state: &AppState,
    user_id: i32,
    start_time: i64,
    end_time: i64,
) -> Result<Vec<WhatsAppMessage>> {

    println!("fetch whastapp messages");

    let (username, access_token, device_id, password) = state.user_repository
        .get_matrix_credentials(user_id)?
        .ok_or_else(|| anyhow!("Matrix credentials not found"))?;

    let homeserver_url = std::env::var("MATRIX_HOMESERVER")
        .expect("MATRIX_HOMESERVER not set");

    let parsed_url = url::Url::parse(&homeserver_url)?;
    let domain = parsed_url.host_str().ok_or_else(|| anyhow!("Invalid homeserver URL"))?;
    let full_user_id = format!("@{}:{}", username, domain);

    // Setup SQLite store path
    let store_path = format!(
        "{}/{}",
        std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
            .expect("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"),
        username
    );
    std::fs::create_dir_all(&store_path)?;

    let client = MatrixClient::builder()
        .homeserver_url(homeserver_url)
        .sqlite_store(store_path.clone(), None)
        .build()
        .await?;

    println!("here");

    use matrix_sdk::{
        Client as MatrixClient,
        AuthSession, SessionMeta, authentication::matrix::{MatrixSession, MatrixSessionTokens},
    };
    use matrix_sdk::ruma::{OwnedUserId, OwnedDeviceId};

    let session = AuthSession::Matrix(MatrixSession {
        meta: SessionMeta {
            user_id:  OwnedUserId::try_from(full_user_id.clone())?,
            device_id: OwnedDeviceId::try_from(device_id.clone())?,
        },
        tokens: MatrixSessionTokens {
            access_token: access_token.clone(),
            refresh_token: None,
        },
    });

    client.restore_session(session).await?;

    // Check if we're logged in first before doing any syncs
    let bridge = state.user_repository.get_whatsapp_bridge(user_id)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("WhatsApp bridge is not connected. Please log in first."));
    }

    // Quick final sync to get latest state

    tracing::info!("checking joined rooms: {:#?}", client.joined_rooms());
    for room in client.joined_rooms() {
        tracing::info!("✅ Joined room: {}", room.room_id());
    }

    let mut messages = Vec::new();

    for room in client.joined_rooms() {
        let room_id = room.room_id();
        let room_name = room.display_name().await.unwrap_or_else(|_| matrix_sdk::RoomDisplayName::Named(room_id.to_string()));

        let mut from_token = None;

        loop {
            let mut options = matrix_sdk::room::MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(100).unwrap();
            if let Some(token) = from_token.clone() {
                options.from = Some(token);
            }

            let response = room.messages(options).await?;
            let chunk = response.chunk;
            from_token = response.end;
            if chunk.is_empty() {
                break;
            }

            for event in chunk {
                let raw_event = event.raw();
                if let Ok(any_sync_event) = raw_event.deserialize() {
                    if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
                        ) = any_sync_event.clone() {

                        let (sender, timestamp, content) = match msg {
                            matrix_sdk::ruma::events::room::message::SyncRoomMessageEvent::Original(e) => {
                                let timestamp = i64::from(e.origin_server_ts.0) / 1000;

                                (e.sender, timestamp, e.content)
                            }
                            matrix_sdk::ruma::events::room::message::SyncRoomMessageEvent::Redacted(_) => continue,
                        };

                        let (msgtype, body) = match content.msgtype {
                            MessageType::Text(t) => ("text", t.body),
                            MessageType::Notice(n) => ("notice", n.body),
                            MessageType::Image(_) => ("image", "📎 IMAGE".into()),
                            MessageType::Video(_) => ("video", "📎 VIDEO".into()),
                            MessageType::File(_) => ("file", "📎 FILE".into()),
                            MessageType::Audio(_) => ("audio", "📎 AUDIO".into()),
                            MessageType::Location(_) => ("location", "📍 LOCATION".into()),
                            MessageType::Emote(t) => ("emote", t.body),
                            _ => continue,
                        };

                        if timestamp < start_time || timestamp > end_time {
                            continue;
                        }

                        messages.push(WhatsAppMessage {
                            sender: sender.to_string(),
                            sender_display_name: sender.localpart().to_string(),
                            content: body,
                            timestamp,
                            message_type: msgtype.to_string(),
                            room_name: room_name.to_string(),
                        });
                    }
                }
            }

            if from_token.is_none() {
                break;
            }

            // Optional: small delay to respect backpressure
            sleep(Duration::from_millis(100)).await;
        }
    }

    messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(messages)
}


use futures::future::join_all;
use matrix_sdk::room::MessagesOptions;
use tokio::sync::Mutex;
use std::collections::HashMap;

pub async fn get_recent_messages(
    state: &AppState,
    user_id: i32,
    start_time: i64,
    end_time: i64,
) -> Result<Vec<WhatsAppMessage>> {
    let limit_per_room = 10;
    let total_limit = 100;
    
    // First, try to get the client from the state
    let client = {
        let clients = state.matrix_user_clients.lock().await;
        match clients.get(&user_id) {
            Some(client) => client.clone(),
            None => {
                // If client not found in state, try to initialize it
                tracing::info!("Matrix client not found in state for user {}, attempting to initialize", user_id);
                drop(clients); // Release the lock before async operation
                
                // Try to initialize the client
                match crate::utils::matrix_auth::get_or_create_matrix_client(user_id, &state.user_repository).await {
                    Ok(new_client) => {
                        // Store the new client in the state for future use
                        let mut clients = state.matrix_user_clients.lock().await;
                        clients.insert(user_id, new_client.clone());
                        new_client
                    },
                    Err(e) => {
                        return Err(anyhow!("Failed to initialize Matrix client: {}", e));
                    }
                }
            }
        }
    };
    
    // Get all rooms the user has joined
    let rooms = client.joined_rooms();
    tracing::info!("Found {} joined rooms for user {}", rooms.len(), user_id);
    
    if rooms.is_empty() {
        return Ok(Vec::new());
    }

    // Create concurrent tasks to fetch messages from each room
    let fetch_tasks = rooms.into_iter().map(|room| {
        let mut options = MessagesOptions::backward();
        options.limit = matrix_sdk::ruma::UInt::new(limit_per_room).unwrap();
            
        async move {
            match room.messages(options).await {
                Ok(response) => {
                    // Filter for message events only
                    let messages = response
                        .chunk
                        .into_iter()
                        .filter_map(|timeline_event| {
                            if let Ok(any_sync_event) = timeline_event.raw().deserialize() {
                                // Return the AnySyncTimelineEvent directly instead of extracting MessageLike events
                                return Some(any_sync_event);
                            }
                            None
                        })
                        .collect::<Vec<AnySyncTimelineEvent>>();
                    Ok(messages)
                }
                Err(e) => {
                    tracing::error!("Failed to fetch messages for room {}: {}", room.room_id(), e);
                    Err(anyhow!("Failed to fetch messages for room {}: {}", room.room_id(), e))
                }
            }
        }
    });

    // Await all tasks concurrently and collect results
    let results = join_all(fetch_tasks).await;
    
    // Combine successful results and log errors
    let mut all_messages = Vec::new();
    for result in results {
        match result {
            Ok(messages) => all_messages.extend(messages),
            Err(e) => tracing::error!("Error fetching messages: {}", e),
        }
    }

    // Sort messages by timestamp (most recent first)
    all_messages.sort_by(|a, b| {
        // origin_server_ts() returns MilliSecondsSinceUnixEpoch directly, not an Option
        let ts_a = a.origin_server_ts().0;
        let ts_b = b.origin_server_ts().0;
        ts_b.cmp(&ts_a)
    });

    // Convert AnySyncTimelineEvent to WhatsAppMessage
    let whatsapp_messages = all_messages
        .into_iter()
        .take(total_limit)
        .filter_map(|event| {
            if let AnySyncTimelineEvent::MessageLike(
                matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
            ) = event {
                match msg {
                    matrix_sdk::ruma::events::room::message::SyncRoomMessageEvent::Original(e) => {
                        let timestamp = i64::from(e.origin_server_ts.0) / 1000;
                        
                        // Skip messages outside the time range
                        if timestamp < start_time || timestamp > end_time {
                            return None;
                        }
                        
                        let (msgtype, body) = match &e.content.msgtype {
                            MessageType::Text(t) => ("text", t.body.clone()),
                            MessageType::Notice(n) => ("notice", n.body.clone()),
                            MessageType::Image(_) => ("image", "📎 IMAGE".into()),
                            MessageType::Video(_) => ("video", "📎 VIDEO".into()),
                            MessageType::File(_) => ("file", "📎 FILE".into()),
                            MessageType::Audio(_) => ("audio", "📎 AUDIO".into()),
                            MessageType::Location(_) => ("location", "📍 LOCATION".into()),
                            MessageType::Emote(t) => ("emote", t.body.clone()),
                            _ => return None,
                        };
                        
                        // Get room from client
                        let client_ref = &client;
                        // Access the room_id from the event
                        
                        // Get room name
                        let room_name = "test".to_string();
                        
                        // Filter out bridge bot messages - only include messages from WhatsApp users
                        // WhatsApp bridge messages typically come from a specific user or have specific patterns
                        // This is a basic filter - you may need to adjust based on your bridge configuration
                        if e.sender.localpart().contains("whatsappbot") || 
                           e.sender.localpart().contains("whatsapp-bridge") {
                            return None;
                        }
                        
                        Some(WhatsAppMessage {
                            sender: e.sender.to_string(),
                            sender_display_name: e.sender.localpart().to_string(),
                            content: body,
                            timestamp,
                            message_type: msgtype.to_string(),
                            room_name,
                        })
                    },
                    matrix_sdk::ruma::events::room::message::SyncRoomMessageEvent::Redacted(_) => None,
                }
            } else {
                None
            }
        })
        .collect();

    Ok(whatsapp_messages)
}
