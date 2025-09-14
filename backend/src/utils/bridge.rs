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

pub fn remove_bridge_suffix(chat_name: &str) -> String {
    if chat_name.ends_with("(WA)") {
        chat_name.trim_end_matches("(WA)").trim().to_string()
    } else if chat_name.ends_with("(Telegram)") {
        chat_name.trim_end_matches("(Telegram)").trim().to_string()
    } else {
        chat_name.to_string()
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

pub async fn get_service_rooms(client: &MatrixClient, service: &str) -> Result<Vec<BridgeRoom>> {
    let joined_rooms = client.joined_rooms();
    let sender_prefix = get_sender_prefix(service);
    let service_cap = capitalize(service);
    let skip_terms = vec![
        format!("{}bot", service),
        format!("{}-bridge", service),
        format!("{} Bridge", service_cap),
        format!("{} bridge bot", service_cap),
    ];
    let mut futures = Vec::new();
    for room in joined_rooms {
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
            // Check membership instead of last message sender
            let members = match room.members(RoomMemberships::JOIN).await {
                Ok(m) => m,
                Err(_) => return None,
            };
            let has_service_member = members.iter().any(|member| member.user_id().localpart().starts_with(&sender_prefix));
            if !has_service_member {
                return None;
            }
            // Get last activity from most recent message, regardless of sender
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
        });
    }
    let results = join_all(futures).await;
    let mut rooms: Vec<BridgeRoom> = results.into_iter().flatten().collect();
    rooms.sort_by_key(|r| std::cmp::Reverse(r.last_activity));
    Ok(rooms)
}

pub fn find_exact_room(
    bridge_rooms: &[BridgeRoom],
    search_term: &str,
) -> Option<BridgeRoom> {
    let search_term_lower = search_term.trim().to_lowercase();
    if let Some(room) = bridge_rooms.iter().find(|r| remove_bridge_suffix(r.display_name.as_str()).to_lowercase() == search_term_lower) {
        tracing::info!("Found exact match for room");
        return Some(room.clone());
    }
    None
}

pub fn search_best_match(
    bridge_rooms: &[BridgeRoom],
    search_term: &str,
) -> Option<BridgeRoom> {
    let search_term_lower = search_term.trim().to_lowercase();
    // Try exact match first (fastest)
    if let Some(room) = bridge_rooms.iter().find(|r| remove_bridge_suffix(r.display_name.as_str()).to_lowercase() == search_term_lower) {
        tracing::info!("Found exact match for room");
        return Some(room.clone());
    }
    // Then try substring match
    if let Some(room) = bridge_rooms.iter()
        .filter(|r| remove_bridge_suffix(r.display_name.as_str()).to_lowercase().contains(&search_term_lower))
        .max_by_key(|r| r.last_activity) {
        tracing::info!("Found substring match for room");
        return Some(room.clone());
    }
    // Finally try similarity match
    let best_match = bridge_rooms.iter()
        .map(|r| (strsim::jaro_winkler(&search_term_lower, &remove_bridge_suffix(r.display_name.as_str()).to_lowercase()), r))
        .filter(|(score, _)| *score >= 0.7)
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    if let Some((score, room)) = best_match {
        tracing::info!("Found similar match with score {}", score);
        Some(room.clone())
    } else {
        None
    }
}

pub fn get_best_matches(
    bridge_rooms: &[BridgeRoom],
    search_term: &str,
) -> Vec<String> {
    let search_term_lower = search_term.trim().to_lowercase();
    let mut matches: Vec<(f64, String)> = bridge_rooms.iter()
        .map(|r| {
            let name = remove_bridge_suffix(&r.display_name);
            let name_lower = name.to_lowercase();
            (strsim::jaro_winkler(&search_term_lower, &name_lower), name.to_string())
        })
        .filter(|(score, _)| *score >= 0.7)
        .collect();
    matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    matches.into_iter().take(5).map(|(_, name)| name).collect()

}

const AI_PROMPT_TEXT: &str = "Hi, I'm Lightfriend, your friend's AI assistant. This message looks time-sensitive—since they're not currently on their computer, would you like me to send them a notification about it? Reply \"yes\" or \"no.\"";

