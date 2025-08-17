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
pub struct BridgeRoom {
    pub room_id: String,
    pub display_name: String,
    pub last_activity: i64,
    pub last_activity_formatted: String,
}


use chrono::{DateTime, TimeZone};
use chrono_tz::Tz;

#[derive(Debug, Serialize, Deserialize)]
pub struct BridgeMessage {
    pub sender: String,
    pub sender_display_name: String,
    pub content: String,
    pub timestamp: i64,
    pub formatted_timestamp: String,
    pub message_type: String,
    pub room_name: String,
    pub media_url: Option<String>,
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

fn get_bridge_bot_username(service: &str) -> String {
    let env_key = format!("{}_BRIDGE_BOT", service.to_uppercase());
    std::env::var(env_key).unwrap_or_else(|_| format!("@{}bot:", service).to_string())
}

fn get_sender_prefix(service: &str) -> String {
    format!("{}_", service)
}

fn get_room_suffix(service: &str) -> String {
    match service {
        "whatsapp" => "(WA)".to_string(),
        "telegram" => "(Telegram)".to_string(),
        "signal" => "(Signal)".to_string(),
        _ => format!(" ({})", service.to_uppercase()),
    }
}

fn infer_service(room_name: &str, sender_localpart: &str) -> Option<String> {
    let sender_localpart = sender_localpart.trim().to_lowercase();
    let room_name = room_name.to_lowercase();

    if room_name.contains("(wa)") || sender_localpart.starts_with("whatsapp_") || sender_localpart.starts_with("whatsapp") {
        println!("Detected WhatsApp");
        return Some("whatsapp".to_string());
    }
    if room_name.contains("(tg)") || sender_localpart.starts_with("telegram_") || sender_localpart.starts_with("telegram") {
        println!("Detected Telegram");
        return Some("telegram".to_string());
    }
    if room_name.contains("Signal") || sender_localpart.starts_with("signal_") || sender_localpart.starts_with("signal") {
        println!("Detected Signal");
        return Some("signal".to_string());
    }
    println!("No service detected");
    None
}

pub async fn fetch_bridge_messages(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    start_time: i64,
    unread_only: bool,
) -> Result<Vec<BridgeMessage>> {

    tracing::info!("Fetching {} messages for user {}", service, user_id);
    
    // Get user and user settings for timezone info
    let user = state.user_core.find_by_id(user_id)?
        .ok_or_else(|| anyhow!("User not found"))?;
    let user_settings = state.user_core.get_user_settings(user_id)?;
    let user_info= state.user_core.get_user_info(user_id)?;

    // Get Matrix client and check bridge status (use cached version for better performance)
    let client = crate::utils::matrix_auth::get_cached_client(user_id, &state).await?;

    let bridge = state.user_repository.get_bridge(user_id, service)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("{} bridge is not connected. Please log in first.", capitalize(&service)));
    }

    let room_suffix = get_room_suffix(service);

    let sender_prefix = get_sender_prefix(service);

    let service_cap = capitalize(&service);

    let skip_terms = vec![
        format!("{}bot", service),
        format!("{}-bridge", service),
        format!("{} Bridge", service_cap),
        format!("{} bridge bot", service_cap),
    ];

    // Structure to hold room info
    struct RoomInfo {
        room: Room,
        last_activity: i64,
        display_name: String,
    }

    // Process rooms in parallel
    let joined_rooms = client.joined_rooms();
    
    let mut futures = Vec::new();
    for room in joined_rooms {
        let room_suffix = room_suffix.clone();
        let skip_terms = skip_terms.clone();
        let sender_prefix = sender_prefix.clone();
        futures.push(async move {
            let room_id = room.room_id();
            
            // Quick checks first
            let display_name = match room.display_name().await {
                Ok(name) => name.to_string(),
                Err(_) => return None,
            };

            if skip_terms.iter().any(|t| display_name.contains(t)){
                return None;
            }

            if unread_only && room.unread_notification_counts().notification_count == 0 {
                return None; 
            }
            let members = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
                Ok(members) => members,
                Err(_) => return None,
            };
            // Check for at least one member with the service prefix to confirm service type
            let has_service_member = members.iter().any(|member| member.user_id().localpart().starts_with(&sender_prefix));
            if !has_service_member {
                return None;
            }

            // Get last message timestamp
            let mut options = matrix_sdk::room::MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(10).unwrap();
            
            let last_activity = match room.messages(options).await {
                Ok(response) => {
                    // Find the most recent WhatsApp message timestamp
                    response.chunk.iter()
                        .filter_map(|event| {
                            if let Ok(any_event) = event.raw().deserialize() {
                                if let AnySyncTimelineEvent::MessageLike(
                                    matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
                                ) = any_event {
                                    if let SyncRoomMessageEvent::Original(e) = msg {
                                        if e.sender.localpart().starts_with(&sender_prefix) {
                                            Some(i64::from(e.origin_server_ts.0) / 1000)
                                        } else {
                                            None
                                        }
                                    } else { None }
                                } else { None }
                            } else { None }
                        })
                        .next() // Get the first (most recent) WhatsApp message timestamp
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
    let mut room_infos = results.into_iter().flatten().collect::<Vec<_>>();
    
    room_infos.sort_by_key(|r| std::cmp::Reverse(r.last_activity));
    let recent_rooms = room_infos.into_iter().take(10).collect::<Vec<_>>();
    
    // Fetch latest message from each room
    let user_timezone = user_info.timezone.clone();
    
    // Process rooms in parallel
    let mut futures = Vec::new();
    for room_info in recent_rooms {
        let room = room_info.room.clone();
        let room_name = room_info.display_name.clone();
        let user_timezone = user_timezone.clone();
        let room_suffix = room_suffix.clone();
        let sender_prefix = sender_prefix.clone();

        futures.push(async move {
            let mut options = matrix_sdk::room::MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(20).unwrap();
                    
            match room.messages(options).await {
                Ok(response) => {
                    // Process all messages in the chunk to find WhatsApp messages
                    for event in response.chunk.iter() {
                        if let Ok(any_sync_event) = event.raw().deserialize() {
                            if let AnySyncTimelineEvent::MessageLike(
                                matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
                            ) = any_sync_event {
                                let (sender, timestamp, content) = match msg {
                                    SyncRoomMessageEvent::Original(e) => {
                                        let timestamp = i64::from(e.origin_server_ts.0) / 1000;
                                        (e.sender, timestamp, e.content)
                                    }
                                    _ => continue,
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
                                   body.contains("Decrypting message from WhatsApp failed") ||
                                   body.starts_with("* Failed to") {
                                    continue;
                                }
                                if sender.localpart().starts_with(&sender_prefix) && room_name.contains(&room_suffix) {
                                    return Some(BridgeMessage {
                                        sender: sender.to_string(),
                                        sender_display_name: sender.localpart().to_string(),
                                        content: body,
                                        timestamp,
                                        formatted_timestamp: format_timestamp(timestamp, user_timezone),
                                        message_type: msgtype.to_string(),
                                        room_name: room_name.clone(),
                                        media_url: None,
                                    });
                                }
                            }
                        }
                    }
                }
                Err(e) => tracing::error!("Failed to fetch messages: {}", e),
            }
            None
        });
    }

    // Collect results from parallel processing
    let results = join_all(futures).await;
    let mut messages = results.into_iter().flatten().collect::<Vec<_>>();

    // Sort by timestamp (most recent first)
    messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    tracing::info!("Retrieved {} latest messages from most active rooms", messages.len());

    Ok(messages)
}


use futures::future::join_all;
use matrix_sdk::room::MessagesOptions;
use tokio::sync::Mutex;
use std::collections::HashMap;


pub async fn send_bridge_message(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    chat_name: &str,
    message: &str,
    media_url: Option<String>,
) -> Result<BridgeMessage> {
    // Get user for timezone info
    tracing::info!("Sending {} message", service);
    
    let client = crate::utils::matrix_auth::get_cached_client(user_id, &state).await?;

    let bridge = state.user_repository.get_bridge(user_id, service)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("{} bridge is not connected. Please log in first.", capitalize(&service)));
    }

    // Get all joined rooms
    let joined_rooms = client.joined_rooms();
    let room_suffix = get_room_suffix(service);
    let sender_prefix = get_sender_prefix(service);
    let service_cap = capitalize(&service);
    let skip_terms = vec![
        format!("{}bot", service),
        format!("{}-bridge", service),
        format!("{} Bridge", service_cap),
        format!("{} bridge bot", service_cap),
    ];
    let search_term_lower = chat_name.trim().to_lowercase();

    let mut futures = Vec::new();
    for room in joined_rooms {
        let room_suffix = room_suffix.clone();
        let sender_prefix = sender_prefix.clone();
        let skip_terms = skip_terms.clone();
        futures.push(async move {
            let display_name = match room.display_name().await {
                Ok(n) => n.to_string(),
                Err(_) => return None,
            };
            if skip_terms.iter().any(|t| display_name.contains(t)) {
                return None;
            }
            // Get room members
            let members = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
                Ok(members) => members,
                Err(_) => return None,
            };
            let has_service_member = members.iter().any(|member| member.user_id().localpart().starts_with(&sender_prefix));
            if !has_service_member {
                return None;
            }
            let chat_name_part = if display_name.contains(&room_suffix) {
                display_name
                    .split(&room_suffix)
                    .next()
                    .unwrap_or(&display_name)
                    .trim()
                    .to_string()
            } else {
                display_name.trim().to_string()
            };
            Some((room, chat_name_part))
        });
    }

    // Collect results
    let found_rooms: Vec<(Room, String)> = join_all(futures)
        .await
        .into_iter()
        .flatten()
        .collect();
    // Find exact match
    let target_room = found_rooms.iter()
        .find(|(_, name)| name.to_lowercase() == search_term_lower)
        .map(|(room, _)| room.clone());
    let room = match target_room {
        Some(r) => r,
        None => {
            // Provide a helpful error message listing similar rooms
            let similar_rooms: Vec<String> = found_rooms
                .iter()
                .filter(|(_, name)| name.to_lowercase().contains(&search_term_lower))
                .map(|(_, name)| name.clone())
                .collect();
            let error_msg = if similar_rooms.is_empty() {
                format!("Could not find exact matching {} room for '{}'", capitalize(&service), chat_name)
            } else {
                format!(
                    "Could not find exact matching {} room for '{}'. Did you mean one of these?\n{}",
                    capitalize(&service),
                    chat_name,
                    similar_rooms.join("\n")
                )
            };
            return Err(anyhow!(error_msg));
        }
    };

    use matrix_sdk::{
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
        let upload_resp = client
            .media()
            .upload(&mime, bytes.to_vec(), None)
            .await?;

        let mxc: matrix_sdk::ruma::OwnedMxcUri = upload_resp.content_uri;

        // ── 4. Build the image-message content with caption in *one* event ──────
        let mut img = ImageMessageEventContent::plain(
            message.to_owned(),   // ← this is the caption / body
            mxc,
        );

        // Optional but nice: add basic metadata so bridges & clients know the size
        let mut imageinfo = matrix_sdk::ruma::events::room::ImageInfo::new();
        imageinfo.size = Some(matrix_sdk::ruma::UInt::new(size as u64).unwrap_or_default());
        img.info = Some(Box::new(imageinfo));

        // Wrap it as a generic “m.room.message”
        let content = RoomMessageEventContent::new(MessageType::Image(img));

        // ── 5. Send it ───────────────────────────────────────────────────────────
        room.send(content).await?;
    } else {
        // plain text
        room.send(RoomMessageEventContent::text_plain(message)).await?;
    }
    tracing::debug!("Message sent!");

    let user_info= state.user_core.get_user_info(user_id)?;
    let current_timestamp = chrono::Utc::now().timestamp();
    // Return the sent message details
    Ok(BridgeMessage {
        sender: "You".to_string(),
        sender_display_name: "You".to_string(),
        content: message.to_string(),
        timestamp: current_timestamp,
        formatted_timestamp: format_timestamp(current_timestamp, user_info.timezone),
        message_type: "text".to_string(),
        room_name: room.display_name().await?.to_string(),
        media_url: None,
    })
}


use matrix_sdk::RoomMemberships;
use strsim;
use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;

#[derive(Debug)]
struct BridgeSearchRoom {
    room: matrix_sdk::room::Room,
    chat_name: String,
    last_activity: i64,
}


pub async fn fetch_bridge_room_messages(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    chat_name: &str,
    limit: Option<u64>,
) -> Result<(Vec<BridgeMessage>, String)> {
    
    tracing::debug!(
        "Starting {} message fetch - User: {}, chat: {}, limit: {}", 
        capitalize(&service),
        user_id, 
        chat_name,
        limit.unwrap_or(20)
    );

    if let Some(bridge) = state.user_repository.get_bridge(user_id, service)? {
        if bridge.status != "connected" {
            return Err(anyhow!("{} bridge is not connected. Please log in first.", capitalize(&service)));
        }
    } else {
        return Err(anyhow!("{} bridge not found", capitalize(&service)));
    }

    let client = crate::utils::matrix_auth::get_cached_client(user_id, &state).await?;

    let room_suffix = get_room_suffix(service);
    let sender_prefix = get_sender_prefix(service);
    let joined_rooms = client.joined_rooms();
    let search_term_lower = chat_name.trim().to_lowercase();
    let service_cap = capitalize(&service);
    let skip_terms = vec![
        format!("{}bot", service),
        format!("{}-bridge", service),
        format!("{} Bridge", service_cap),
        format!("{} bridge bot", service_cap),
    ];

    let mut futures = Vec::new();
    for room in joined_rooms {
        let room_suffix = room_suffix.clone();
        let sender_prefix = sender_prefix.clone();
        let skip_terms = skip_terms.clone();
        futures.push(async move {
            let display_name = match room.display_name().await {
                Ok(name) => name.to_string(),
                Err(_) => return None,
            };

            if skip_terms.iter().any(|t| display_name.contains(t)) {
                return None;
            }

            // Check bridge bot membership efficiently
            let members = match room.members(RoomMemberships::JOIN).await {
                Ok(members) => members,
                Err(_) => return None,
            };

            let has_service_member = members.iter().any(|member| member.user_id().localpart().starts_with(&sender_prefix));
            if !has_service_member {
                return None;
            }
            let chat_name_part = if display_name.contains(&room_suffix) {
                display_name
                    .split(&room_suffix)
                    .next()
                    .unwrap_or(&display_name)
                    .trim()
                    .to_string()
            } else {
                display_name.trim().to_string()
            };

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

            Some(BridgeSearchRoom {
                room,
                chat_name: chat_name_part,
                last_activity,
            })
        });
    }

    // Collect parallel results
    let bridge_rooms: Vec<BridgeSearchRoom> = join_all(futures)
        .await
        .into_iter()
        .flatten()
        .collect();


    // Find matching room with optimized search
    let matching_room = {
        // Try exact match first (fastest)
        if let Some(room) = bridge_rooms.iter().find(|r| r.chat_name.to_lowercase() == search_term_lower) {
            tracing::info!("Found exact match for room");
            Some(room)
        }
        // Then try substring match
        else if let Some(room) = bridge_rooms.iter()
            .filter(|r| r.chat_name.to_lowercase().contains(&search_term_lower))
            .max_by_key(|r| r.last_activity) {
            tracing::info!("Found substring match for room");
            Some(room)
        }
        // Finally try similarity match
        else {
            let best_match = bridge_rooms.iter()
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

    let user_settings = state.user_core.get_user_settings(user_id)?;
    let user_info = state.user_core.get_user_info(user_id)?;
    match matching_room {
        Some(room) => fetch_messages_from_room(service, room.room.clone(), limit, user_info.timezone).await,
        None => Err(anyhow!("No matching {} room found for '{}'", capitalize(&service), chat_name))
    }
}

async fn fetch_messages_from_room(
    service: &str,
    room: matrix_sdk::room::Room,
    limit: Option<u64>,
    timezone: Option<String>,
) -> Result<(Vec<BridgeMessage>, String)> {
    let room_name = room.display_name().await?.to_string();
    let sender_prefix = get_sender_prefix(service);
    let mut options = MessagesOptions::backward();
    options.limit = matrix_sdk::ruma::UInt::new(limit.unwrap_or(20)).unwrap();

    let response = room.messages(options).await?;
    
    let mut futures = Vec::with_capacity(response.chunk.len());
    let room_name_clone = room_name.clone();
    
    for event in response.chunk {
        let timezone = timezone.clone();
        let room_name = room_name_clone.clone();
        let sender_prefix = sender_prefix.clone();
        futures.push(async move {
            if let Ok(AnySyncTimelineEvent::MessageLike(
                matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
            )) = event.raw().deserialize() {
                let (sender, timestamp, content) = match msg {
                    SyncRoomMessageEvent::Original(e) => (e.sender, i64::from(e.origin_server_ts.0) / 1000, e.content),
                    _ => return None,
                };

                if !sender.localpart().starts_with(&sender_prefix) {
                    return None;
                }

                let (msgtype, body) = match content.msgtype {
                    MessageType::Text(t) => ("text", t.body),
                    MessageType::Notice(n) => ("notice", n.body),
                    MessageType::Image(i) => ("image", if i.body.is_empty() { "📎 IMAGE".into() } else { i.body }),
                    MessageType::Video(v) => ("video", if v.body.is_empty() { "📎 VIDEO".into() } else { v.body }),
                    MessageType::File(f) => ("file", if f.body.is_empty() { "📎 FILE".into() } else { f.body }),
                    MessageType::Audio(a) => ("audio", if a.body.is_empty() { "📎 AUDIO".into() } else { a.body }),
                    MessageType::Location(l) => ("location", "📍 LOCATION".into()), // Location has no body field
                    MessageType::Emote(t) => ("emote", t.body),
                    _ => return None,
                };

                Some(BridgeMessage {
                    sender: sender.to_string(),
                    sender_display_name: sender.localpart().to_string(),
                    content: body,
                    timestamp,
                    formatted_timestamp: format_timestamp(timestamp, timezone),
                    message_type: msgtype.to_string(),
                    room_name: room_name.clone(),
                    media_url: None,
                })
            } else {
                None
            }
        });
    }

    // Collect results from parallel processing
    let mut messages: Vec<BridgeMessage> = join_all(futures).await
        .into_iter()
        .flatten()
        .collect();

    // Sort messages by timestamp (most recent first)
    messages.sort_unstable_by_key(|m| std::cmp::Reverse(m.timestamp));

    Ok((messages, room_name))
}

use std::time::{SystemTime, UNIX_EPOCH};

pub async fn handle_bridge_message(
    event: OriginalSyncRoomMessageEvent,
    room: Room,
    client: MatrixClient,
    state: Arc<AppState>,
) {
    tracing::debug!("Entering bridge message handler");
    // Check message age
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let message_ts = event.origin_server_ts.0;
    let age_ms = now.saturating_sub(message_ts.into()); // Use saturating_sub to handle any potential clock skew
    const HALF_HOUR_MS: u64 = 30 * 60 * 1000;
    if age_ms > HALF_HOUR_MS {
        tracing::debug!(
            "Skipping old message: age {} ms (event ID: {})",
            age_ms,
            event.event_id
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
    let user = match state.user_repository.get_user_by_matrix_user_id(local_user_id) {
        Ok(Some(user)) => user,
        _ => return,
    };
    let user_id = user.id;
    // New: Check if this is a bridge management room
    let room_id_str = room.room_id().to_string();
    let bridge_types = vec!["signal", "telegram", "whatsapp"];
    let mut bridges = Vec::new();
    for bridge_type in &bridge_types {
        if let Ok(Some(bridge)) = state.user_repository.get_bridge(user_id, bridge_type) {
            bridges.push(bridge);
        }
    }
    if let Some(bridge) = bridges.iter().find(|b| b.room_id.as_ref().map_or(false, |rid| rid == &room_id_str)) {
        // This is a management room for a bridge
        tracing::debug!("Processing message in {} bridge management room", bridge.bridge_type);
        // Skip if bridge is in connecting state (handled by monitor task)
        if bridge.status == "connecting" {
            tracing::debug!("Skipping disconnection check during initial connection for {}", bridge.bridge_type);
            return;
        }

        
        // Get the bridge bot ID for this service
        let bridge_bot_var = match bridge.bridge_type.as_str() {
            "signal" => "SIGNAL_BRIDGE_BOT",
            "whatsapp" => "WHATSAPP_BRIDGE_BOT",
            "telegram" => "TELEGRAM_BRIDGE_BOT",
            _ => return, // Unknown bridge type
        };
        let bridge_bot = match std::env::var(bridge_bot_var) {
            Ok(bot) => bot,
            Err(_) => {
                tracing::error!("{} not set", bridge_bot_var);
                return;
            }
        };
        let bot_user_id = match matrix_sdk::ruma::OwnedUserId::try_from(bridge_bot) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("Invalid bridge bot ID: {}", e);
                return;
            }
        };
        
        // Check if sender is the bridge bot
        if event.sender != bot_user_id {
            tracing::debug!("Message not from bridge bot, skipping");
            return;
        }
        
        // Extract message content
        let content = match event.content.msgtype {
            MessageType::Text(t) => t.body,
            MessageType::Notice(n) => n.body,
            _ => {
                tracing::debug!("Non-text/notice message in management room, skipping");
                return;
            }
        };

        println!("bridge bot management room content: {}", content);
        
        // Define disconnection patterns (customize per bridge if needed)
        let disconnection_patterns = vec![
            "disconnected",
            "connection lost",
            "logged out",
            "authentication failed",
            "login failed",
            "error",
            "failed",
            "timeout",
            "invalid",
        ];
        let lower_content = content.to_lowercase();
        if disconnection_patterns.iter().any(|p| lower_content.contains(p)) {
            tracing::info!("Detected disconnection in {} bridge for user {}: {}", bridge.bridge_type, user_id, content);
            
            // Delete the bridge record
            if let Err(e) = state.user_repository.delete_bridge(user_id, &bridge.bridge_type) {
                tracing::error!("Failed to delete {} bridge: {}", bridge.bridge_type, e);
            }
            
            // Check if there are any remaining active bridges
            let has_active_bridges = match state.user_repository.has_active_bridges(user_id) {
                Ok(has) => has,
                Err(e) => {
                    tracing::error!("Failed to check active bridges: {}", e);
                    false
                }
            };
            if !has_active_bridges {
                // No active bridges left, remove client and sync task
                let mut matrix_clients = state.matrix_clients.lock().await;
                let mut sync_tasks = state.matrix_sync_tasks.lock().await;
                if let Some(task) = sync_tasks.remove(&user_id) {
                    task.abort();
                    tracing::debug!("Aborted sync task for user {}", user_id);
                }
                if matrix_clients.remove(&user_id).is_some() {
                    tracing::debug!("Removed Matrix client for user {}", user_id);
                }
            }
        } else {
            tracing::debug!("No disconnection detected in management room message");
        }
        
        // Return early since this is not a portal message
        return;
    }
    // Proceed with existing portal message handling if not a management room
    // Get room name
    let room_name = match room.display_name().await {
        Ok(name) => name.to_string(),
        Err(e) => {
            tracing::error!("Failed to get room name: {}", e);
            return;
        }
    };
    let sender_localpart = event.sender.localpart().to_string();
    if user_id == 1 {
        println!("room_name: {}", room_name);
        println!("sender_localpart: {}", sender_localpart);
    }
    let service = match infer_service(&room_name, &sender_localpart) {
        Some(s) => s,
        None => {
            tracing::error!("Could not infer service, skipping");
            return;
        }
    };
    let room_suffix = get_room_suffix(&service);
    let sender_prefix = get_sender_prefix(&service);
    if !room_name.contains(&room_suffix) {
        tracing::debug!("Skipping non-{} room", service);
        return;
    }
    if !sender_localpart.starts_with(&sender_prefix) {
        tracing::debug!("Skipping non-{} sender", service);
        return;
    }
    // Check if user has valid subscription
    let has_valid_sub = state.user_repository.has_valid_subscription_tier(user_id, "tier 2").unwrap_or(false) ||
        state.user_repository.has_valid_subscription_tier(user_id, "self_hosted").unwrap_or(false);
    if !has_valid_sub {
        tracing::debug!("User {} does not have valid subscription for WhatsApp monitoring", user_id);
        return;
    }
    if !state.user_core.get_proactive_agent_on(user_id).unwrap_or(true) {
        tracing::debug!("User {} does not have monitoring enabled", user_id);
        return;
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
    if user_id == 1 { // if admin for debugging
        println!("message: {}", content);
    }
    // Skip error messages
    if content.contains("Failed to bridge media") ||
       content.contains("media no longer available") ||
       content.contains("Decrypting message from WhatsApp failed") ||
       content.starts_with("* Failed to") {
        tracing::debug!("Skipping error message because content contained error messages");
        return;
    }
    let chat_name = room_name
        .split(&room_suffix)
        .next()
        .unwrap_or(&room_name)
        .trim()
        .to_string();
   
    let sender_name = sender_localpart
        .strip_prefix(&sender_prefix)
        .unwrap_or(&sender_localpart)
        .to_string();
    let waiting_checks = state.user_repository.get_waiting_checks(user_id, "messaging").unwrap_or(Vec::new());
    let priority_senders = state.user_repository.get_priority_senders(user_id, &service).unwrap_or(Vec::new());
    fn trim_for_sms(service: &str, sender: &str, content: &str) -> String {
        let prefix = format!("{} from ", capitalize(&service));
        let separator = ": ";
        let max_len = 157;
        let static_len = prefix.len() + separator.len();
        let mut remaining = max_len - static_len;
        // Reserve up to 30 chars for sender
        let mut sender_trimmed = sender.chars().take(30).collect::<String>();
        if sender.len() > sender_trimmed.len() {
            sender_trimmed.push('…');
        }
        remaining = remaining.saturating_sub(sender_trimmed.len());
        let mut content_trimmed = content.chars().take(remaining).collect::<String>();
        if content.len() > content_trimmed.len() {
            content_trimmed.push('…');
        }
        format!("{}{}{}{}", prefix, sender_trimmed, separator, content_trimmed)
    }
    let service_cap = capitalize(&service);

    let running_environment = std::env::var("ENVIRONMENT").unwrap();
    if running_environment == "development".to_string() {
        return;
    }
        // FAST CHECKS SECOND - Check priority senders if active
    for priority_sender in &priority_senders {
        let clean_priority_sender = priority_sender.sender
            .split(&room_suffix)
            .next()
            .unwrap_or(&priority_sender.sender)
            .trim()
            .to_string();
        if chat_name.to_lowercase().contains(&clean_priority_sender.to_lowercase()) ||
           sender_name.to_lowercase().contains(&clean_priority_sender.to_lowercase()) {
          
            // Determine suffix based on noti_type
            let suffix = match priority_sender.noti_type.as_ref().map(|s| s.as_str()) {
                Some("call") => "_call",
                _ => "_sms",
            };
            let notification_type = format!("{}_priority{}", service, suffix);
          
            // Check if user has enough credits for notification
            match crate::utils::usage::check_user_credits(&state, &user, "noti_msg", None).await {
                Ok(()) => {
                    // User has enough credits, proceed with notification
                    let state_clone = state.clone();
                    let content_clone = content.clone();
                    let message = trim_for_sms(&service, &priority_sender.sender, &content_clone);
                    let first_message = format!("Hello, you have an important {} message from {}.", service_cap, priority_sender.sender);
                  
                    // Spawn a new task for sending notification
                    tokio::spawn(async move {
                        // Send the notification
                        crate::proactive::utils::send_notification(
                            &state_clone,
                            user_id,
                            &message,
                            notification_type,
                            Some(first_message),
                        ).await;
                      
                    });
                    return;
                }
                Err(e) => {
                    tracing::warn!("User {} does not have enough credits for priority sender notification: {}, continuing though", user_id, e);
                }
            }
        }
    }

    if !waiting_checks.is_empty() {
        // Check if any waiting checks match the message
        if let Ok((check_id_option, message, first_message)) = crate::proactive::utils::check_waiting_check_match(
            &state,
            &format!("{} from {}: {}", service_cap, chat_name, content),
            &waiting_checks,
        ).await {
            if let Some(check_id) = check_id_option {
                let message = message.unwrap_or(format!("Waiting check matched in {}, but failed to get content", service).to_string());
                let first_message = first_message.unwrap_or(format!("Hey, I found a match for one of your waiting checks in {}.", service_cap));
              
                // Find the matched waiting check to determine noti_type
                let matched_waiting_check = waiting_checks.iter().find(|wc| wc.id == Some(check_id)).cloned();
                let suffix = if let Some(wc) = matched_waiting_check {
                    match wc.noti_type.as_ref().map(|s| s.as_str()) {
                        Some("call") => "_call",
                        _ => "_sms",
                    }
                } else {
                    "_sms"
                };
                let notification_type = format!("{}_waiting_check{}", service, suffix);
              
                // Delete the matched waiting check
                if let Err(e) = state.user_repository.delete_waiting_check_by_id(user_id, check_id) {
                    tracing::error!("Failed to delete waiting check {}: {}", check_id, e);
                }
              
                // Send notification
                let state_clone = state.clone();
                tokio::spawn(async move {
                    crate::proactive::utils::send_notification(
                        &state_clone,
                        user_id,
                        &message,
                        notification_type,
                        Some(first_message),
                    ).await;
                });
                return;
            }
        }
    }
    // Check message importance based on waiting checks and criticality
    let user_settings = match state.user_core.get_user_settings(user_id) {
        Ok(settings) => settings,
        Err(e) => {
            tracing::error!("Failed to get user settings: {}", e);
            return;
        }
    };
    if user_settings.critical_enabled.is_none() {
        tracing::debug!("Critical message checking disabled for user {}", user_id);
        return;
    }
    if let Ok((is_critical, message, first_message)) = crate::proactive::utils::check_message_importance(&state, &format!("{} from {}: {}", service_cap, chat_name, content)).await {
        if is_critical {
            let message = message.unwrap_or(format!("Critical {} message found, failed to get content, but you can check your {} to see it.", service_cap, service));
            let first_message = first_message.unwrap_or(format!("Hey, I found some critical {} message.", service_cap));
           
            // Spawn a new task for sending critical message notification
            let state_clone = state.clone();
            let notification_type = format!("{}_critical", service);
            tokio::spawn(async move {
                crate::proactive::utils::send_notification(
                    &state_clone,
                    user_id,
                    &message,
                    notification_type,
                    Some(first_message),
                ).await;
            });
        }
    }
}


pub async fn search_bridge_rooms(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    search_term: &str,
) -> Result<Vec<BridgeRoom>> {
    let bridge_bot_username = get_bridge_bot_username(service);
    // Validate bridge connection first
    let bridge = state.user_repository.get_bridge(user_id, service)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("{} bridge is not connected. Please log in first.", capitalize(&service)));
    }
    let client = crate::utils::matrix_auth::get_cached_client(user_id, &state).await?;
    let joined_rooms = client.joined_rooms();
    let search_term_lower = search_term.trim().to_lowercase();
    let room_suffix = get_room_suffix(service);
    let sender_prefix = get_sender_prefix(service); // Define here for cloning into futures
    let service_cap = capitalize(service);
    // Add skip_terms to avoid control/admin rooms (e.g., "Telegram bridge bot")
    let skip_terms = vec![
        format!("{}bot", service),
        format!("{}-bridge", service),
        format!("{} Bridge", service_cap),
        format!("{} bridge bot", service_cap),
    ];
    // Process rooms in parallel
    let room_futures = joined_rooms.into_iter().map(|room| {
        let room_suffix = room_suffix.clone();
        let sender_prefix = sender_prefix.clone();
        let skip_terms = skip_terms.clone();
        async move {
            // Quick check for room name
            let room_name = match room.display_name().await {
                Ok(name) => name.to_string(),
                Err(_) => return None,
            };
            // NEW: Skip control/admin rooms based on name terms
            if skip_terms.iter().any(|t| room_name.contains(t)) {
                return None;
            }
            let members = match room.members(RoomMemberships::JOIN).await {
                Ok(m) => m,
                Err(_) => return None,
            };
            // Removed strict suffix check; instead, confirm bridge room via membership
            let has_service_member = members.iter().any(|member| member.user_id().localpart().starts_with(&sender_prefix));
            if !has_service_member {
                return None;
            }
            // Handle clean_name with or without suffix
            let clean_name = if room_name.contains(&room_suffix) {
                room_name
                    .split(&room_suffix)
                    .next()
                    .unwrap_or(&room_name)
                    .trim()
                    .to_string()
            } else {
                room_name.trim().to_string()
            };
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
            Some((clean_name, BridgeRoom {
                room_id: room.room_id().to_string(),
                display_name: room_name,
                last_activity,
                last_activity_formatted: format_timestamp(last_activity, None),
            }))
        }
    });
    // Collect results from parallel processing
    let all_rooms: Vec<(String, BridgeRoom)> = join_all(room_futures)
        .await
        .into_iter()
        .flatten()
        .collect();
    // Single-pass matching with prioritized results
    let mut matching_rooms: Vec<(f64, BridgeRoom)> = all_rooms
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
    tracing::info!("Found {} matching {} rooms", matching_rooms.len(), capitalize(&service));
   
    Ok(matching_rooms.into_iter().map(|(_, room)| room).collect())
}

pub async fn fetch_recent_bridge_contacts(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<Vec<BridgeRoom>> {
    // Validate bridge connection first
    let bridge = state.user_repository.get_bridge(user_id, service)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("{} bridge is not connected. Please log in first.", capitalize(service)));
    }

    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
    let joined_rooms = client.joined_rooms();

    let room_suffix = get_room_suffix(service);
    let sender_prefix = get_sender_prefix(service);
    let service_cap = capitalize(service);
    let skip_terms = vec![
        format!("{}bot", service),
        format!("{}-bridge", service),
        format!("{} Bridge", service_cap),
        format!("{} bridge bot", service_cap),
    ];

    // Process rooms in parallel
    let room_futures = joined_rooms.into_iter().map(|room| {
        let sender_prefix = sender_prefix.clone();
        let skip_terms = skip_terms.clone();
        async move {
            // Quick check for room name
            let display_name = match room.display_name().await {
                Ok(name) => name.to_string(),
                Err(_) => return None,
            };

            // Skip control/admin rooms based on name terms
            if skip_terms.iter().any(|t| display_name.contains(t)) {
                return None;
            }

            // Confirm bridge room via membership
            let members = match room.members(RoomMemberships::JOIN).await {
                Ok(m) => m,
                Err(_) => return None,
            };

            if members.len() > 3 {
                return None;  // Skip if not exactly a DM (more members indicate a group)
            }
            let has_service_member = members.iter().any(|member| member.user_id().localpart().starts_with(&sender_prefix));
            if !has_service_member {
                return None;
            }

            // Get last activity timestamp efficiently (most recent message, sent or received)
            let mut options = matrix_sdk::room::MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(1).unwrap();

            let last_activity = match room.messages(options).await {
                Ok(response) => response.chunk.first()
                    .and_then(|event| event.raw().deserialize().ok())
                    .map(|e: AnySyncTimelineEvent| i64::from(e.origin_server_ts().0) / 1000)
                    .unwrap_or(0),
                Err(_) => 0,
            };

            Some(BridgeRoom {
                room_id: room.room_id().to_string(),
                display_name,
                last_activity,
                last_activity_formatted: format_timestamp(last_activity, None),
            })
        }
    });

    // Collect results from parallel processing
    let mut recent_rooms: Vec<BridgeRoom> = join_all(room_futures)
        .await
        .into_iter()
        .flatten()
        .collect();

    // Sort by last activity (most recent first) and take top 10
    recent_rooms.sort_unstable_by_key(|r| std::cmp::Reverse(r.last_activity));
    recent_rooms.truncate(10);

    tracing::info!("Retrieved {} most recent {} contacts", recent_rooms.len(), capitalize(service));

    Ok(recent_rooms)
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
