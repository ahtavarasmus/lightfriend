use std::sync::Arc;
use anyhow::{anyhow, Result};
use matrix_sdk::{
    Client as MatrixClient,
    room::Room,
    ruma::{
        events::room::message::{RoomMessageEventContent, SyncRoomMessageEvent, MessageType},
        events::{AnyTimelineEvent, AnySyncTimelineEvent},
        OwnedRoomId,
        TransactionId,
    },
};
use serde::{Deserialize, Serialize};
use crate::{AppState, models::user_models::Bridge};

#[derive(Debug, Serialize, Clone)]
pub struct WhatsAppRoom {
    pub room_id: String,
    pub display_name: String,
    pub last_activity: i64,
}

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
    tracing::info!("Fetching WhatsApp messages for user {}", user_id);

    // Get Matrix client using get_client function - skip encryption setup as we only need to fetch messages
    let client = crate::utils::matrix_auth::get_client(user_id, &state.user_repository, false).await?;

    // Check if we're logged in first before doing any syncs
    let bridge = state.user_repository.get_whatsapp_bridge(user_id)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("WhatsApp bridge is not connected. Please log in first."));
    }

    // Perform a full sync with longer timeout to ensure we have all rooms
    let sync_settings = matrix_sdk::config::SyncSettings::default().timeout(std::time::Duration::from_secs(10));
    client.sync_once(sync_settings).await?;

    // Get all joined rooms
    let joined_rooms = client.joined_rooms();
    
    tracing::info!("Found {} total joined rooms", joined_rooms.len());
    tracing::info!("Found {} joined rooms for user {}", joined_rooms.len(), user_id);

    // Get bridge bot username from environment variable or use default pattern
    let bridge_bot_username = std::env::var("WHATSAPP_BRIDGE_BOT")
        .unwrap_or_else(|_| "@whatsappbot:".to_string());

    // Create a structure to hold room info with last activity timestamp
    #[derive(Debug)]
    struct RoomInfo {
        room: Room,
        last_activity: i64,
        display_name: String,
    }

    // Collect room info with last activity time
    let mut room_infos = Vec::new();
    for room in joined_rooms {
        let room_id = room.room_id();
        
        // Get room display name
        let display_name = match room.display_name().await {
            Ok(name) => name.to_string(),
            Err(_) => room_id.to_string(),
        };

        // Skip bridge management rooms and non-WhatsApp rooms
        let is_bridge_management = display_name.contains(&bridge_bot_username) || 
                                 display_name.contains("whatsappbot") ||
                                 display_name.contains("whatsapp-bridge") ||
                                 display_name == "WhatsApp Bridge" ||
                                 display_name == "WhatsApp bridge bot";

        // Strict WhatsApp room detection - only rooms with (WA)
        let is_whatsapp_room = display_name.contains("(WA)");

        if !is_bridge_management && is_whatsapp_room {
        
            // Get last message timestamp (if any)
            let mut options = matrix_sdk::room::MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(1).unwrap();
            
            let last_activity = match room.messages(options).await {
                Ok(response) => {
                    if let Some(event) = response.chunk.first() {
                        if let Ok(any_event) = event.raw().deserialize() {
                            i64::from(any_event.origin_server_ts().0) / 1000
                        } else {
                            0
                        }
                    } else {
                        0
                    }
                },
                Err(_) => 0,
            };
        
            room_infos.push(RoomInfo {
                room,
                last_activity,
                display_name,
            });
        }
    }
    
    // Sort rooms by last activity (most recent first)
    room_infos.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
    
    // Take the 50 most recent rooms
    let room_limit = 50;
    let recent_rooms = if room_infos.len() > room_limit {
        room_infos.drain(..room_limit).collect::<Vec<_>>()
    } else {
        room_infos
    };
    
    // Print room information
    tracing::info!("Latest {} rooms:", recent_rooms.len());
    for (i, room_info) in recent_rooms.iter().enumerate() {
        let timestamp_str = if room_info.last_activity > 0 {
            chrono::DateTime::<chrono::Utc>::from_timestamp(room_info.last_activity, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            "no messages".to_string()
        };
        
    }
    
    // Fetch messages from the recent rooms
    let mut messages = Vec::new();
    let messages_per_room = 20; // Limit messages per room
    
    for room_info in recent_rooms {
        let room = room_info.room;
        let room_id = room.room_id();
        let room_name = room_info.display_name;
        
        
        let mut options = matrix_sdk::room::MessagesOptions::backward();
        options.limit = matrix_sdk::ruma::UInt::new(messages_per_room).unwrap();
        
        match room.messages(options).await {
            Ok(response) => {
                let chunk = response.chunk;
                
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

                            // Skip messages outside the time range
                            if timestamp < start_time || timestamp > end_time {
                                continue;
                            }

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

                            // Skip error messages and failed bridge notifications
                            if body.contains("Failed to bridge media") ||
                               body.contains("media no longer available") ||
                               body.starts_with("* Failed to") {
                                continue;
                            }

                            // Strict filtering - only allow messages from whatsapp_ senders in (WA) rooms
                            if sender.localpart().starts_with("whatsapp_") && room_name.contains("(WA)") {
                                messages.push(WhatsAppMessage {
                                    sender: sender.to_string(),
                                    sender_display_name: sender.localpart().to_string(),
                                    content: body,
                                    timestamp,
                                    message_type: msgtype.to_string(),
                                    room_name: room_name.clone(),
                                });
                            }
                        }

                    }
                }
            },
            Err(e) => {
                tracing::error!("Failed to fetch messages from room {}: {}", room_id, e);
                // Continue with other rooms even if one fails
            }

        }
        
        // Optional: small delay to respect backpressure
        sleep(Duration::from_millis(100)).await;
    }

    // Sort all messages by timestamp (most recent first)
    messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    
    tracing::info!("Retrieved a total of {} messages across all rooms", messages.len());
    
    // Print message details in a readable format
    println!("\n📱 WhatsApp Messages Summary:");
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
            "location" => "📍",
            "emote" => "🎭",
            _ => "📝",
        };
        
        println!("\n{} Room: {}", message_type_icon, msg.room_name);
        println!("👤 {}", msg.sender_display_name);
        println!("🕒 {}", datetime);
        println!("📄 {}", msg.content);
        println!("─────────────────────────────────────");
    }
    
    println!("\nTotal messages: {}\n", messages.len());
    Ok(messages)
}