pub async fn get_triggering_message_in_room(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    room_id_str: &str,
) -> Result<Option<BridgeMessage>> {
    tracing::info!(
        "Fetching triggering message in {} - User: {}, room_id: {}",
        capitalize(service),
        user_id,
        room_id_str
    );

    // Validate bridge connection
    if let Some(bridge) = state.user_repository.get_bridge(user_id, service)? {
        if bridge.status != "connected" {
            return Err(anyhow!("{} bridge is not connected. Please log in first.", capitalize(service)));
        }
    } else {
        return Err(anyhow!("{} bridge not found", capitalize(service)));
    }

    // Get Matrix client
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;

    // Get user info for timezone
    let user_info = state.user_core.get_user_info(user_id)?;

    // Get the room
    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_str)?;
    let room = client.get_room(&room_id).ok_or(anyhow!("Room not found"))?;

    // Fetch room display name
    let room_display_name = room.display_name().await?.to_string();
    let cleaned_room_name = remove_bridge_suffix(&room_display_name);

    // Fetch messages backward (latest first)
    let mut options = MessagesOptions::backward();
    options.limit = matrix_sdk::ruma::UInt::new(100).unwrap(); // Limit to avoid fetching too many; increase if needed

    let response = room.messages(options).await?;

    // Sender prefix for bridge bots (incoming messages start with this)
    let sender_prefix = get_sender_prefix(service);

    // User's Matrix user ID (for sent messages)
    let user_matrix_id = client.user_id().ok_or(anyhow!("User ID not available"))?;

    // Iterate through messages from latest to oldest
    let mut found_prompt = false;
    for event in response.chunk {
        if let Ok(AnySyncTimelineEvent::MessageLike(
            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
        )) = event.raw().deserialize() {
            if let SyncRoomMessageEvent::Original(e) = msg {
                let sender_localpart = e.sender.localpart().to_string();

                if !found_prompt {
                    // Look for the AI prompt sent by the user
                    if e.sender == user_matrix_id && !sender_localpart.starts_with(&sender_prefix) {
                        let body = match e.content.msgtype {
                            MessageType::Text(ref t) => t.body.clone(),
                            _ => continue,
                        };
                        if body.contains(AI_PROMPT_TEXT) {
                            found_prompt = true;
                            continue; // Skip to the next message (older)
                        }
                    }
                } else {
                    // After finding the prompt, look for the next incoming message
                    if sender_localpart.starts_with(&sender_prefix) {
                        let timestamp = i64::from(e.origin_server_ts.0) / 1000;

                        // Extract message type and body
                        let (msgtype, body) = match e.content.msgtype {
                            MessageType::Text(t) => ("text", t.body),
                            MessageType::Notice(n) => ("notice", n.body),
                            MessageType::Image(i) => ("image", if i.body.is_empty() { "📎 IMAGE".into() } else { i.body }),
                            MessageType::Video(v) => ("video", if v.body.is_empty() { "📎 VIDEO".into() } else { v.body }),
                            MessageType::File(f) => ("file", if f.body.is_empty() { "📎 FILE".into() } else { f.body }),
                            MessageType::Audio(a) => ("audio", if a.body.is_empty() { "📎 AUDIO".into() } else { a.body }),
                            MessageType::Location(l) => ("location", "📍 LOCATION".into()),
                            MessageType::Emote(t) => ("emote", t.body),
                            _ => continue,
                        };

                        // Skip error-like messages
                        if body.contains("Failed to bridge media") ||
                           body.contains("media no longer available") ||
                           body.contains("Decrypting message from WhatsApp failed") ||
                           body.starts_with("* Failed to") {
                            continue;
                        }

                        return Ok(Some(BridgeMessage {
                            sender: e.sender.to_string(),
                            sender_display_name: sender_localpart,
                            content: body,
                            timestamp,
                            formatted_timestamp: format_timestamp(timestamp, user_info.timezone.clone()),
                            message_type: msgtype.to_string(),
                            room_name: cleaned_room_name,
                            media_url: None,
                        }));
                    }
                }
            }
        }
    }

    // If no triggering message found after the prompt
    tracing::info!("No triggering incoming message found before the AI prompt in room '{}'", room_id_str);
    Ok(None)
}


