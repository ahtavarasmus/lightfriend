
use std::sync::Arc;
use anyhow::{anyhow, Result};
use matrix_sdk::{
    Client as MatrixClient,
    room::Room,
    ruma::{
        events::room::message::{RoomMessageEventContent, SyncRoomMessageEvent, MessageType},
        events::AnySyncTimelineEvent,
    },
};


use serde::{Deserialize, Serialize};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TelegramRoom {
    pub room_id: String,
    pub display_name: String,
    pub last_activity: i64,
    pub last_activity_formatted: String,
}

use chrono::{DateTime, TimeZone};
use chrono_tz::Tz;

#[derive(Debug, Serialize, Deserialize)]
pub struct TelegramMessage {
    pub sender: String,
    pub sender_display_name: String,
    pub content: String,
    pub timestamp: i64,
    pub formatted_timestamp: String,
    pub message_type: String,
    pub room_name: String,
}


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

pub async fn fetch_telegram_messages(
    state: &Arc<AppState>,
    user_id: i32,
    start_time: i64,
    end_time: i64,
) -> Result<Vec<TelegramMessage>> {

    tracing::info!("Fetching Telegram messages for user {}", user_id);
    
    // Get user and user settings for timezone info
    let user = state.user_core.find_by_id(user_id)?
        .ok_or_else(|| anyhow!("User not found"))?;
    let user_settings = state.user_core.get_user_settings(user_id)?;

    // Get Matrix client and check bridge status (use cached version for better performance)
    let client = crate::utils::matrix_auth::get_cached_client(user_id, &state).await?;

    let bridge = state.user_repository.get_telegram_bridge(user_id)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("Telegram bridge is not connected. Please log in first."));
    }

    let bridge_bot_username = std::env::var("TELEGRAM_BRIDGE_BOT")
        .unwrap_or_else(|_| "@telegrambot:".to_string());

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

            // Skip non-Telegram and management rooms early
            if !display_name.contains("(WA)") || 
               display_name.contains("telegrambot") ||
               display_name.contains("telegram-bridge") ||
               display_name == "Telegram Bridge" ||
               display_name == "Telegram bridge bot" {
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
            options.limit = matrix_sdk::ruma::UInt::new(10).unwrap(); // Look at more messages to find Telegram ones
            
            let last_activity = match room.messages(options).await {
                Ok(response) => {
                    // Find the most recent Telegram message timestamp
                    response.chunk.iter()
                        .filter_map(|event| {
                            if let Ok(any_event) = event.raw().deserialize() {
                                if let AnySyncTimelineEvent::MessageLike(
                                    matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
                                ) = any_event {
                                    match msg {
                                        SyncRoomMessageEvent::Original(e) => {
                                            // Only count messages from Telegram users
                                            if e.sender.localpart().starts_with("telegram_") {
                                                Some(i64::from(e.origin_server_ts.0) / 1000)
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                        .next() // Get the first (most recent) Telegram message timestamp
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
    let user_settings = state.user_core.get_user_settings(user_id)?;
    let user_timezone = user_settings.timezone.clone();
        
        futures.push(async move {
    let mut options = matrix_sdk::room::MessagesOptions::backward();
    options.limit = matrix_sdk::ruma::UInt::new(20).unwrap(); // Increase limit to find Telegram messages
            
    match room.messages(options).await {
        Ok(response) => {
            // Process all messages in the chunk to find Telegram messages
            for event in response.chunk {
                if let Ok(any_sync_event) = event.raw().deserialize() {
                    if let AnySyncTimelineEvent::MessageLike(
                        matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
                    ) = any_sync_event {
                        let (sender, timestamp, content) = match msg {
                            SyncRoomMessageEvent::Original(e) => {
                                let timestamp = i64::from(e.origin_server_ts.0) / 1000;
                                (e.sender, timestamp, e.content)
                            }
                            SyncRoomMessageEvent::Redacted(_) => continue,
                        };

                        // Skip messages outside time range
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

                        // Skip error messages
                        if body.contains("Failed to bridge media") ||
                           body.contains("media no longer available") ||
                           body.contains("Decrypting message from Telegram failed") ||
                           body.starts_with("* Failed to") {
                            continue;
                        }

                        // Only include Telegram messages
                        if sender.localpart().starts_with("telegram_") && room_name.contains("(WA)") {
                            return Some(TelegramMessage {
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
            tracing::error!("Failed to fetch messages: {}", e);
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


pub async fn send_telegram_message(
    state: &Arc<AppState>,
    user_id: i32,
    chat_name: &str,
    message: &str,
    media_url: Option<String>,
) -> Result<TelegramMessage> {
    // Get user for timezone info
    let user = state.user_core.find_by_id(user_id)?
        .ok_or_else(|| anyhow!("User not found"))?;
    // Get bridge bot username from environment variable or use default pattern
    let bridge_bot_username = std::env::var("TELEGRAM_BRIDGE_BOT")
        .unwrap_or_else(|_| "@telegrambot:".to_string());
    tracing::info!("Sending Telegram message for user {}", user_id);

    // Normalize phone number format
    
    tracing::debug!("Attempting to get Matrix client for user {}", user_id);
    let client = match crate::utils::matrix_auth::get_cached_client(user_id, &state).await {
        Ok(client) => {
            tracing::debug!("Successfully obtained cached Matrix client");
            client
        },
        Err(e) => {
            tracing::error!("Failed to get Matrix client: {}", e);
            return Err(e);
        }
    };

    tracing::debug!("Checking telegram bridge connection status");
    let bridge = state.user_repository.get_telegram_bridge(user_id)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("Telegram bridge is not connected. Please log in first."));
    }

    // Get all joined rooms
    let joined_rooms = client.joined_rooms();
    
    // Find Telegram room with exact match (case insensitive)
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

        // Only process Telegram rooms
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
            format!("Could not find exact matching Telegram room for '{}'", chat_name)
        } else {
            format!(
                "Could not find exact matching Telegram room for '{}'. Did you mean one of these?\n{}",
                chat_name,
                similar_rooms.join("\n")
            )
        };
        return Err(anyhow!(error_msg));
    };

    use matrix_sdk::{
        attachment::AttachmentConfig,   // optional, for metadata
        ruma::events::room::message::{
            RoomMessageEventContent, MessageType, ImageMessageEventContent,
        },
    };

    // Create message content
    let content = RoomMessageEventContent::text_plain(message);



    if let Some(url) = media_url {

        // ── 1. Download the image and get MIME type ────────────────────────────────
        let resp = reqwest::get(&url).await?;
        
        // Get MIME type from headers before consuming the response
        let mime: mime_guess::mime::Mime = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| mime_guess::MimeGuess::from_path(&url).first_or_octet_stream());

        // Now consume the response to get the bytes
        let bytes = resp.bytes().await?;
        let size = bytes.len();

        // ── 2. Filename (best-effort) ────────────────────────────────────────────

        let filename = std::path::Path::new(&url)
            .file_name()
            .and_then(|p| p.to_str())
            .unwrap_or("file");

        // ── 3. Upload to the homeserver ──────────────────────────────────────────
        // This gives you an `mxc://…` URI without posting anything to the room.
        let upload_resp = client
            .media()
            .upload(&mime, bytes.to_vec(), None)
            .await?;                                     // :contentReference[oaicite:0]{index=0}

        let mxc: matrix_sdk::ruma::OwnedMxcUri = upload_resp.content_uri;

        // ── 4. Build the image-message content with caption in *one* event ──────
        let mut img = ImageMessageEventContent::plain(
            message.to_owned(),   // ← this is the caption / body
            mxc,
        );

        // Optional but nice: add basic metadata so bridges & clients know the size
        let mut imageinfo = matrix_sdk::ruma::events::room::ImageInfo::new();
        imageinfo.size = Some(matrix_sdk::ruma::UInt::new(size as u64).unwrap_or_default());
        img.info = Some(Box::new(
            imageinfo
        ));

        // Wrap it as a generic “m.room.message”
        let content = RoomMessageEventContent::new(MessageType::Image(img));

        // ── 5. Send it ───────────────────────────────────────────────────────────
        room.send(content).await?;

    } else {
        // plain text
        room.send(RoomMessageEventContent::text_plain(message)).await?;
    }
    // Send the message with transaction ID
    let txn_id = matrix_sdk::ruma::TransactionId::new();
    room.send(content.clone()).with_transaction_id(txn_id).await?;
    println!("Message sent!");

    let user_settings = state.user_core.get_user_settings(user_id)?;
    // Return the sent message details
    let current_timestamp = chrono::Utc::now().timestamp();
    Ok(TelegramMessage {
        sender: "You".to_string(),
        sender_display_name: "You".to_string(),
        content: message.to_string(),
        timestamp: current_timestamp,
        formatted_timestamp: format_timestamp(current_timestamp, user_settings.timezone),
        message_type: "text".to_string(),
        room_name: room.display_name().await?.to_string(),
    })
}


use matrix_sdk::RoomMemberships;
use strsim;
use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;

#[derive(Debug)]
struct TelegramSearchRoom {
    room: matrix_sdk::room::Room,
    chat_name: String,
    last_activity: i64,
}

pub async fn fetch_telegram_room_messages(
    state: &Arc<AppState>,
    user_id: i32,
    chat_name: &str,
    limit: Option<u64>,
) -> Result<(Vec<TelegramMessage>, String)> {
    
    tracing::info!(
        "Starting Telegram message fetch - User: {}, Message Limit: {}", 
        user_id, 
        limit.unwrap_or(20)
    );

    // Early validation of bridge status
    if let Some(bridge) = state.user_repository.get_telegram_bridge(user_id)? {
        if bridge.status != "connected" {
            return Err(anyhow!("Telegram bridge is not connected. Please log in first."));
        }
    } else {
        return Err(anyhow!("Telegram bridge not found"));
    }

    let client = crate::utils::matrix_auth::get_cached_client(user_id, &state).await?;

    let bridge_bot_username = std::env::var("TELEGRAM_BRIDGE_BOT")
        .unwrap_or_else(|_| "@telegrambot:".to_string());
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

            // Quick filter for non-Telegram rooms
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

            Some(TelegramSearchRoom {
                room,
                chat_name: chat_name_part,
                last_activity,
            })
        });
    }

    // Collect parallel results
    let telegram_rooms: Vec<TelegramSearchRoom> = join_all(room_futures)
        .await
        .into_iter()
        .flatten()
        .collect();


    // Find matching room with optimized search
    let matching_room = {
        // Try exact match first (fastest)
        if let Some(room) = telegram_rooms.iter().find(|r| r.chat_name.to_lowercase() == search_term_lower) {
            tracing::info!("Found exact match for room");
            Some(room)
        }
        // Then try substring match
        else if let Some(room) = telegram_rooms.iter()
            .filter(|r| r.chat_name.to_lowercase().contains(&search_term_lower))
            .max_by_key(|r| r.last_activity) {
            tracing::info!("Found substring match for room");
            Some(room)
        }
        // Finally try similarity match
        else {
            let best_match = telegram_rooms.iter()
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
            let user_settings = state.user_core.get_user_settings(user_id)?;
            fetch_messages_from_room(room.room.clone(), chat_name, limit, user_settings.timezone).await
        },
        None => Err(anyhow!("No matching Telegram room found for '{}'", chat_name))
    }
}

async fn fetch_messages_from_room(
    room: matrix_sdk::room::Room,
    chat_name: &str,
    limit: Option<u64>,
    timezone: Option<String>,
) -> Result<(Vec<TelegramMessage>, String)> {
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

                // Skip non-Telegram messages early
                if !sender.localpart().starts_with("telegram_") {
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

                Some(TelegramMessage {
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
    let mut messages: Vec<TelegramMessage> = join_all(message_futures)
        .await
        .into_iter()
        .flatten()
        .collect();

    // Sort messages by timestamp (most recent first)
    messages.sort_unstable_by_key(|m| std::cmp::Reverse(m.timestamp));

    Ok((messages, room_name))
}


pub async fn handle_telegram_message(
    event: OriginalSyncRoomMessageEvent,
    room: Room,
    client: MatrixClient,
    state: Arc<AppState>,
) {
    tracing::info!("Entering Telegram message handler");
    // Get room name
    let room_name = match room.display_name().await {
        Ok(name) => name.to_string(),
        Err(e) => {
            tracing::error!("Failed to get room name: {}", e);
            return;
        }
    };

    // Only process Telegram rooms
    if !room_name.contains("(WA)") {
        tracing::info!("Skipping non-Telegram room");
        return;
    }

    // Only process messages from Telegram users
    if !event.sender.localpart().starts_with("telegram_") {
        tracing::debug!(
            "Skipping non-Telegram sender",
        );
        return;
    }

    // Find the user ID for this Matrix client
    let client_user_id = client.user_id().unwrap().to_string();

    // Extract the local part of the Matrix user ID (before the domain)
    let local_user_id = client_user_id
        .split(':')
        .next()
        .map(|s| s.trim_start_matches('@')) // Remove leading '@'
        .unwrap_or(&client_user_id); // Fallback to original if parsing fails

    // Get user for additional info
    let user = match state.user_repository.get_user_by_matrix_user_id(local_user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::error!("User {} not found", local_user_id);
            return;
        },
        Err(e) => {
            tracing::error!("Failed to get user {}: {}", local_user_id, e);
            return;
        }
    };

    let user_id = user.id;



    // Check if user has valid subscription and messages left
    match state.user_repository.has_valid_subscription_tier(user_id, "tier 2") {
        Ok(true) => {
            tracing::info!(
                "User {} has valid subscription and messages for Telegram monitoring",
                user_id
            );
        },
        Ok(false) => {
            tracing::info!(
                "User {} does not have valid subscription or messages left for Telegram monitoring",
                user_id
            );
            return;
        },
        Err(e) => {
            tracing::error!("Failed to check subscription status for user {}: {}", user_id, e);
            return;
        }
    }



    // Extract message content
    let content = match event.content.msgtype {
        MessageType::Text(t) => t.body,
        MessageType::Notice(n) => n.body,
        MessageType::Image(_) => "📎 IMAGE".into(),
        MessageType::Video(_) => "📎 VIDEO".into(),
        MessageType::File(_) => "📎 FILE".into(),
        MessageType::Audio(_) => "📎 AUDIO".into(),
        MessageType::Location(_) => "📍 LOCATION".into(),
        MessageType::Emote(t) => t.body,
        _ => return,
    };

    // Skip error messages
    if content.contains("Failed to bridge media") ||
       content.contains("media no longer available") ||
       content.contains("Decrypting message from Telegram failed") ||
       content.starts_with("* Failed to") {
        tracing::info!("Skipping error message because content contained error messages");
        return;
    }

    // Extract clean chat name from room name
    let chat_name = room_name
        .split(" (WA)")
        .next()
        .unwrap_or(&room_name)
        .trim()
        .to_string();

    // Extract sender name from Matrix user ID
    let sender_name = event.sender.localpart()
        .strip_prefix("telegram_")
        .unwrap_or(event.sender.localpart())
        .to_string();


}



pub async fn search_telegram_rooms(
    state: &Arc<AppState>,
    user_id: i32,
    search_term: &str,
) -> Result<Vec<TelegramRoom>> {
    let bridge_bot_username = std::env::var("TELEGRAM_BRIDGE_BOT")
        .unwrap_or_else(|_| "@telegrambot:".to_string());

    // Validate bridge connection first
    let bridge = state.user_repository.get_telegram_bridge(user_id)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("Telegram bridge is not connected. Please log in first."));
    }

    let client = crate::utils::matrix_auth::get_cached_client(user_id, &state).await?;
    let joined_rooms = client.joined_rooms();
    let search_term_lower = search_term.trim().to_lowercase();

    // Process rooms in parallel
    let room_futures = joined_rooms.into_iter().map(|room| {
        let bridge_bot_username = bridge_bot_username.clone();
        async move {
            // Quick check for room name
            let room_name = match room.display_name().await {
                Ok(name) => name.to_string(),
                Err(_) => return None,
            };

            // Early filter for Telegram rooms
            if !room_name.contains("(WA)") {
                return None;
            }

            // Check bridge bot membership
            let has_bridge_bot = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
                Ok(members) => members.iter().any(|member| 
                    member.user_id().to_string().contains(&bridge_bot_username)
                ),
                Err(_) => return None,
            };

            if !has_bridge_bot {
                return None;
            }

            // Get clean name and last activity
            let clean_name = room_name
                .split(" (WA)")
                .next()
                .unwrap_or(&room_name)
                .trim()
                .to_string();

            // Get last activity timestamp efficiently
            let mut options = matrix_sdk::room::MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(1).unwrap();
            
            let last_activity = match room.messages(options).await {
                Ok(response) => response.chunk.first()
                    .and_then(|event| event.raw().deserialize().ok())
                    .map(|e: AnySyncTimelineEvent| i64::from(e.origin_server_ts().0) / 1000)
                    .unwrap_or(0),
                Err(_) => 0,
            };

            Some((clean_name, TelegramRoom {
                room_id: room.room_id().to_string(),
                display_name: room_name,
                last_activity,
                last_activity_formatted: format_timestamp(last_activity, None),
            }))
        }
    });

    // Collect results from parallel processing
    let all_telegram_rooms: Vec<(String, TelegramRoom)> = join_all(room_futures)
        .await
        .into_iter()
        .flatten()
        .collect();

    // Single-pass matching with prioritized results
    let mut matching_rooms: Vec<(f64, TelegramRoom)> = all_telegram_rooms
        .into_iter()
        .filter_map(|(name, room)| {
            let name_lower = name.to_lowercase();
            if name_lower == search_term_lower {
                // Exact match gets highest priority
                Some((2.0, room))
            } else if name_lower.contains(&search_term_lower) {
                // Substring match gets medium priority
                Some((1.0, room))
            } else {
                // Try similarity match only if needed
                let similarity = strsim::jaro_winkler(&name_lower, &search_term_lower);
                if similarity >= 0.7 {
                    Some((similarity, room))
                } else {
                    None
                }
            }
        })
        .collect();

    // Sort by match quality (higher score = better match) and then by last activity
    matching_rooms.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.1.last_activity.cmp(&a.1.last_activity))
    });

    tracing::info!("Found {} matching Telegram rooms", matching_rooms.len());
    
    Ok(matching_rooms.into_iter().map(|(_, room)| room).collect())
}