use futures::future::join_all;
use matrix_sdk::room::MessagesOptions;
use tokio::sync::Mutex;
use std::collections::HashMap;


pub async fn send_whatsapp_message(
    state: &AppState,
    user_id: i32,
    chat_name: &str,
    message: &str,
) -> Result<WhatsAppMessage> {
    tracing::info!("Sending WhatsApp message for user {} to room_name {}", user_id, chat_name);

    // Normalize phone number format
    
    tracing::debug!("Attempting to get Matrix client for user {}", user_id);
    let client = match crate::utils::matrix_auth::get_client(user_id, &state.user_repository, false).await {
        Ok(client) => {
            tracing::debug!("Successfully obtained Matrix client");
            client
        },
        Err(e) => {
            tracing::error!("Failed to get Matrix client: {}", e);
            return Err(e);
        }
    };

    tracing::debug!("Checking WhatsApp bridge connection status");
    let bridge = state.user_repository.get_whatsapp_bridge(user_id)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("WhatsApp bridge is not connected. Please log in first."));
    }

    // Perform a quick sync to get latest room state
    client.sync_once(matrix_sdk::config::SyncSettings::default()).await?;

    // Get all joined rooms
    let joined_rooms = client.joined_rooms();
    
    // Find WhatsApp room by checking both formats
    let mut target_room = None;
    
    for room in joined_rooms {
        let room_name = room.display_name().await?.to_string();
        
        
        // Check if room name matches the "<number> (WA)" format
        if room_name.to_lowercase().contains(&chat_name.to_lowercase()){
            target_room = Some(room);
            tracing::info!("Found matching room by room name: {}", room_name);
            break;
        }
        
    }

    let room = target_room.clone().ok_or_else(|| {
        anyhow!("Could not find WhatsApp room for phone number: {:#?}", chat_name)
    })?;

    // Create message content
    let content = RoomMessageEventContent::text_plain(message);

    // Send the message with transaction ID
    let txn_id = matrix_sdk::ruma::TransactionId::new();
    println!("sending message: {:#?} to user: {:#?}", content.clone(), target_room.clone());
    room.send(content.clone()).with_transaction_id(txn_id).await?;

    // Return the sent message details
    Ok(WhatsAppMessage {
        sender: "You".to_string(),
        sender_display_name: "You".to_string(),
        content: message.to_string(),
        timestamp: chrono::Utc::now().timestamp(),
        message_type: "text".to_string(),
        room_name: room.display_name().await?.to_string(),
    })
}