pub async fn get_latest_sent_message_in_room(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    room_id_str: &str,
) -> Result<Option<BridgeMessage>> {
    tracing::info!(
        "Fetching latest sent message in {} - User: {}, room_id: {}",
        capitalize(service),
        user_id,
        room_id_str
    );

    // Validate bridge connection
    if let Some(bridge) = state.user_repository.get_bridge(user_id, service)? {
        if bridge.status != "connected" {
            return Err(anyhow!("{} bridge is not connected. Please log in first.", capitalize(service)));
        }
    } else {
        return Err(anyhow!("{} bridge not found", capitalize(service)));
    }

    // Get Matrix client
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;

    // Get user info for timezone
    let user_info = state.user_core.get_user_info(user_id)?;

    // Get the room
    let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id_str)?;
    let room = client.get_room(&room_id).ok_or(anyhow!("Room not found"))?;

    // Fetch room display name
    let room_display_name = room.display_name().await?.to_string();
    let cleaned_room_name = remove_bridge_suffix(&room_display_name);

    // Fetch messages backward (latest first)
    let mut options = MessagesOptions::backward();
    options.limit = matrix_sdk::ruma::UInt::new(100).unwrap(); // Limit to avoid fetching too many; increase if needed

    let response = room.messages(options).await?;

    // Sender prefix for bridge bots (to exclude incoming messages)
    let sender_prefix = get_sender_prefix(service);

    // User's Matrix user ID
    let user_matrix_id = client.user_id().ok_or(anyhow!("User ID not available"))?;

    for event in response.chunk {
        if let Ok(AnySyncTimelineEvent::MessageLike(
            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
        )) = event.raw().deserialize() {
            if let SyncRoomMessageEvent::Original(e) = msg {
                let sender_localpart = e.sender.localpart().to_string();
                // Check if sender is the user (matches user_matrix_id and not a bridge bot prefix)
                if e.sender == user_matrix_id && !sender_localpart.starts_with(&sender_prefix) {
                    let timestamp = i64::from(e.origin_server_ts.0) / 1000;

                    // Extract message type and body
                    let (msgtype, body) = match e.content.msgtype {
                        MessageType::Text(t) => ("text", t.body),
                        MessageType::Notice(n) => ("notice", n.body),
                        MessageType::Image(i) => ("image", if i.body.is_empty() { "📎 IMAGE".into() } else { i.body }),
                        MessageType::Video(v) => ("video", if v.body.is_empty() { "📎 VIDEO".into() } else { v.body }),
                        MessageType::File(f) => ("file", if f.body.is_empty() { "📎 FILE".into() } else { f.body }),
                        MessageType::Audio(a) => ("audio", if a.body.is_empty() { "📎 AUDIO".into() } else { a.body }),
                        MessageType::Location(l) => ("location", "📍 LOCATION".into()),
                        MessageType::Emote(t) => ("emote", t.body),
                        _ => continue,
                    };

                    // Skip error-like messages if needed (adapted from existing logic)
                    if body.contains("Failed to bridge media") ||
                       body.contains("media no longer available") ||
                       body.contains("Decrypting message from WhatsApp failed") ||
                       body.starts_with("* Failed to") {
                        continue;
                    }

                    return Ok(Some(BridgeMessage {
                        sender: "You".to_string(),
                        sender_display_name: "You".to_string(),
                        content: body,
                        timestamp,
                        formatted_timestamp: format_timestamp(timestamp, user_info.timezone.clone()),
                        message_type: msgtype.to_string(),
                        room_name: cleaned_room_name,
                        media_url: None,
                    }));
                }
            }
        }
    }

    // If no user-sent message found within the limit
    tracing::info!("No sent message found in the last 100 messages for room '{}'", room_id_str);
    Ok(None)
}

