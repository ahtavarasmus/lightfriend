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

    // Get Matrix client and check bridge status
    let client = crate::utils::matrix_auth::get_client(user_id, &state.user_repository, false).await?;

    let bridge = state.user_repository.get_whatsapp_bridge(user_id)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("WhatsApp bridge is not connected. Please log in first."));
    }

    // Quick sync with shorter timeout
    let sync_settings = matrix_sdk::config::SyncSettings::default().timeout(std::time::Duration::from_secs(5));
    client.sync_once(sync_settings).await?;


    let bridge_bot_username = std::env::var("WHATSAPP_BRIDGE_BOT")
        .unwrap_or_else(|_| "@whatsappbot:".to_string());

    // Structure to hold room info
    #[derive(Debug)]
    struct RoomInfo {
        room: Room,
        last_activity: i64,
        display_name: String,
    }

    // Process rooms in parallel
    let joined_rooms = client.joined_rooms();
    let mut room_infos = Vec::new();
    
    let mut futures = Vec::new();
    for room in joined_rooms {
        let bridge_bot_username = bridge_bot_username.clone();
        futures.push(async move {
            let room_id = room.room_id();
            
            // Quick checks first
            let display_name = match room.display_name().await {
                Ok(name) => name.to_string(),
                Err(_) => return None,
            };

            // Skip non-WhatsApp and management rooms early
            if !display_name.contains("(WA)") || 
               display_name.contains("whatsappbot") ||
               display_name.contains("whatsapp-bridge") ||
               display_name == "WhatsApp Bridge" ||
               display_name == "WhatsApp bridge bot" {
                return None;
            }

            // Check bridge bot membership
            let members = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
                Ok(members) => members,
                Err(_) => return None,
            };

            if !members.iter().any(|member| member.user_id().to_string().contains(&bridge_bot_username)) {
                return None;
            }

            // Get last message timestamp
            let mut options = matrix_sdk::room::MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(1).unwrap();
            
            let last_activity = match room.messages(options).await {
                Ok(response) => {
                    response.chunk.first()
                        .and_then(|event| event.raw().deserialize().ok())
                        .map(|event: AnySyncTimelineEvent| i64::from(event.origin_server_ts().0) / 1000)
                        .unwrap_or(0)
                },
                Err(_) => 0,
            };

            Some(RoomInfo {
                room,
                last_activity,
                display_name,
            })
        });
    }

    // Collect results from parallel processing
    let results = join_all(futures).await;
    room_infos.extend(results.into_iter().flatten());
    
    // Sort and take top 10 most recent rooms
    room_infos.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
    let recent_rooms = room_infos.into_iter().take(10).collect::<Vec<_>>();
    
    // Fetch latest message from each room
    let mut messages = Vec::new();
    
    // Process rooms in parallel
    let mut futures = Vec::new();
    for room_info in recent_rooms {
        let room = room_info.room;

        let room_name = room_info.display_name;
        let user_timezone = user.timezone.clone();
        
        futures.push(async move {
            let mut options = matrix_sdk::room::MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(1).unwrap(); // Only get latest message
            
            match room.messages(options).await {
                Ok(response) => {
                    if let Some(event) = response.chunk.first() {
                        if let Ok(any_sync_event) = event.raw().deserialize() {
                            if let AnySyncTimelineEvent::MessageLike(
                                matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
                            ) = any_sync_event {
                                let (sender, timestamp, content) = match msg {
                                    SyncRoomMessageEvent::Original(e) => {
                                        let timestamp = i64::from(e.origin_server_ts.0) / 1000;
                                        (e.sender, timestamp, e.content)
                                    }
                                    SyncRoomMessageEvent::Redacted(_) => return None,
                                };

                                // Skip messages outside time range
                                if timestamp < start_time {
                                    return None;
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
                                    _ => return None,
                                };

                                // Skip error messages
                                if body.contains("Failed to bridge media") ||
                                   body.contains("media no longer available") ||
                                   body.contains("Decrypting message from WhatsApp failed") ||
                                   body.starts_with("* Failed to") {
                                    return None;
                                }

                                // Only include WhatsApp messages
                                if sender.localpart().starts_with("whatsapp_") && room_name.contains("(WA)") {
                                    return Some(WhatsAppMessage {
                                        sender: sender.to_string(),
                                        sender_display_name: sender.localpart().to_string(),
                                        content: body,
                                        timestamp,
                                        formatted_timestamp: format_timestamp(timestamp, user_timezone),
                                        message_type: msgtype.to_string(),
                                        room_name: room_name.clone(),
                                    });
                                }
                            }
                        }

                    }
                }
                Err(e) => {
                    tracing::error!("Failed to fetch message: {}", e);
                }
            }
            None
        });
    }

    // Collect results from parallel processing
    let results = join_all(futures).await;
    messages.extend(results.into_iter().flatten());

    // Sort by timestamp (most recent first)
    messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    
    tracing::info!("Retrieved {} latest messages from most active rooms", messages.len());
    

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

    let user = state.user_repository.find_by_id(user_id)?
        .ok_or_else(|| anyhow!("User not found"))?;
    
    tracing::info!(
        "Starting WhatsApp message fetch - User: {}, Message Limit: {}", 
        user_id, 
        limit.unwrap_or(20)
    );

    // Early validation of bridge status
    if let Some(bridge) = state.user_repository.get_whatsapp_bridge(user_id)? {
        if bridge.status != "connected" {
            return Err(anyhow!("WhatsApp bridge is not connected. Please log in first."));
        }
    } else {
        return Err(anyhow!("WhatsApp bridge not found"));
    }

    let client = crate::utils::matrix_auth::get_client(user_id, &state.user_repository, false).await?;
    
    // Faster sync with shorter timeout
    let sync_settings = matrix_sdk::config::SyncSettings::default()
        .timeout(std::time::Duration::from_secs(3));
    client.sync_once(sync_settings).await?;

    let bridge_bot_username = std::env::var("WHATSAPP_BRIDGE_BOT")
        .unwrap_or_else(|_| "@whatsappbot:".to_string());
    let joined_rooms = client.joined_rooms();
    let search_term_lower = chat_name.trim().to_lowercase();

    // Process rooms in parallel
    let mut room_futures = Vec::new();
    for room in joined_rooms {
        let bridge_bot_username = bridge_bot_username.clone();
        room_futures.push(async move {
            let display_name = match room.display_name().await {
                Ok(name) => name.to_string(),
                Err(_) => return None,
            };

            // Quick filter for non-WhatsApp rooms
            if !display_name.contains("(WA)") {
                return None;
            }

            // Check bridge bot membership efficiently
            let members = match room.members(RoomMemberships::JOIN).await {
                Ok(members) => members,
                Err(_) => return None,
            };

            if !members.iter().any(|m| m.user_id().to_string().contains(&bridge_bot_username)) {
                return None;
            }

            let chat_name_part = display_name
                .split(" (WA)")
                .next()
                .unwrap_or(&display_name)
                .trim()
                .to_string();

            // Get last activity timestamp efficiently
            let mut options = MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(1).unwrap();
            let last_activity = match room.messages(options).await {
                Ok(resp) => resp.chunk.first()
                    .and_then(|e| e.raw().deserialize().ok())
                    .map(|e: AnySyncTimelineEvent| i64::from(e.origin_server_ts().0) / 1000)
                    .unwrap_or(0),
                Err(_) => 0,
            };

            Some(WhatsAppSearchRoom {
                room,
                chat_name: chat_name_part,
                last_activity,
            })
        });
    }

    // Collect parallel results
    let whatsapp_rooms: Vec<WhatsAppSearchRoom> = join_all(room_futures)
        .await
        .into_iter()
        .flatten()
        .collect();


    // Find matching room with optimized search
    let matching_room = {
        // Try exact match first (fastest)
        if let Some(room) = whatsapp_rooms.iter().find(|r| r.chat_name.to_lowercase() == search_term_lower) {
            tracing::info!("Found exact match for room");
            Some(room)
        }
        // Then try substring match
        else if let Some(room) = whatsapp_rooms.iter()
            .filter(|r| r.chat_name.to_lowercase().contains(&search_term_lower))
            .max_by_key(|r| r.last_activity) {
            tracing::info!("Found substring match for room");
            Some(room)
        }
        // Finally try similarity match
        else {
            let best_match = whatsapp_rooms.iter()
                .map(|r| (strsim::jaro_winkler(&search_term_lower, &r.chat_name.to_lowercase()), r))
                .filter(|(score, _)| *score >= 0.7)
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            
            if let Some((score, room)) = best_match {
                tracing::info!("Found similar match with score {}", score);
                Some(room)
            } else {
                None
            }
        }
    };

    match matching_room {
        Some(room) => {
            fetch_messages_from_room(room.room.clone(), chat_name, limit, user.timezone).await
        },
        None => Err(anyhow!("No matching WhatsApp room found for '{}'", chat_name))
    }
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


    let response = room.messages(options).await?;
    
    // Process messages in parallel
    let mut message_futures = Vec::with_capacity(response.chunk.len());
    let room_name_clone = room_name.clone();
    
    for event in response.chunk {
        let timezone = timezone.clone();
        let room_name = room_name_clone.clone();
        
        message_futures.push(async move {
            if let Ok(AnySyncTimelineEvent::MessageLike(
                matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
            )) = event.raw().deserialize() {
                let (sender, timestamp, content) = match msg {
                    SyncRoomMessageEvent::Original(e) => (e.sender, i64::from(e.origin_server_ts.0) / 1000, e.content),
                    SyncRoomMessageEvent::Redacted(_) => return None,
                };

                // Skip non-WhatsApp messages early
                if !sender.localpart().starts_with("whatsapp_") {
                    return None;
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
                    _ => return None,
                };

                Some(WhatsAppMessage {
                    sender: sender.to_string(),
                    sender_display_name: sender.localpart().to_string(),
                    content: body,
                    timestamp,
                    formatted_timestamp: format_timestamp(timestamp, timezone),
                    message_type: msgtype.to_string(),
                    room_name: room_name.clone(),
                })
            } else {
                None
            }
        });
    }

    // Collect results from parallel processing
    let mut messages: Vec<WhatsAppMessage> = join_all(message_futures)
        .await
        .into_iter()
        .flatten()
        .collect();

    // Sort messages by timestamp (most recent first)
    messages.sort_unstable_by_key(|m| std::cmp::Reverse(m.timestamp));

    Ok((messages, room_name))
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


