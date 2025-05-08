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
    pub last_activity_formatted: String,
}

use chrono::{DateTime, TimeZone, Utc, Local};
use chrono_tz::Tz;

#[derive(Debug, Serialize, Deserialize)]
pub struct WhatsAppMessage {
    pub sender: String,
    pub sender_display_name: String,
    pub content: String,
    pub timestamp: i64,
    pub formatted_timestamp: String,
    pub message_type: String,
    pub room_name: String,
}

use std::time::Duration;
use tokio::time::sleep;

fn format_timestamp(timestamp: i64, timezone: Option<String>) -> String {
    // Convert timestamp to DateTime<Utc>
    let dt_utc = match DateTime::from_timestamp(timestamp, 0) {
        Some(dt) => dt,
        None => return "Invalid timestamp".to_string(),
    };
    
    // Convert to user's timezone if provided, otherwise use UTC
    let formatted = if let Some(tz_str) = timezone {
        match tz_str.parse::<Tz>() {
            Ok(tz) => dt_utc.with_timezone(&tz).format("%Y-%m-%d %H:%M:%S").to_string(),
            Err(_) => {
                tracing::warn!("Invalid timezone '{}', falling back to UTC", tz_str);
                dt_utc.format("%Y-%m-%d %H:%M:%S UTC").to_string()
            }
        }
    } else {
        dt_utc.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    };
    
    formatted
}