pub async fn fetch_whatsapp_room_messages(
    state: &AppState,
    user_id: i32,
    chat_name: &str,
    limit: Option<u64>,
) -> Result<Vec<WhatsAppMessage>> {
    tracing::info!("Starting WhatsApp message fetch - User: {}, Room: {}, Message Limit: {}", 
        user_id, 
        chat_name, 
        limit.unwrap_or(20)
    );

    // Get Matrix client using get_client function
    let client = crate::utils::matrix_auth::get_client(user_id, &state.user_repository, false).await?;

    // Check if we're logged in first
    let bridge = state.user_repository.get_whatsapp_bridge(user_id)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("WhatsApp bridge is not connected. Please log in first."));
    }

    tracing::debug!("Starting Matrix sync with 5 second timeout");
    let sync_settings = matrix_sdk::config::SyncSettings::default().timeout(std::time::Duration::from_secs(5));
    match client.sync_once(sync_settings).await {
        Ok(_) => tracing::debug!("Matrix sync completed successfully"),
        Err(e) => {
            tracing::error!("Matrix sync failed: {}", e);
            return Err(anyhow!("Failed to sync with Matrix server: {}", e));
        }
    }

    tracing::debug!("Getting list of joined rooms");
    let joined_rooms = client.joined_rooms();
    
    tracing::debug!("Searching for target WhatsApp room matching '{}'", chat_name);
    let mut target_room = None;
    let mut rooms_checked = 0;
    let mut whatsapp_rooms_found = 0;

    for room in joined_rooms {
        rooms_checked += 1;
        let room_name = match room.display_name().await {
            Ok(name) => name.to_string(),
            Err(e) => {
                tracing::warn!("Failed to get display name for room {}: {}", room.room_id(), e);
                continue;
            }
        };
        
        if room_name.contains("(WA)") {
            whatsapp_rooms_found += 1;
            tracing::debug!("Found WhatsApp room: {}", room_name);
            
            if room_name.to_lowercase().contains(&chat_name.to_lowercase()) {
                target_room = Some(room);
                tracing::info!("Found matching room: {}", room_name);
                break;
            }
        }
    }

    tracing::debug!(
        "Room search complete - Checked: {}, WhatsApp rooms found: {}, Match found: {}", 
        rooms_checked, 
        whatsapp_rooms_found, 
        target_room.is_some()
    );

    let room = target_room.ok_or_else(|| {
        anyhow!("Could not find WhatsApp room matching: {}", chat_name)
    })?;

    let room_name = room.display_name().await?.to_string();
    
    // Set up message fetching options
    let mut options = matrix_sdk::room::MessagesOptions::backward();
    options.limit = matrix_sdk::ruma::UInt::new(limit.unwrap_or(20 as u64)).unwrap();
    
    let mut messages = Vec::new();
    
    tracing::debug!("Fetching messages with limit: {}", options.limit);
    match room.messages(options).await {
        Ok(response) => {
            for (index, event) in response.chunk.iter().enumerate() {
                if let Ok(any_sync_event) = event.raw().deserialize() {
                    if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
                        ) = any_sync_event.clone() {

                        let (sender, timestamp, content) = match msg {
                            SyncRoomMessageEvent::Original(e) => {
                                let timestamp = i64::from(e.origin_server_ts.0) / 1000;
                                (e.sender, timestamp, e.content)
                            }
                            SyncRoomMessageEvent::Redacted(_) => continue,
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

                        // Only include messages from whatsapp_ senders
                        if sender.localpart().starts_with("whatsapp_") {
                            messages.push(WhatsAppMessage {
                                sender: sender.to_string(),
                                sender_display_name: sender.localpart().to_string(),
                                content: body,
                                timestamp,
                                message_type: msgtype.to_string(),
                                room_name: room_name.clone(),
                            });
                        }
                    }
                }
            }
        },
        Err(e) => {
            tracing::error!("Failed to fetch messages from room {}: {}", room_name, e);
            return Err(anyhow!("Failed to fetch messages: {}", e));
        }
    }

    // Sort messages by timestamp (most recent first)
    messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    
    let total_messages = messages.len();
    tracing::info!(
        "Message retrieval complete - Room: {}, Total messages: {}, Types: {}",
        room_name,
        total_messages,
        messages.iter()
            .map(|m| m.message_type.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ")
    );

    // Log message timestamps range if messages exist
    if !messages.is_empty() {
        let oldest = messages.iter().map(|m| m.timestamp).min().unwrap();
        let newest = messages.iter().map(|m| m.timestamp).max().unwrap();
        tracing::debug!(
            "Message time range - Oldest: {}, Newest: {}", 
            chrono::DateTime::<chrono::Utc>::from_timestamp(oldest, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            chrono::DateTime::<chrono::Utc>::from_timestamp(newest, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "unknown".to_string())
        );
    }
    
    Ok(messages)
}

pub async fn search_whatsapp_rooms(
    state: &AppState,
    user_id: i32,
    search_term: &str,
) -> Result<Vec<WhatsAppRoom>> {
    tracing::info!("Searching WhatsApp rooms for user {} with term: {}", user_id, search_term);

    // Get Matrix client using get_client function
    let client = crate::utils::matrix_auth::get_client(user_id, &state.user_repository, false).await?;

    // Check if we're logged in first
    let bridge = state.user_repository.get_whatsapp_bridge(user_id)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("WhatsApp bridge is not connected. Please log in first."));
    }

    // Perform a sync to ensure we have latest room state
    let sync_settings = matrix_sdk::config::SyncSettings::default().timeout(std::time::Duration::from_secs(5));
    client.sync_once(sync_settings).await?;

    // Get all joined rooms
    let joined_rooms = client.joined_rooms();
    tracing::info!("Found {} total joined rooms", joined_rooms.len());

    let mut matching_rooms = Vec::new();
    let search_term_lower = search_term.to_lowercase();

    for room in joined_rooms {
        let room_name = room.display_name().await?.to_string();
        
        // Only include WhatsApp rooms (marked with WA)
        if !room_name.contains("(WA)") {
            continue;
        }

        // Check if room name matches search term (case insensitive)
        if room_name.to_lowercase().contains(&search_term_lower) {
            // Get last message timestamp
            let mut options = matrix_sdk::room::MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(1).unwrap();
            
            let last_activity = match room.messages(options).await {
                Ok(response) => {
                    if let Some(event) = response.chunk.first() {
                        if let Ok(any_event) = event.raw().deserialize() {
                            i64::from(any_event.origin_server_ts().0) / 1000
                        } else {
                            0
                        }
                    } else {
                        0
                    }
                },
                Err(_) => 0,
            };

            matching_rooms.push(WhatsAppRoom {
                room_id: room.room_id().to_string(),
                display_name: room_name,
                last_activity,
            });
        }
    }

    // Sort rooms by last activity (most recent first)
    matching_rooms.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

    tracing::info!("Found {} matching WhatsApp rooms", matching_rooms.len());

    // Log the found rooms for debugging
    for room in matching_rooms.iter() {
        let timestamp = if room.last_activity > 0 {
            chrono::DateTime::<chrono::Utc>::from_timestamp(room.last_activity, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            "no messages".to_string()
        };
    }

    Ok(matching_rooms)
}

fn normalize_phone_number(phone: &str) -> Result<(String, String)> {
    // Remove any spaces, dashes, or parentheses
    let cleaned = phone.chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect::<String>();

    // If number doesn't start with +, assume it's a local number and add +358
    let normalized = if !cleaned.starts_with('+') {
        if cleaned.starts_with("0") {
            format!("+358{}", &cleaned[1..])
        } else {
            format!("+358{}", cleaned)
        }
    } else {
        cleaned
    };

    // Validate the number format
    if !normalized.starts_with("+") || normalized.len() < 10 {
        return Err(anyhow!("Invalid phone number format. Please use international format (e.g., +358442105886)"));
    }

    // Create the two formats
    let room_format = format!("{} (WA)", normalized);
    let sender_format = format!("whatsapp_{}", &normalized[1..]); // Remove leading +

    Ok((room_format, sender_format))
}

