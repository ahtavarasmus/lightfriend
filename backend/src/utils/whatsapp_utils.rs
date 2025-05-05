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

        // Get room members
        let members = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
            Ok(members) => members,
            Err(_) => continue,
        };

        // Check if bridge bot is a member of the room
        let has_bridge_bot = members.iter().any(|member| 
            member.user_id().to_string().contains(&bridge_bot_username)
        );

        // Skip bridge management rooms
        let is_bridge_management = display_name.contains("whatsappbot") ||
                                 display_name.contains("whatsapp-bridge") ||
                                 display_name == "WhatsApp Bridge" ||
                                 display_name == "WhatsApp bridge bot";

        if !is_bridge_management {
        
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
                            if timestamp < start_time {
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
                            //if sender.localpart().starts_with("whatsapp_") && room_name.contains("(WA)") {
                                messages.push(WhatsAppMessage {
                                    sender: sender.to_string(),
                                    sender_display_name: sender.localpart().to_string(),
                                    content: body,
                                    timestamp,
                                    message_type: msgtype.to_string(),
                                    room_name: room_name.clone(),
                                });
                            //}
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
    // Get bridge bot username from environment variable or use default pattern
    let bridge_bot_username = std::env::var("WHATSAPP_BRIDGE_BOT")
        .unwrap_or_else(|_| "@whatsappbot:".to_string());
    tracing::info!("Sending WhatsApp message for user {}", user_id);

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
        
            
        // Get room members
        let members = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
            Ok(members) => members,
            Err(_) => continue,
        };

        // Check if bridge bot is a member of the room
        let has_bridge_bot = members.iter().any(|member| 
            member.user_id().to_string().contains(&bridge_bot_username)
        );
        if !has_bridge_bot {
            continue;
        }
        // Split the room name by " (WA)" to isolate the chat name
        let parts = room_name.split(" (WA)").collect::<Vec<&str>>();
        if parts.len() > 0 && parts[0].trim().to_lowercase() == chat_name.trim().to_lowercase() {
            target_room = Some(room);
            tracing::info!("Found matching room: {}", room_name);
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
    room.send(content.clone()).with_transaction_id(txn_id).await?;
    println!("just sent the message");

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


use matrix_sdk::RoomMemberships;
use strsim;

#[derive(Debug)]
struct WhatsAppSearchRoom {
    room: matrix_sdk::room::Room,
    chat_name: String,
    last_activity: i64,
}

pub async fn fetch_whatsapp_room_messages(
    state: &AppState,
    user_id: i32,
    chat_name: &str,
    limit: Option<u64>,
) -> Result<(Vec<WhatsAppMessage>, String)> {
    println!(
        "Starting WhatsApp message fetch - User: {}, Room: {}, Message Limit: {}", 
        user_id, 
        chat_name, 
        limit.unwrap_or(20)
    );

    let client = crate::utils::matrix_auth::get_client(user_id, &state.user_repository, false).await?;
    let bridge = state.user_repository.get_whatsapp_bridge(user_id)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("WhatsApp bridge is not connected. Please log in first."));
    }

    let sync_settings = matrix_sdk::config::SyncSettings::default().timeout(std::time::Duration::from_secs(5));
    client.sync_once(sync_settings).await.map_err(|e| anyhow!("Failed to sync: {}", e))?;

    let bridge_bot_username = std::env::var("WHATSAPP_BRIDGE_BOT").unwrap_or_else(|_| "@whatsappbot:".to_string());
    let joined_rooms = client.joined_rooms();

    // Fetch all WhatsApp rooms
    let mut whatsapp_rooms = Vec::new();
    for room in joined_rooms {
        let members = room.members(RoomMemberships::JOIN).await?;
        if !members.iter().any(|m| m.user_id().to_string().contains(&bridge_bot_username)) {
            continue;
        }

        let display_name = room.display_name().await?.to_string();
        if !display_name.contains("(WA)") {
            continue;
        }

        let chat_name_part = display_name.split(" (WA)").next().unwrap_or(&display_name).trim().to_string();
        let mut options = MessagesOptions::backward();
        options.limit = matrix_sdk::ruma::UInt::new(1).unwrap();
        let last_activity = match room.messages(options).await {
            Ok(resp) => resp.chunk.first()
                .and_then(|e| e.raw().deserialize().ok())
                .map(|e: AnySyncTimelineEvent| i64::from(e.origin_server_ts().0) / 1000)
                .unwrap_or(0),
            Err(_) => 0,
        };

        whatsapp_rooms.push(WhatsAppSearchRoom {
            room,
            chat_name: chat_name_part,
            last_activity,
        });
    }

    // Find the best matching room
    let search_term_lower = chat_name.trim().to_lowercase();
    
    // Exact match
    if let Some(room) = whatsapp_rooms.iter().find(|r| r.chat_name.to_lowercase() == search_term_lower) {
        println!("Found exact match: {}", room.chat_name);
        return fetch_messages_from_room(room.room.clone(), chat_name, limit).await;
    }

    // Substring match
    let substring_matches: Vec<&WhatsAppSearchRoom> = whatsapp_rooms.iter()
        .filter(|r| r.chat_name.to_lowercase().contains(&search_term_lower))
        .collect();
    if !substring_matches.is_empty() {
        let best_room = substring_matches.iter()
            .max_by_key(|r| r.last_activity)
            .unwrap();
        println!("Found substring match with recent activity: {}", best_room.chat_name);
        return fetch_messages_from_room(best_room.room.clone(), chat_name, limit).await;
    }

    // Similarity match
    let similarities: Vec<(f64, &WhatsAppSearchRoom)> = whatsapp_rooms.iter()
        .map(|r| (strsim::jaro_winkler(&search_term_lower, &r.chat_name.to_lowercase()), r))
        .collect();
    if let Some((similarity, room)) = similarities.iter().max_by(|a, b| a.0.partial_cmp(&b.0).unwrap()) {
        if *similarity >= 0.7 {
            println!("Found similar match (score {}): {}", similarity, room.chat_name);
            return fetch_messages_from_room(room.room.clone(), chat_name, limit).await;
        }
    }

    Err(anyhow!("No matching WhatsApp room found for '{}'", chat_name))
}