pub async fn fetch_whatsapp_messages(
    state: &AppState,
    user_id: i32,
    start_time: i64,
    end_time: i64,
) -> Result<Vec<WhatsAppMessage>> {

    tracing::info!("Fetching WhatsApp messages for user {}", user_id);
    // Get user for timezone info
    let user = state.user_repository.find_by_id(user_id)?
        .ok_or_else(|| anyhow!("User not found"))?;


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
                               body.contains("Decrypting message from WhatsApp failed") ||
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
                                    formatted_timestamp: format_timestamp(timestamp, user.timezone.clone()),
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
    // Get user for timezone info
    let user = state.user_repository.find_by_id(user_id)?
        .ok_or_else(|| anyhow!("User not found"))?;
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
    
    // Find WhatsApp room with exact match (case insensitive)
    let mut target_room = None;
    let search_term_lower = chat_name.trim().to_lowercase();
    let mut found_rooms = Vec::new();
    
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

        // Only process WhatsApp rooms
        if !room_name.contains("(WA)") {
            continue;
        }

        // Extract chat name from room name
        let chat_name_part = room_name
            .split(" (WA)")
            .next()
            .unwrap_or(&room_name)
            .trim()
            .to_string();

        found_rooms.push((room.clone(), chat_name_part.clone()));

        // Check for exact match (case insensitive)
        if chat_name_part.to_lowercase() == search_term_lower {
            target_room = Some(room);
            tracing::info!("Found exact matching room: {}", room_name);
            break;
        }
    }

    let room = if let Some(room) = target_room {
        room
    } else {
        // Provide a helpful error message listing similar rooms
        let similar_rooms: Vec<String> = found_rooms
            .iter()
            .filter(|(_, name)| name.to_lowercase().contains(&search_term_lower))
            .map(|(_, name)| name.clone())
            .collect();

        let error_msg = if similar_rooms.is_empty() {
            format!("Could not find exact matching WhatsApp room for '{}'", chat_name)
        } else {
            format!(
                "Could not find exact matching WhatsApp room for '{}'. Did you mean one of these?\n{}",
                chat_name,
                similar_rooms.join("\n")
            )
        };
        return Err(anyhow!(error_msg));
    };

    // Create message content
    let content = RoomMessageEventContent::text_plain(message);

    // Send the message with transaction ID
    let txn_id = matrix_sdk::ruma::TransactionId::new();
    room.send(content.clone()).with_transaction_id(txn_id).await?;
    println!("Message sent!");

    // Return the sent message details
    let current_timestamp = chrono::Utc::now().timestamp();
    Ok(WhatsAppMessage {
        sender: "You".to_string(),
        sender_display_name: "You".to_string(),
        content: message.to_string(),
        timestamp: current_timestamp,
        formatted_timestamp: format_timestamp(current_timestamp, user.timezone),
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
    // Get user for timezone info
    let user = state.user_repository.find_by_id(user_id)?
        .ok_or_else(|| anyhow!("User not found"))?;
    println!(
        "Starting WhatsApp message fetch - User: {}, Message Limit: {}", 
        user_id, 
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
        println!("Found exact match");
        return fetch_messages_from_room(room.room.clone(), chat_name, limit, user.timezone).await;
    }

    // Substring match
    let substring_matches: Vec<&WhatsAppSearchRoom> = whatsapp_rooms.iter()
        .filter(|r| r.chat_name.to_lowercase().contains(&search_term_lower))
        .collect();
    if !substring_matches.is_empty() {
        let best_room = substring_matches.iter()
            .max_by_key(|r| r.last_activity)
            .unwrap();
        println!("Found substring match with recent activity");
        return fetch_messages_from_room(best_room.room.clone(), chat_name, limit, user.timezone).await;
    }

    // Similarity match
    let similarities: Vec<(f64, &WhatsAppSearchRoom)> = whatsapp_rooms.iter()
        .map(|r| (strsim::jaro_winkler(&search_term_lower, &r.chat_name.to_lowercase()), r))
        .collect();
    if let Some((similarity, room)) = similarities.iter().max_by(|a, b| a.0.partial_cmp(&b.0).unwrap()) {
        if *similarity >= 0.7 {
            println!("Found similar match (score {})", similarity);
            return fetch_messages_from_room(room.room.clone(), chat_name, limit, user.timezone).await;
        }
    }

    Err(anyhow!("No matching WhatsApp room found for '{}'", chat_name))
}

async fn fetch_messages_from_room(
    room: matrix_sdk::room::Room,
    chat_name: &str,
    limit: Option<u64>,
    timezone: Option<String>,
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
                    formatted_timestamp: format_timestamp(timestamp, timezone.clone()),
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
    tracing::info!("Searching WhatsApp rooms for user {} ", user_id);

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

    let mut all_whatsapp_rooms = Vec::new();
    let search_term_lower = search_term.trim().to_lowercase();

    // First pass: collect all WhatsApp rooms with their details
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

        // Skip if bridge bot is not a member or if not a WhatsApp room
        if !has_bridge_bot || !room_name.contains("(WA)") {
            continue;
        }

        // Extract clean room name (without WA suffix)
        let clean_name = room_name
            .split(" (WA)")
            .next()
            .unwrap_or(&room_name)
            .trim()
            .to_string();

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

        all_whatsapp_rooms.push((clean_name, WhatsAppRoom {
            room_id: room.room_id().to_string(),
            display_name: room_name,
            last_activity,
            last_activity_formatted: format_timestamp(last_activity, None),
        }));
    }

    let mut matching_rooms = Vec::new();

    // Exact matches (case insensitive)
    let exact_matches: Vec<WhatsAppRoom> = all_whatsapp_rooms.iter()
        .filter(|(name, _)| name.to_lowercase() == search_term_lower)
        .map(|(_, room)| room.clone())
        .collect();
    matching_rooms.extend(exact_matches);

    // Substring matches
    let substring_matches: Vec<WhatsAppRoom> = all_whatsapp_rooms.iter()
        .filter(|(name, _)| name.to_lowercase().contains(&search_term_lower))
        .filter(|(name, _)| name.to_lowercase() != search_term_lower) // Exclude exact matches
        .map(|(_, room)| room.clone())
        .collect();
    matching_rooms.extend(substring_matches);

    // Similarity matches (if no exact or substring matches found)
    if matching_rooms.is_empty() {
        let mut similarity_matches: Vec<(f64, WhatsAppRoom)> = all_whatsapp_rooms.iter()
            .map(|(name, room)| {
                let similarity = strsim::jaro_winkler(&name.to_lowercase(), &search_term_lower);
                (similarity, room.clone())
            })
            .filter(|(similarity, _)| *similarity >= 0.7) // Only include rooms with high similarity
            .collect();

        // Sort by similarity score (highest first)
        similarity_matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        
        // Add similar rooms to results
        matching_rooms.extend(similarity_matches.into_iter().map(|(_, room)| room));
    }

    // Sort all results by last activity (most recent first)
    matching_rooms.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

    tracing::info!(
        "Found {} matching WhatsApp rooms (including similar matches)",
        matching_rooms.len()
    );

    Ok(matching_rooms)
}