pub async fn fetch_bridge_room_messages(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    chat_name: &str,
    limit: Option<u64>,
) -> Result<(Vec<BridgeMessage>, String)> {
    tracing::info!(
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
    let rooms = get_service_rooms(&client, service).await?;
    let matching_room = search_best_match(&rooms, chat_name);
    let user_info = state.user_core.get_user_info(user_id)?;
    match matching_room {
        Some(room_info) => {
            let room_id = match matrix_sdk::ruma::OwnedRoomId::try_from(room_info.room_id.as_str()) {
                Ok(id) => id,
                Err(e) => return Err(anyhow!("Invalid room ID: {}", e)),
            };
            let room = match client.get_room(&room_id) {
                Some(r) => r,
                None => return Err(anyhow!("Room not found")),
            };
            fetch_messages_from_room(service, room, limit, user_info.timezone).await
        }
        None => Err(anyhow!("No matching {} room found for '{}'", capitalize(&service), chat_name))
    }
}

use matrix_sdk::notification_settings::RoomNotificationMode;

pub async fn fetch_bridge_messages(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    start_time: i64,
    unread_only: bool,
) -> Result<Vec<BridgeMessage>> {
    tracing::info!("Fetching {} messages for user {}", service, user_id);
  
    let user_info= state.user_core.get_user_info(user_id)?;
    // Get Matrix client and check bridge status (use cached version for better performance)
    let client = crate::utils::matrix_auth::get_cached_client(user_id, &state).await?;
    let bridge = state.user_repository.get_bridge(user_id, service)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("{} bridge is not connected. Please log in first.", capitalize(&service)));
    }
    let service_rooms = get_service_rooms(&client, service).await?;
    let mut room_infos: Vec<(Room, BridgeRoom)> = Vec::new();
    for bridge_room in service_rooms {
        let room_id = match matrix_sdk::ruma::OwnedRoomId::try_from(bridge_room.room_id.as_str()) {
            Ok(id) => id,
            Err(_) => continue,
        };
        let Some(room) = client.get_room(&room_id) else { continue; };
        if room.user_defined_notification_mode().await == Some(RoomNotificationMode::Mute) {
            continue;
        }
        if unread_only && room.unread_notification_counts().notification_count == 0 {
            continue;
        }
        room_infos.push((room, bridge_room));
    }
    // Already sorted by last_activity desc from get_service_rooms
    room_infos.truncate(5);
    // Fetch messages in parallel
    let user_timezone = user_info.timezone.clone();
    let sender_prefix = get_sender_prefix(service);
    let mut futures = Vec::new();
    for (room, bridge_room) in room_infos {
        let sender_prefix = sender_prefix.clone();
        let user_timezone = user_timezone.clone();
        let room_name = remove_bridge_suffix(&bridge_room.display_name);
        futures.push(async move {
            let mut options = matrix_sdk::room::MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(50).unwrap(); // Fetch enough to cover filters
            let mut messages: Vec<BridgeMessage> = Vec::new();
            match room.messages(options).await {
                Ok(response) => {
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
                                if !sender.localpart().starts_with(&sender_prefix) {
                                    continue;
                                }
                                let (msgtype, body) = match content.msgtype {
                                    MessageType::Text(t) => ("text", t.body),
                                    MessageType::Notice(n) => ("notice", n.body),
                                    MessageType::Image(i) => ("image", if i.body.is_empty() { "📎 IMAGE".into() } else { i.body }),
                                    MessageType::Video(v) => ("video", if v.body.is_empty() { "📎 VIDEO".into() } else { v.body }),
                                    MessageType::File(f) => ("file", if f.body.is_empty() { "📎 FILE".into() } else { f.body }),
                                    MessageType::Audio(a) => ("audio", if a.body.is_empty() { "📎 AUDIO".into() } else { a.body }),
                                    MessageType::Location(l) => ("location", "📍 LOCATION".into()),
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
                                messages.push(BridgeMessage {
                                    sender: sender.to_string(),
                                    sender_display_name: sender.localpart().to_string(),
                                    content: body,
                                    timestamp,
                                    formatted_timestamp: format_timestamp(timestamp, user_timezone.clone()),
                                    message_type: msgtype.to_string(),
                                    room_name: room_name.clone(),
                                    media_url: None,
                                });
                                if messages.len() == 5 {
                                    break;
                                }
                            }
                        }
                    }
                }
                Err(e) => tracing::error!("Failed to fetch messages: {}", e),
            }
            messages
        });
    }
    // Collect results
    let results = join_all(futures).await;
    let mut messages: Vec<BridgeMessage> = results.into_iter().flatten().collect();
    // Sort by timestamp (most recent first)
    messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    tracing::info!("Retrieved {} latest messages from most active rooms", messages.len());
    Ok(messages)
}