async fn fetch_messages_from_room(
    room: matrix_sdk::room::Room,
    chat_name: &str,
    limit: Option<u64>,
) -> Result<(Vec<WhatsAppMessage>, String)> {
    let room_name = room.display_name().await?.to_string();
    let mut options = MessagesOptions::backward();
    options.limit = matrix_sdk::ruma::UInt::new(limit.unwrap_or(20)).unwrap();

    let mut messages = Vec::new();
    let response = room.messages(options).await?;
    for event in response.chunk {
        if let Ok(AnySyncTimelineEvent::MessageLike(
            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
        )) = event.raw().deserialize() {
            let (sender, timestamp, content) = match msg {
                SyncRoomMessageEvent::Original(e) => (e.sender, i64::from(e.origin_server_ts.0) / 1000, e.content),
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

    messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok((messages, room.display_name().await?.to_string()))
}


pub async fn search_whatsapp_rooms(
    state: &AppState,
    user_id: i32,
    search_term: &str,
) -> Result<Vec<WhatsAppRoom>> {
    // Get bridge bot username from environment variable or use default pattern
    let bridge_bot_username = std::env::var("WHATSAPP_BRIDGE_BOT")
        .unwrap_or_else(|_| "@whatsappbot:".to_string());
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
        
    // Get room members
    let members = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
        Ok(members) => members,
        Err(_) => continue,
    };

    // Check if bridge bot is a member of the room
    let has_bridge_bot = members.iter().any(|member| 
        member.user_id().to_string().contains(&bridge_bot_username)
    );

    // Skip if bridge bot is not a member
    if !has_bridge_bot {
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

    Ok(matching_rooms)
}