use futures::future::join_all;
use matrix_sdk::room::MessagesOptions;

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
    let service_rooms = get_service_rooms(&client, service).await?;
    let exact_room = find_exact_room(&service_rooms, chat_name);
    let room = match exact_room {
        Some(room_info) => {
            let room_id = match matrix_sdk::ruma::OwnedRoomId::try_from(room_info.room_id.as_str()) {
                Ok(id) => id,
                Err(e) => return Err(anyhow!("Invalid room ID: {}", e)),
            };
            match client.get_room(&room_id) {
                Some(r) => r,
                None => return Err(anyhow!("Room not found")),
            }
        }
        None => {
            let suggestions = get_best_matches(&service_rooms, chat_name);
            let error_msg = if suggestions.is_empty() {
                format!("Could not find exact matching {} room for '{}'", capitalize(&service), chat_name)
            } else {
                format!(
                    "Could not find exact matching {} room for '{}'. Did you mean one of these?\n{}",
                    capitalize(&service),
                    chat_name,
                    suggestions.join("\n")
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
            message.to_owned(), // ← this is the caption / body
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
    if room.user_defined_notification_mode().await == Some(RoomNotificationMode::Mute) {
        tracing::info!("Skipping message from a muted room");
        return;
    }
    // Check message age
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let message_ts = event.origin_server_ts.0;
    let age_ms = now.saturating_sub(message_ts.into()); // Use saturating_sub to handle any potential clock skew
    const HALF_HOUR_MS: u64 = 30 * 60 * 1000;
    if age_ms > HALF_HOUR_MS {
        tracing::info!(
            "Skipping old message: age {} ms (event ID: {})",
            age_ms,
            event.event_id
        );
        return;
    }

    // Find the user ID for this Matrix client
    let matrix_user_id = client.user_id().unwrap().to_owned(); // Clone to OwnedUserId
    let client_user_id = matrix_user_id.to_string();
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
        if user_id == 1 {
            println!("bridge bot management room content: {}", content);
        }
      
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
    let service = match infer_service(&room_name, &sender_localpart) {
        Some(s) => s,
        None => {
            tracing::error!("Could not infer service, skipping");
            return;
        }
    };
    use matrix_sdk::ruma::{events::receipt::{ReceiptType, ReceiptThread}, api::client::room::get_room_event};
    use tokio::time::{sleep, Duration};
    let bridge = match state.user_repository.get_bridge(user_id, service.as_str()) {
        Ok(Some(b)) => b,
        Ok(None) => {
            tracing::error!("No bridge found for service {}", service);
            return;
        }
        Err(e) => {
            tracing::error!("Error getting bridge for service {}: {}", service, e);
            return;
        }
    };

    tracing::info!("Computing wait time based on last seen");

    // Get current time in seconds (Unix timestamp)
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    const SHORT_WAIT: u64 = 30;
    const LONG_WAIT: u64 = 300;
    const ACTIVITY_THRESHOLD: i32 = 300;  // 5 minutes

    println!("last_seen_online: {:#?}", bridge.last_seen_online);
    let wait_time = match bridge.last_seen_online {
        Some(last_seen) => {
            let age = now_secs - last_seen;
            if age > ACTIVITY_THRESHOLD {
                SHORT_WAIT
            } else {
                LONG_WAIT
            }
        }
        None => SHORT_WAIT,
    };
    tracing::info!("Waiting for {} seconds before processing (user activity inferred as {})", 
        wait_time, 
        if wait_time == SHORT_WAIT { "inactive" } else { "active" }
    );
    
    sleep(Duration::from_secs(wait_time)).await;

    // Check if user has read this or a later message (via bridged receipt)
    let own_user_id = client.user_id().unwrap();
    if let Ok(Some((receipt_event_id, _))) = room.load_user_receipt(ReceiptType::Read, ReceiptThread::Unthreaded, own_user_id).await {
        // Fetch the receipted event to get its timestamp (approximate order)
        let request = get_room_event::v3::Request::new(room.room_id().to_owned(), receipt_event_id.clone());
        if let Ok(response) = client.send(request).await {
            if let Ok(any_event) = response.event.deserialize_as::<AnySyncTimelineEvent>() {
                if any_event.origin_server_ts().0 >= event.origin_server_ts.0 {
                    tracing::info!("Skipping processing because user has read this or a later message");
                    let last_seen_online = i32::try_from(any_event.origin_server_ts().as_secs()).unwrap();
                    let rows = state.user_repository.update_bridge_last_seen_online(
                        user_id,
                        service.as_str(),
                        last_seen_online,
                    ).unwrap();
                    tracing::info!("Updated {:#?} rows for last_seen_online (user_id: {}, service: {}, value: {})", rows, user_id, service, last_seen_online);
                    if rows == 0 {
                        tracing::warn!("No bridge row matched for update - possible race or mismatch");
                    }
                    tracing::info!("set the last_seen_online to: {}", last_seen_online);
                    return;
                }
            }
        }
    }

    tracing::info!("No recent read detected; proceeding with message processing");
    let sender_prefix = get_sender_prefix(&service);
    if user_id == 1 {
        println!("sender_prefix: {}", sender_prefix);
    }
    if !sender_localpart.starts_with(&sender_prefix) {
        tracing::info!("Skipping non-{} sender", service);
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
    // Check if this is a group room (more than 3 members)
    let members = match room.members(RoomMemberships::JOIN).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Failed to fetch room members: {}", e);
            return;
        }
    };
    let member_count = members.len() as u64;
    if user_id == 1 {
        println!("members: {}", member_count);
    }
    if member_count > 3 {
        let is_mentioned = event.content.mentions.as_ref()
            .map(|m| m.user_ids.contains(&matrix_user_id))
            .unwrap_or(false);
        if !is_mentioned {
            tracing::info!("Skipping message from group room ({} members) since user wasn't mentioned", member_count);
            return;
        }
        tracing::info!("User {} is mentioned in message (event ID: {})", user_id, event.event_id);
    }
    // Skip error messages
    if content.contains("Failed to bridge media") ||
       content.contains("media no longer available") ||
       content.contains("Decrypting message from WhatsApp failed") ||
       content.starts_with("* Failed to") {
        tracing::debug!("Skipping error message because content contained error messages");
        return;
    }
    // New logic for handling "yes" responses in non-group chats
    if member_count <= 3 {
        let lowered_content = content.trim().to_lowercase();
        if lowered_content == "yes" || lowered_content == "y" {
            // Fetch the latest sent message
            let room_id_str = room.room_id().as_str();
            match get_latest_sent_message_in_room(&service, &state, user_id, room_id_str).await {
                Ok(Some(prev_msg)) => {
                    if prev_msg.content.contains("Hi, I'm Lightfriend, your friend's AI assistant. This message looks time-sensitive—since they're not currently on their computer, would you like me to send them a notification about it? Reply \"yes\" or \"no.\"") {
                        // Fetch the triggering message
                        match get_triggering_message_in_room(&service, &state, user_id, room_id_str).await {
                            Ok(Some(triggering_msg)) => {
                                let service_cap = capitalize(&service);
                                let chat_name = remove_bridge_suffix(&room_name);
                                let message = format!("{} from {}: {}", service_cap, chat_name, triggering_msg.content);
                                let first_message = format!("Hey, someone confirmed a time-sensitive {} message.", service_cap);
                                
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
                                return;
                            }
                            Ok(None) => {
                                tracing::info!("No triggering message found for 'yes' response");
                                return;
                            }
                            Err(e) => {
                                tracing::error!("Failed to fetch triggering message: {}", e);
                                return;
                            }
                        }
                    } else {
                        // Ignore the message if previous doesn't match
                        tracing::debug!("Ignoring 'yes' message as previous sent message does not match the expected prompt");
                        return;
                    }
                }
                Ok(None) => {
                    // Ignore if no previous sent message found
                    tracing::debug!("Ignoring 'yes' message as no previous sent message found");
                    return;
                }
                Err(e) => {
                    tracing::error!("Failed to fetch latest sent message: {}", e);
                    // Proceed to normal handling on error
                }
            }
        }
    }
    let chat_name = remove_bridge_suffix(room_name.as_str());
 
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
    // FAST CHECKS SECOND - Check priority senders if active
    for priority_sender in &priority_senders {
        if priority_sender.noti_mode == "all" {
            let clean_priority_sender = remove_bridge_suffix(priority_sender.sender.as_str());
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
    if user_id == 1 {
        println!("service: {}", service);
        println!("chat_name: {}", chat_name);
        println!("content: {}", content);
        println!("content contains call: {}", content.contains("Incoming call"));

            let action = user_settings.action_on_critical_message.as_ref().map(|s| s.as_str()).unwrap_or("notify");
            println!("action: {}", action);
    }
    if let Ok((is_critical, message_opt, first_message_opt)) = crate::proactive::utils::check_message_importance(&state, user_id, &format!("{} from {}: {}", service_cap, chat_name, content), service_cap.as_str(), chat_name.as_str(), content.as_str()).await {
        println!("is critical: {}", is_critical);
        if is_critical {
            let is_family = priority_senders.iter().filter(|ps| ps.noti_mode == "focus").any(|ps| {
                let clean_priority_sender = remove_bridge_suffix(&ps.sender);
                chat_name.to_lowercase().contains(&clean_priority_sender.to_lowercase()) ||
                sender_name.to_lowercase().contains(&clean_priority_sender.to_lowercase())
            });
            println!("is_family: {}", is_family);
            let action = user_settings.action_on_critical_message.as_ref().map(|s| s.as_str()).unwrap_or("notify");
            let prompt = "Hi, I'm Lightfriend, your friend's AI assistant. This message looks time-sensitive—since they're not currently on their computer, would you like me to send them a notification about it? Reply \"yes\" or \"no.\"".to_string();
            let mut should_notify = false;
            let mut should_ask = false;
            println!("action: {}", action);
            match action {
                "ask_sender" => {
                    should_ask = true;
                }
                "ask_sender_exclude_family" => {
                    if !is_family {
                        should_ask = true;
                    } else {
                        should_notify = true;
                    }
                }
                "notify_family" => {
                    if is_family {
                        should_notify = true;
                    }
                }
                _ => { // includes "notify_call" and default
                    should_notify = true;
                }
            }
            if should_ask {
                if member_count <= 3 {
                    let state_clone = state.clone();
                    let service_clone = service.clone();
                    let chat_name_clone = chat_name.clone();
                    tokio::spawn(async move {
                        if let Err(e) = send_bridge_message(&service_clone, &state_clone, user_id, &chat_name_clone, &prompt, None).await {
                            tracing::error!("Failed to send AI prompt: {}", e);
                        }
                    });
                    return;
                } else {
                    should_notify = true;
                }
            }
            if should_notify {
                let message = message_opt.unwrap_or(format!("Critical {} message found, failed to get content, but you can check your {} to see it.", service_cap, service));
                let first_message = first_message_opt.unwrap_or(format!("Hey, I found some critical {} message.", service_cap));
             
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
}


pub async fn search_bridge_rooms(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    search_term: &str,
) -> Result<Vec<BridgeRoom>> {
    // Validate bridge connection first
    let bridge = state.user_repository.get_bridge(user_id, service)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("{} bridge is not connected. Please log in first.", capitalize(&service)));
    }
    let client = crate::utils::matrix_auth::get_cached_client(user_id, &state).await?;
    let all_rooms = get_service_rooms(&client, service).await?;
    let search_term_lower = search_term.trim().to_lowercase();
    // Single-pass matching with prioritized results
    let mut matching_rooms: Vec<(f64, BridgeRoom)> = all_rooms
        .into_iter()
        .filter_map(|room| {
            let name = remove_bridge_suffix(&room.display_name);
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
) -> Result<Vec<String>> {
    let bridge = state.user_repository.get_bridge(user_id, service)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!("{} bridge is not connected. Please log in first.", capitalize(service)));
    }
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
    let rooms = get_service_rooms(&client, service).await?;
    let mut futures = Vec::new();
    for bridge_room in rooms {
        let client = client.clone();
        futures.push(async move {
            let room_id = match matrix_sdk::ruma::OwnedRoomId::try_from(bridge_room.room_id.as_str()) {
                Ok(id) => id,
                Err(_) => return None,
            };
            let room = match client.get_room(&room_id) {
                Some(r) => r,
                None => return None,
            };
            let members = match room.members(RoomMemberships::JOIN).await {
                Ok(m) => m,
                Err(_) => return None,
            };
            if members.len() > 3 {
                return None;
            }
            Some(remove_bridge_suffix(&bridge_room.display_name))
        });
    }
    let results = join_all(futures).await;
    let mut seen = std::collections::HashSet::new();
    let mut room_names: Vec<String> = Vec::new();
    for name in results.into_iter().flatten() {
        if seen.insert(name.clone()) {
            room_names.push(name);
        }
    }
    room_names.truncate(10);
    tracing::info!("Retrieved {} most recent {} contacts", room_names.len(), capitalize(service));
    Ok(room_names)
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
