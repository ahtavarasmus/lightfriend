use crate::api::matrix_client::MatrixClientInterface;
// Re-export trait types and pure functions for external use
pub use crate::api::matrix_client::{
    infer_service_from_room, is_disconnection_message, is_error_message, is_health_check_message,
    should_process_message, IncomingBridgeEvent, IncomingMessageContent, MatrixClientWrapper,
    RoomInterface, RoomWrapper,
};
use crate::UserCoreOps;
use anyhow::{anyhow, Result};
use matrix_sdk::{
    room::Room,
    ruma::{
        events::room::message::{MessageType, SyncRoomMessageEvent},
        events::AnySyncTimelineEvent,
    },
    Client as MatrixClient,
};
use std::sync::Arc;

use crate::AppState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BridgeRoom {
    pub room_id: String,
    pub display_name: String,
    pub last_activity: i64,
    pub last_activity_formatted: String,
    #[serde(default)]
    pub is_group: bool,
}

use chrono::DateTime;
use chrono_tz::Tz;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BridgeMessage {
    pub sender: String,
    pub sender_display_name: String,
    pub content: String,
    pub timestamp: i64,
    pub formatted_timestamp: String,
    pub message_type: String,
    pub room_name: String,
    pub media_url: Option<String>,
    pub room_id: Option<String>,
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
            Ok(tz) => dt_utc
                .with_timezone(&tz)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
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

fn get_sender_prefix(service: &str) -> String {
    format!("{}_", service)
}

pub fn remove_bridge_suffix(chat_name: &str) -> String {
    for suffix in &["(WA~)", "(WA)", "(Signal~)", "(Signal)", "(Telegram)"] {
        if chat_name.ends_with(suffix) {
            return chat_name.trim_end_matches(suffix).trim().to_string();
        }
    }
    chat_name.to_string()
}

/// Returns whether the room name indicates a phone contact (from phone book).
/// (WA) / (Signal) = phone contact (has FullName/ContactName)
/// (WA~) / (Signal~) = not a phone contact (only PushName/ProfileName)
/// Telegram / unknown = None (can't determine)
pub fn is_phone_contact_from_room_name(room_name: &str) -> Option<bool> {
    let trimmed = room_name.trim();
    if trimmed.ends_with("(WA)") || trimmed.ends_with("(Signal)") {
        Some(true)
    } else if trimmed.ends_with("(WA~)") || trimmed.ends_with("(Signal~)") {
        Some(false)
    } else {
        None // Telegram, email, or unknown
    }
}

fn infer_service(room_name: &str, sender_localpart: &str) -> Option<String> {
    let sender_localpart = sender_localpart.trim().to_lowercase();
    let room_name = room_name.to_lowercase();

    if room_name.contains("(wa)")
        || room_name.contains("(wa~)")
        || sender_localpart.starts_with("whatsapp_")
        || sender_localpart.starts_with("whatsapp")
    {
        tracing::debug!("Detected WhatsApp");
        return Some("whatsapp".to_string());
    }
    if room_name.contains("(tg)")
        || sender_localpart.starts_with("telegram_")
        || sender_localpart.starts_with("telegram")
    {
        tracing::debug!("Detected Telegram");
        return Some("telegram".to_string());
    }
    if room_name.contains("(signal)")
        || room_name.contains("(signal~)")
        || sender_localpart.starts_with("signal_")
        || sender_localpart.starts_with("signal")
    {
        tracing::debug!("Detected Signal");
        return Some("signal".to_string());
    }
    tracing::debug!("No service detected");
    None
}

pub async fn get_service_rooms(client: &MatrixClient, service: &str) -> Result<Vec<BridgeRoom>> {
    let joined_rooms = client.joined_rooms();
    let sender_prefix = get_sender_prefix(service);
    let service_cap = capitalize(service);
    let skip_terms = [
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
            let has_service_member = members
                .iter()
                .any(|member| member.user_id().localpart().starts_with(&sender_prefix));
            if !has_service_member {
                return None;
            }
            let is_group = members.len() > 3;
            // Get last activity from most recent message, regardless of sender
            let mut options = matrix_sdk::room::MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(1).unwrap();
            let last_activity = match room.messages(options).await {
                Ok(response) => response
                    .chunk
                    .first()
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
                is_group,
            })
        });
    }
    let results = join_all(futures).await;
    let mut rooms: Vec<BridgeRoom> = results.into_iter().flatten().collect();
    rooms.sort_by_key(|r| std::cmp::Reverse(r.last_activity));
    Ok(rooms)
}

// ============================================================================
// Trait-Based Functions (for testability)
// ============================================================================

/// Get service rooms using the trait interface (testable version)
pub async fn get_service_rooms_trait(
    client: &dyn MatrixClientInterface,
    service: &str,
) -> Result<Vec<BridgeRoom>> {
    let joined_rooms = client.get_joined_rooms().await?;
    let sender_prefix = get_sender_prefix(service);
    let service_cap = capitalize(service);
    let skip_terms = [
        format!("{}bot", service),
        format!("{}-bridge", service),
        format!("{} Bridge", service_cap),
        format!("{} bridge bot", service_cap),
    ];

    let mut rooms = Vec::new();
    for room_info in joined_rooms {
        // Skip management rooms
        if skip_terms
            .iter()
            .any(|t| room_info.display_name.contains(t))
        {
            continue;
        }

        // Get the room for more details
        if let Some(room) = client.get_room(&room_info.room_id).await {
            // Check membership for service users
            let members = match room.get_members().await {
                Ok(m) => m,
                Err(_) => continue,
            };

            let has_service_member = members
                .iter()
                .any(|member| member.localpart.starts_with(&sender_prefix));

            if !has_service_member {
                continue;
            }

            let is_group = members.len() > 3;
            let last_activity = room.get_last_activity().await;
            rooms.push(BridgeRoom {
                room_id: room_info.room_id,
                display_name: room_info.display_name,
                last_activity,
                last_activity_formatted: format_timestamp(last_activity, None),
                is_group,
            });
        }
    }

    rooms.sort_by_key(|r| std::cmp::Reverse(r.last_activity));
    Ok(rooms)
}

/// Send a bridge message using the trait interface (testable version)
pub async fn send_bridge_message_trait(
    client: &dyn MatrixClientInterface,
    service: &str,
    chat_name: &str,
    message: &str,
    media_url: Option<String>,
    timezone: Option<String>,
) -> Result<BridgeMessage> {
    let service_rooms = get_service_rooms_trait(client, service).await?;
    let exact_room = find_exact_room(&service_rooms, chat_name);

    let room = match exact_room {
        Some(room_info) => client
            .get_room(&room_info.room_id)
            .await
            .ok_or_else(|| anyhow!("Room not found"))?,
        None => {
            let suggestions = get_best_matches(&service_rooms, chat_name);
            let error_msg = if suggestions.is_empty() {
                format!(
                    "Could not find exact matching {} room for '{}'",
                    capitalize(service),
                    chat_name
                )
            } else {
                format!(
                    "Could not find exact matching {} room for '{}'. Did you mean one of these?\n{}",
                    capitalize(service),
                    chat_name,
                    suggestions.join("\n")
                )
            };
            return Err(anyhow!(error_msg));
        }
    };

    let is_image = media_url.is_some();
    if let Some(url) = media_url {
        // Download and upload image
        let resp = reqwest::get(&url).await?;
        let mime_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = resp.bytes().await?;
        let size = bytes.len() as u64;

        let mxc_uri = client.upload_media(&mime_type, bytes.to_vec()).await?;
        room.send_image(&mxc_uri, message, size).await?;
    } else {
        room.send_text(message).await?;
    }

    let room_name = room.display_name().await?;
    let room_id_str = room.room_id().to_string();
    let current_timestamp = chrono::Utc::now().timestamp();
    Ok(BridgeMessage {
        sender: "You".to_string(),
        sender_display_name: "You".to_string(),
        content: message.to_string(),
        timestamp: current_timestamp,
        formatted_timestamp: format_timestamp(current_timestamp, timezone),
        message_type: if is_image { "image" } else { "text" }.to_string(),
        room_name,
        media_url: None,
        room_id: Some(room_id_str),
    })
}

/// Fetch bridge messages using the trait interface (testable version)
pub async fn fetch_bridge_messages_trait(
    client: &dyn MatrixClientInterface,
    service: &str,
    start_time: i64,
    timezone: Option<String>,
) -> Result<Vec<BridgeMessage>> {
    let service_rooms = get_service_rooms_trait(client, service).await?;
    let sender_prefix = get_sender_prefix(service);

    let mut all_messages = Vec::new();

    // Process top 5 most active rooms
    for room_info in service_rooms.into_iter().take(5) {
        if let Some(room) = client.get_room(&room_info.room_id).await {
            // Skip muted rooms
            if room.is_muted().await {
                continue;
            }

            let room_name = remove_bridge_suffix(&room_info.display_name);

            // Fetch messages with the service prefix filter
            if let Ok(mut messages) = room.fetch_messages(50, Some(&sender_prefix)).await {
                // Filter by timestamp and format
                for msg in &mut messages {
                    if msg.timestamp > start_time {
                        msg.formatted_timestamp = format_timestamp(msg.timestamp, timezone.clone());
                        msg.room_name = room_name.clone();
                        all_messages.push(msg.clone());
                    }
                }
            }
        }
    }

    // Sort by timestamp (most recent first)
    all_messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(all_messages)
}

// ============================================================================
// Original Functions (maintained for backward compatibility)
// ============================================================================

pub fn find_exact_room(bridge_rooms: &[BridgeRoom], search_term: &str) -> Option<BridgeRoom> {
    let search_term_lower = search_term.trim().to_lowercase();
    if let Some(room) = bridge_rooms
        .iter()
        .find(|r| remove_bridge_suffix(r.display_name.as_str()).to_lowercase() == search_term_lower)
    {
        tracing::info!("Found exact match for room");
        return Some(room.clone());
    }
    None
}

pub fn search_best_match(bridge_rooms: &[BridgeRoom], search_term: &str) -> Option<BridgeRoom> {
    let search_term_lower = search_term.trim().to_lowercase();
    // Try exact match first (fastest)
    if let Some(room) = bridge_rooms
        .iter()
        .find(|r| remove_bridge_suffix(r.display_name.as_str()).to_lowercase() == search_term_lower)
    {
        tracing::info!("Found exact match for room");
        return Some(room.clone());
    }
    // Then try substring match
    if let Some(room) = bridge_rooms
        .iter()
        .filter(|r| {
            remove_bridge_suffix(r.display_name.as_str())
                .to_lowercase()
                .contains(&search_term_lower)
        })
        .max_by_key(|r| r.last_activity)
    {
        tracing::info!("Found substring match for room");
        return Some(room.clone());
    }
    // Finally try similarity match
    let best_match = bridge_rooms
        .iter()
        .map(|r| {
            (
                strsim::jaro_winkler(
                    &search_term_lower,
                    &remove_bridge_suffix(r.display_name.as_str()).to_lowercase(),
                ),
                r,
            )
        })
        .filter(|(score, _)| *score >= 0.7)
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    if let Some((score, room)) = best_match {
        tracing::info!("Found similar match with score {}", score);
        Some(room.clone())
    } else {
        None
    }
}

pub fn get_best_matches(bridge_rooms: &[BridgeRoom], search_term: &str) -> Vec<String> {
    let search_term_lower = search_term.trim().to_lowercase();
    let mut matches: Vec<(f64, String)> = bridge_rooms
        .iter()
        .map(|r| {
            let name = remove_bridge_suffix(&r.display_name);
            let name_lower = name.to_lowercase();
            (
                strsim::jaro_winkler(&search_term_lower, &name_lower),
                name.to_string(),
            )
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
            return Err(anyhow!(
                "{} bridge is not connected. Please log in first.",
                capitalize(service)
            ));
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
            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(
                SyncRoomMessageEvent::Original(e),
            ),
        )) = event.raw().deserialize()
        {
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
                        MessageType::Image(i) => (
                            "image",
                            if i.body.is_empty() {
                                "📎 IMAGE".into()
                            } else {
                                i.body
                            },
                        ),
                        MessageType::Video(v) => (
                            "video",
                            if v.body.is_empty() {
                                "📎 VIDEO".into()
                            } else {
                                v.body
                            },
                        ),
                        MessageType::File(f) => (
                            "file",
                            if f.body.is_empty() {
                                "📎 FILE".into()
                            } else {
                                f.body
                            },
                        ),
                        MessageType::Audio(a) => (
                            "audio",
                            if a.body.is_empty() {
                                "📎 AUDIO".into()
                            } else {
                                a.body
                            },
                        ),
                        MessageType::Location(_l) => ("location", "📍 LOCATION".into()),
                        MessageType::Emote(t) => ("emote", t.body),
                        _ => continue,
                    };

                    // Skip error-like messages
                    if body.contains("Failed to bridge media")
                        || body.contains("media no longer available")
                        || body.contains("Decrypting message from WhatsApp failed")
                        || body.starts_with("* Failed to")
                    {
                        continue;
                    }

                    return Ok(Some(BridgeMessage {
                        sender: e.sender.to_string(),
                        sender_display_name: sender_localpart,
                        content: body,
                        timestamp,
                        formatted_timestamp: format_timestamp(
                            timestamp,
                            user_info.timezone.clone(),
                        ),
                        message_type: msgtype.to_string(),
                        room_name: cleaned_room_name,
                        media_url: None,
                        room_id: Some(room_id.to_string()),
                    }));
                }
            }
        }
    }

    // If no triggering message found after the prompt
    tracing::info!(
        "No triggering incoming message found before the AI prompt in room '{}'",
        room_id_str
    );
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
            return Err(anyhow!(
                "{} bridge is not connected. Please log in first.",
                capitalize(service)
            ));
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
            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(
                SyncRoomMessageEvent::Original(e),
            ),
        )) = event.raw().deserialize()
        {
            let sender_localpart = e.sender.localpart().to_string();
            // Check if sender is the user (matches user_matrix_id and not a bridge bot prefix)
            if e.sender == user_matrix_id && !sender_localpart.starts_with(&sender_prefix) {
                let timestamp = i64::from(e.origin_server_ts.0) / 1000;

                // Extract message type and body
                let (msgtype, body) = match e.content.msgtype {
                    MessageType::Text(t) => ("text", t.body),
                    MessageType::Notice(n) => ("notice", n.body),
                    MessageType::Image(i) => (
                        "image",
                        if i.body.is_empty() {
                            "📎 IMAGE".into()
                        } else {
                            i.body
                        },
                    ),
                    MessageType::Video(v) => (
                        "video",
                        if v.body.is_empty() {
                            "📎 VIDEO".into()
                        } else {
                            v.body
                        },
                    ),
                    MessageType::File(f) => (
                        "file",
                        if f.body.is_empty() {
                            "📎 FILE".into()
                        } else {
                            f.body
                        },
                    ),
                    MessageType::Audio(a) => (
                        "audio",
                        if a.body.is_empty() {
                            "📎 AUDIO".into()
                        } else {
                            a.body
                        },
                    ),
                    MessageType::Location(_l) => ("location", "📍 LOCATION".into()),
                    MessageType::Emote(t) => ("emote", t.body),
                    _ => continue,
                };

                // Skip error-like messages if needed (adapted from existing logic)
                if body.contains("Failed to bridge media")
                    || body.contains("media no longer available")
                    || body.contains("Decrypting message from WhatsApp failed")
                    || body.starts_with("* Failed to")
                {
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
                    room_id: Some(room_id.to_string()),
                }));
            }
        }
    }

    // If no user-sent message found within the limit
    tracing::info!(
        "No sent message found in the last 100 messages for room '{}'",
        room_id_str
    );
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
        capitalize(service),
        user_id,
        chat_name,
        limit.unwrap_or(20)
    );
    if let Some(bridge) = state.user_repository.get_bridge(user_id, service)? {
        if bridge.status != "connected" {
            return Err(anyhow!(
                "{} bridge is not connected. Please log in first.",
                capitalize(service)
            ));
        }
    } else {
        return Err(anyhow!("{} bridge not found", capitalize(service)));
    }
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
    let rooms = get_service_rooms(&client, service).await?;
    let matching_room = search_best_match(&rooms, chat_name);
    let user_info = state.user_core.get_user_info(user_id)?;
    match matching_room {
        Some(room_info) => {
            let room_id = match matrix_sdk::ruma::OwnedRoomId::try_from(room_info.room_id.as_str())
            {
                Ok(id) => id,
                Err(e) => return Err(anyhow!("Invalid room ID: {}", e)),
            };
            let room = match client.get_room(&room_id) {
                Some(r) => r,
                None => return Err(anyhow!("Room not found")),
            };
            fetch_messages_from_room(service, room, limit, user_info.timezone).await
        }
        None => Err(anyhow!(
            "No matching {} room found for '{}'",
            capitalize(service),
            chat_name
        )),
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

    let user_info = state.user_core.get_user_info(user_id)?;
    // Get Matrix client and check bridge status (use cached version for better performance)
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
    let bridge = state.user_repository.get_bridge(user_id, service)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!(
            "{} bridge is not connected. Please log in first.",
            capitalize(service)
        ));
    }
    // Get last_seen_online from the bridge for additional filtering when unread_only
    let bridge_last_seen = state
        .user_repository
        .get_bridge(user_id, service)?
        .and_then(|b| b.last_seen_online)
        .unwrap_or(0) as i64;

    let service_rooms = get_service_rooms(&client, service).await?;
    if user_id == 1 {
        tracing::info!(
            "DEBUG user 1: Found {} {} rooms",
            service_rooms.len(),
            service
        );
        for room in &service_rooms {
            tracing::info!(
                "DEBUG user 1: Room: {} (last_activity: {})",
                room.display_name,
                room.last_activity_formatted
            );
        }
    }
    let mut room_infos: Vec<(Room, BridgeRoom, i64)> = Vec::new(); // (room, bridge_room, seen_until)
    for bridge_room in service_rooms {
        let room_id = match matrix_sdk::ruma::OwnedRoomId::try_from(bridge_room.room_id.as_str()) {
            Ok(id) => id,
            Err(_) => continue,
        };
        let Some(room) = client.get_room(&room_id) else {
            continue;
        };
        if room.user_defined_notification_mode().await == Some(RoomNotificationMode::Mute) {
            continue;
        }

        // Calculate seen_until: the timestamp up to which messages should be filtered out
        let seen_until = if unread_only {
            // Get the room's seen timestamp from read receipts and user replies
            let room_seen = get_room_seen_timestamp(&room, &client).await.unwrap_or(0);
            // Use the max of: start_time, room_seen, bridge_last_seen
            start_time.max(room_seen).max(bridge_last_seen)
        } else {
            // For live fetching, just use start_time (no filtering by seen status)
            start_time
        };

        room_infos.push((room, bridge_room, seen_until));
    }
    // Already sorted by last_activity desc from get_service_rooms
    room_infos.truncate(5);
    if user_id == 1 {
        tracing::info!(
            "DEBUG user 1: Processing {} rooms after truncate, start_time={}, unread_only={}",
            room_infos.len(),
            start_time,
            unread_only
        );
        for (_, br, seen_until) in &room_infos {
            tracing::info!(
                "DEBUG user 1: Will check room: {} with seen_until={}",
                br.display_name,
                seen_until
            );
        }
    }
    // Fetch messages in parallel
    let user_timezone = user_info.timezone.clone();
    let sender_prefix = get_sender_prefix(service);
    let mut futures = Vec::new();
    for (room, bridge_room, seen_until) in room_infos {
        let sender_prefix = sender_prefix.clone();
        let user_timezone = user_timezone.clone();
        let room_name = remove_bridge_suffix(&bridge_room.display_name);
        let bridge_room_id = bridge_room.room_id.clone();
        let debug_user = user_id == 1;
        if room.user_defined_notification_mode().await == Some(RoomNotificationMode::Mute) {
            tracing::info!("Skipping message from a muted room");
            continue;
        }
        futures.push(async move {
            let mut options = matrix_sdk::room::MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(50).unwrap(); // Fetch enough to cover filters
            let mut messages: Vec<BridgeMessage> = Vec::new();
            match room.messages(options).await {
                Ok(response) => {
                    for event in response.chunk.iter() {
                        if let Ok(AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
                        )) = event.raw().deserialize() {
                            let (sender, timestamp, content) = match msg {
                                SyncRoomMessageEvent::Original(e) => {
                                    let timestamp = i64::from(e.origin_server_ts.0) / 1000;
                                    (e.sender, timestamp, e.content)
                                }
                                _ => continue,
                            };
                            // Skip messages that user has already seen (or outside time range)
                            if timestamp <= seen_until {
                                if debug_user {
                                    tracing::info!("DEBUG user 1: Skipping msg in {} - timestamp {} <= seen_until {}", room_name, timestamp, seen_until);
                                }
                                continue;
                            }
                            if !sender.localpart().starts_with(&sender_prefix) {
                                if debug_user {
                                    tracing::info!("DEBUG user 1: Skipping msg in {} - sender {} doesn't start with {}", room_name, sender.localpart(), sender_prefix);
                                }
                                continue;
                            }
                            let (msgtype, body) = match content.msgtype {
                                MessageType::Text(t) => ("text", t.body),
                                MessageType::Notice(n) => ("notice", n.body),
                                MessageType::Image(i) => ("image", if i.body.is_empty() { "📎 IMAGE".into() } else { i.body }),
                                MessageType::Video(v) => ("video", if v.body.is_empty() { "📎 VIDEO".into() } else { v.body }),
                                MessageType::File(f) => ("file", if f.body.is_empty() { "📎 FILE".into() } else { f.body }),
                                MessageType::Audio(a) => ("audio", if a.body.is_empty() { "📎 AUDIO".into() } else { a.body }),
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
                            if debug_user {
                                tracing::info!("DEBUG user 1: Including msg in {} - timestamp {}", room_name, timestamp);
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
                                room_id: Some(bridge_room_id.clone()),
                            });
                            if messages.len() == 5 {
                                break;
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
    tracing::info!(
        "Retrieved {} latest messages from most active rooms",
        messages.len()
    );
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

    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
    let bridge = state.user_repository.get_bridge(user_id, service)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        return Err(anyhow!(
            "{} bridge is not connected. Please log in first.",
            capitalize(service)
        ));
    }
    let service_rooms = get_service_rooms(&client, service).await?;
    let exact_room = find_exact_room(&service_rooms, chat_name);
    let room = match exact_room {
        Some(room_info) => {
            let room_id = match matrix_sdk::ruma::OwnedRoomId::try_from(room_info.room_id.as_str())
            {
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
                format!(
                    "Could not find exact matching {} room for '{}'",
                    capitalize(service),
                    chat_name
                )
            } else {
                format!(
                    "Could not find exact matching {} room for '{}'. Did you mean one of these?\n{}",
                    capitalize(service),
                    chat_name,
                    suggestions.join("\n")
                )
            };
            return Err(anyhow!(error_msg));
        }
    };
    use matrix_sdk::ruma::events::room::message::{
        ImageMessageEventContent, MessageType, RoomMessageEventContent,
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
        // ── 2. Upload to the homeserver ──────────────────────────────────────────
        let upload_resp = client.media().upload(&mime, bytes.to_vec(), None).await?;
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
        room.send(RoomMessageEventContent::text_plain(message))
            .await?;
    }
    tracing::debug!("Message sent!");
    let user_info = state.user_core.get_user_info(user_id)?;
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
        room_id: Some(room.room_id().to_string()),
    })
}

use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;
use matrix_sdk::RoomMemberships;
use strsim;

async fn fetch_messages_from_room(
    service: &str,
    room: matrix_sdk::room::Room,
    limit: Option<u64>,
    timezone: Option<String>,
) -> Result<(Vec<BridgeMessage>, String)> {
    let room_name = room.display_name().await?.to_string();
    let room_id_str = room.room_id().to_string();
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
        let room_id_str = room_id_str.clone();
        futures.push(async move {
            if let Ok(AnySyncTimelineEvent::MessageLike(
                matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg),
            )) = event.raw().deserialize()
            {
                let (sender, timestamp, content) = match msg {
                    SyncRoomMessageEvent::Original(e) => {
                        (e.sender, i64::from(e.origin_server_ts.0) / 1000, e.content)
                    }
                    _ => return None,
                };

                if !sender.localpart().starts_with(&sender_prefix) {
                    return None;
                }

                let (msgtype, body) = match content.msgtype {
                    MessageType::Text(t) => ("text", t.body),
                    MessageType::Notice(n) => ("notice", n.body),
                    MessageType::Image(i) => (
                        "image",
                        if i.body.is_empty() {
                            "📎 IMAGE".into()
                        } else {
                            i.body
                        },
                    ),
                    MessageType::Video(v) => (
                        "video",
                        if v.body.is_empty() {
                            "📎 VIDEO".into()
                        } else {
                            v.body
                        },
                    ),
                    MessageType::File(f) => (
                        "file",
                        if f.body.is_empty() {
                            "📎 FILE".into()
                        } else {
                            f.body
                        },
                    ),
                    MessageType::Audio(a) => (
                        "audio",
                        if a.body.is_empty() {
                            "📎 AUDIO".into()
                        } else {
                            a.body
                        },
                    ),
                    MessageType::Location(_) => ("location", "📍 LOCATION".into()), // Location has no body field
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
                    room_id: Some(room_id_str.clone()),
                })
            } else {
                None
            }
        });
    }

    // Collect results from parallel processing
    let mut messages: Vec<BridgeMessage> = join_all(futures).await.into_iter().flatten().collect();

    // Sort messages by timestamp (most recent first)
    messages.sort_unstable_by_key(|m| std::cmp::Reverse(m.timestamp));

    Ok((messages, room_name))
}

use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the timestamp (in seconds) up to which the user has "seen" messages in this room.
/// This checks read receipts and user replies to determine what the user has already seen.
/// Returns None if no seen status can be determined.
pub async fn get_room_seen_timestamp(room: &Room, client: &MatrixClient) -> Option<i64> {
    use matrix_sdk::ruma::{
        api::client::room::get_room_event,
        events::receipt::{ReceiptThread, ReceiptType},
    };

    let own_user_id = client.user_id()?;
    let mut seen_until: Option<i64> = None;

    // Check read receipt - if user has read messages up to a certain point
    if let Ok(Some((receipt_event_id, _))) = room
        .load_user_receipt(ReceiptType::Read, ReceiptThread::Unthreaded, own_user_id)
        .await
    {
        let request =
            get_room_event::v3::Request::new(room.room_id().to_owned(), receipt_event_id.clone());
        if let Ok(response) = client.send(request).await {
            if let Ok(any_event) = response.event.deserialize_as::<AnySyncTimelineEvent>() {
                let receipt_ts = i64::from(any_event.origin_server_ts().as_secs());
                seen_until = Some(seen_until.unwrap_or(0).max(receipt_ts));
            }
        }
    }

    // Check for user replies - if user replied, they've seen messages before that
    let messages = room.messages(MessagesOptions::backward()).await;
    if let Ok(messages) = messages {
        for message_event in messages.chunk {
            if let Ok(AnySyncTimelineEvent::MessageLike(msg_event)) =
                message_event.raw().deserialize()
            {
                if msg_event.sender() == own_user_id {
                    let reply_ts = i64::from(msg_event.origin_server_ts().as_secs());
                    seen_until = Some(seen_until.unwrap_or(0).max(reply_ts));
                    break; // Found the most recent user reply
                }
            }
        }
    }

    seen_until
}

pub async fn handle_bridge_message(
    event: OriginalSyncRoomMessageEvent,
    room: Room,
    client: MatrixClient,
    state: Arc<AppState>,
) {
    tracing::info!(
        "🔔 Bridge message handler - room: {}, sender: {}",
        room.room_id(),
        event.sender
    );
    if room.user_defined_notification_mode().await == Some(RoomNotificationMode::Mute) {
        tracing::info!("Skipping message from a muted room");
        return;
    }

    // Check message age using pure function
    const HALF_HOUR_MS: u64 = 30 * 60 * 1000;
    let message_ts: u64 = event.origin_server_ts.0.into();
    if !should_process_message(message_ts, HALF_HOUR_MS) {
        tracing::info!("Skipping old message (event ID: {})", event.event_id);
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
    let user = match state
        .user_repository
        .get_user_by_matrix_user_id(local_user_id)
    {
        Ok(Some(user)) => user,
        _ => return,
    };
    let user_id = user.id;

    // Early exit: Skip ALL message processing if any bridge is in "connecting" state
    // This prevents blocking the connection flow with long waits
    let connecting_bridge_types = vec!["signal", "telegram", "whatsapp", "instagram", "messenger"];
    for bridge_type in &connecting_bridge_types {
        if let Ok(Some(bridge)) = state.user_repository.get_bridge(user_id, bridge_type) {
            if bridge.status == "connecting" {
                tracing::debug!(
                    "⏳ Skipping message processing - {} bridge is connecting",
                    bridge_type
                );
                return;
            }
        }
    }

    // Check if this is a bridge management room
    let room_id_str = room.room_id().to_string();
    let bridge_types = vec!["signal", "telegram", "whatsapp"];
    let mut bridges = Vec::new();
    for bridge_type in &bridge_types {
        if let Ok(Some(bridge)) = state.user_repository.get_bridge(user_id, bridge_type) {
            bridges.push(bridge);
        }
    }
    tracing::info!(
        "🔍 Checking if room {} is a management room (found {} bridges)",
        room_id_str,
        bridges.len()
    );
    if let Some(bridge) = bridges
        .iter()
        .find(|b| b.room_id.as_ref() == Some(&room_id_str))
    {
        // This is a management room for a bridge
        tracing::info!(
            "📋 Processing message in {} bridge management room (status: {})",
            bridge.bridge_type,
            bridge.status
        );
        // Skip if bridge is in connecting state (handled by monitor task)
        if bridge.status == "connecting" {
            tracing::info!(
                "⏳ Skipping disconnection check during initial connection for {}",
                bridge.bridge_type
            );
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
            tracing::info!(
                "📤 Message not from bridge bot (sender: {}, expected: {}), skipping",
                event.sender,
                bot_user_id
            );
            return;
        }
        tracing::info!("✅ Message IS from bridge bot");

        // Extract message content
        let content = match event.content.msgtype {
            MessageType::Text(t) => t.body,
            MessageType::Notice(n) => n.body,
            _ => {
                tracing::debug!("Non-text/notice message in management room, skipping");
                return;
            }
        };
        // Log bridge bot management room messages for debugging (no more email)
        tracing::info!(
            "🤖 Bridge bot ({}) management room message: {}",
            bridge.bridge_type,
            content
        );

        // Skip health check related messages - these are handled by the health check endpoint
        if is_health_check_message(&content) {
            tracing::info!("⏭️ Skipping health check / status message in management room");
            return;
        }

        // Check for disconnection patterns using the pure function
        tracing::info!("📝 Bot message content: {}", content);
        if is_disconnection_message(&content) {
            tracing::info!(
                "🚨 Detected disconnection in {} bridge for user {}: {}",
                bridge.bridge_type,
                user_id,
                content
            );

            // Record disconnection as triage item + legacy event
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            let bridge_name = match bridge.bridge_type.as_str() {
                "whatsapp" => "WhatsApp",
                "telegram" => "Telegram",
                "signal" => "Signal",
                other => other,
            };

            let new_item = crate::models::user_models::NewItem {
                user_id,
                summary: format!("System: {} bridge disconnected.", bridge_name),
                monitor: false,
                due_at: None,
                next_check_at: None,
                priority: 1,
                source_id: None,
                created_at: current_time,
            };

            if let Err(e) = state.item_repository.create_item(&new_item) {
                tracing::error!("Failed to create item for bridge disconnection: {}", e);
            }

            // Legacy table (backward compat)
            if let Err(e) = state.user_repository.record_bridge_disconnection(
                user_id,
                &bridge.bridge_type,
                current_time,
            ) {
                tracing::error!(
                    "Failed to record disconnection event for user {}: {}",
                    user_id,
                    e
                );
            }

            // Delete the bridge record
            if let Err(e) = state
                .user_repository
                .delete_bridge(user_id, &bridge.bridge_type)
            {
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
    use matrix_sdk::ruma::{
        api::client::room::get_room_event,
        events::receipt::{ReceiptThread, ReceiptType},
    };
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

    const SHORT_WAIT: u64 = 120; // 2 minutes (increased from 30s)
    const LONG_WAIT: u64 = 600; // 10 minutes (increased from 5min)
    const ACTIVITY_THRESHOLD: i32 = 300; // 5 minutes

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
    tracing::info!(
        "Waiting for {} seconds before processing (user activity inferred as {})",
        wait_time,
        if wait_time == SHORT_WAIT {
            "inactive"
        } else {
            "active"
        }
    );

    sleep(Duration::from_secs(wait_time)).await;

    // Check if user has read this or a later message (via bridged receipt)
    let own_user_id = client.user_id().unwrap();
    if let Ok(Some((receipt_event_id, _))) = room
        .load_user_receipt(ReceiptType::Read, ReceiptThread::Unthreaded, own_user_id)
        .await
    {
        // Fetch the receipted event to get its timestamp (approximate order)
        let request =
            get_room_event::v3::Request::new(room.room_id().to_owned(), receipt_event_id.clone());
        if let Ok(response) = client.send(request).await {
            if let Ok(any_event) = response.event.deserialize_as::<AnySyncTimelineEvent>() {
                if any_event.origin_server_ts().0 >= event.origin_server_ts.0 {
                    tracing::info!(
                        "Skipping processing because user has read this or a later message"
                    );
                    let last_seen_online =
                        i32::try_from(any_event.origin_server_ts().as_secs()).unwrap();
                    let rows = state
                        .user_repository
                        .update_bridge_last_seen_online(user_id, service.as_str(), last_seen_online)
                        .unwrap();
                    tracing::info!("Updated {:#?} rows for last_seen_online (user_id: {}, service: {}, value: {})", rows, user_id, service, last_seen_online);
                    if rows == 0 {
                        tracing::warn!(
                            "No bridge row matched for update - possible race or mismatch"
                        );
                    }
                    tracing::info!("set the last_seen_online to: {}", last_seen_online);
                    // Auto-dismiss any pending items for this room
                    let _ = state
                        .item_repository
                        .delete_items_by_source(user_id, &room_id_str);
                    return;
                }
            }
        }
    }

    tracing::info!("No recent read detected via read receipt; checking for user replies");

    // Check if user has sent any replies after this message (strongest signal they've seen it)
    let messages = room.messages(MessagesOptions::backward()).await;
    if let Ok(messages) = messages {
        let mut found_user_reply = false;
        for message_event in messages.chunk {
            if let Ok(AnySyncTimelineEvent::MessageLike(msg_event)) =
                message_event.raw().deserialize()
            {
                // Check if this message is from the user (not the bridge bot)
                if msg_event.sender() == own_user_id {
                    // Check if user's message came after the trigger message
                    if msg_event.origin_server_ts().0 > event.origin_server_ts.0 {
                        tracing::info!(
                            "User has sent a reply after this message - skipping notification"
                        );
                        found_user_reply = true;

                        // Update last_seen_online based on user's reply timestamp
                        let last_seen_online =
                            i32::try_from(msg_event.origin_server_ts().as_secs()).unwrap();
                        let rows = state
                            .user_repository
                            .update_bridge_last_seen_online(
                                user_id,
                                service.as_str(),
                                last_seen_online,
                            )
                            .unwrap();
                        tracing::info!("Updated {} rows for last_seen_online based on reply (user_id: {}, service: {}, value: {})",
                            rows, user_id, service, last_seen_online);
                        break;
                    }
                }
            }
        }

        if found_user_reply {
            // Auto-dismiss any pending items for this room
            let _ = state
                .item_repository
                .delete_items_by_source(user_id, &room_id_str);
            return;
        }
    }

    tracing::info!("No user reply detected; proceeding with message processing");
    let sender_prefix = get_sender_prefix(&service);
    tracing::debug!("sender_prefix: {}", sender_prefix);
    if !sender_localpart.starts_with(&sender_prefix) {
        tracing::info!("Skipping non-{} sender", service);
        return;
    }
    // Check if user has valid subscription
    let has_valid_sub = state
        .user_repository
        .has_valid_subscription_tier(user_id, "tier 2")
        .unwrap_or(false);
    if !has_valid_sub {
        tracing::debug!(
            "User {} does not have valid subscription for WhatsApp monitoring",
            user_id
        );
        return;
    }
    if !state
        .user_core
        .get_proactive_agent_on(user_id)
        .unwrap_or(true)
    {
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
    tracing::debug!("message: {}", content);

    // Check monitor items for messaging matches
    let message_context = format!(
        "From: {}\nService: {}\nRoom: {}\nContent: {}",
        sender_localpart,
        service,
        room_name.as_str(),
        content
    );
    let monitor_items = match state.item_repository.get_monitor_items(user_id) {
        Ok(items) => items,
        Err(e) => {
            tracing::error!("Failed to get monitor items for user {}: {}", user_id, e);
            Vec::new()
        }
    };
    if !monitor_items.is_empty() {
        // Extract data from Result immediately to drop non-Send Box<dyn Error> before any .await
        let maybe_match: Option<(
            crate::proactive::utils::ItemMatchResponse,
            crate::models::user_models::Item,
        )> = crate::proactive::utils::check_item_monitor_match(
            &state,
            user_id,
            &message_context,
            &monitor_items,
        )
        .await
        .ok()
        .flatten()
        .and_then(|resp| {
            let item_id = resp.task_id.unwrap_or(0);
            monitor_items
                .iter()
                .find(|i| i.id == Some(item_id))
                .cloned()
                .map(|item| (resp, item))
        });
        if let Some((match_response, matched_item)) = maybe_match {
            crate::proactive::utils::apply_monitor_lifecycle(
                &state,
                &matched_item,
                &match_response,
            )
            .await;
            return;
        }
    }

    // Extract chat_name early for contact profile matching
    let chat_name = remove_bridge_suffix(room_name.as_str());
    let current_room_id = room.room_id().to_string();

    // Load contact profiles for matching
    let contact_profiles = state
        .user_repository
        .get_contact_profiles(user_id)
        .unwrap_or_default();

    // Find matching contact profile - try room_id first (stable), then display name
    let matching_profile = contact_profiles
        .iter()
        .find(|p| {
            // Try room_id match first
            let profile_room_id = match service.as_str() {
                "whatsapp" => p.whatsapp_room_id.as_deref(),
                "telegram" => p.telegram_room_id.as_deref(),
                "signal" => p.signal_room_id.as_deref(),
                _ => None,
            };
            profile_room_id == Some(current_room_id.as_str())
        })
        .or_else(|| {
            // Fall back to display name matching for legacy/proactive profiles
            let name_match = contact_profiles.iter().find(|p| {
                let chat_lower = chat_name.to_lowercase();
                match service.as_str() {
                    "whatsapp" => p
                        .whatsapp_chat
                        .as_ref()
                        .map(|c| {
                            let c_lower = remove_bridge_suffix(c).to_lowercase();
                            chat_lower.contains(&c_lower) || c_lower.contains(&chat_lower)
                        })
                        .unwrap_or(false),
                    "telegram" => p
                        .telegram_chat
                        .as_ref()
                        .map(|c| {
                            let c_lower = remove_bridge_suffix(c).to_lowercase();
                            chat_lower.contains(&c_lower) || c_lower.contains(&chat_lower)
                        })
                        .unwrap_or(false),
                    "signal" => p
                        .signal_chat
                        .as_ref()
                        .map(|c| {
                            let c_lower = remove_bridge_suffix(c).to_lowercase();
                            chat_lower.contains(&c_lower) || c_lower.contains(&chat_lower)
                        })
                        .unwrap_or(false),
                    _ => false,
                }
            });

            // Auto-save room_id on name-based match so future messages use room_id directly
            if let Some(profile) = name_match {
                let profile_room_id = match service.as_str() {
                    "whatsapp" => profile.whatsapp_room_id.as_deref(),
                    "telegram" => profile.telegram_room_id.as_deref(),
                    "signal" => profile.signal_room_id.as_deref(),
                    _ => None,
                };
                if profile_room_id.is_none() || profile_room_id == Some("") {
                    if let Some(pid) = profile.id {
                        if let Err(e) = state.user_repository.update_profile_room_id(
                            pid,
                            &service,
                            &current_room_id,
                        ) {
                            tracing::warn!(
                                "Failed to auto-save room_id for profile {}: {}",
                                pid,
                                e
                            );
                        } else {
                            tracing::info!(
                                "Auto-saved {} room_id {} for profile {} ({})",
                                service,
                                current_room_id,
                                pid,
                                profile.nickname
                            );
                        }
                    }
                }
            }

            name_match
        });

    // Check if this is a group room (more than 3 members)
    let members = match room.members(RoomMemberships::JOIN).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Failed to fetch room members: {}", e);
            return;
        }
    };
    let member_count = members.len() as u64;
    tracing::debug!("members: {}", member_count);

    // Group chat handling: check notification mode to bypass @mention requirement
    if member_count > 3 {
        // Check platform-specific exception mode first, then fall back to profile mode
        let effective_mode = matching_profile
            .and_then(|p| {
                let profile_id = p.id.unwrap_or(0);
                state
                    .user_repository
                    .get_profile_exception_for_platform(profile_id, &service)
                    .ok()
                    .flatten()
                    .map(|exc| exc.notification_mode)
                    .or_else(|| Some(p.notification_mode.clone()))
            })
            .unwrap_or_default();
        let is_priority_group = effective_mode == "all";

        if !is_priority_group {
            let is_mentioned = event
                .content
                .mentions
                .as_ref()
                .map(|m| m.user_ids.contains(&matrix_user_id))
                .unwrap_or(false);
            if !is_mentioned {
                tracing::info!("Skipping message from group room ({} members) since user wasn't mentioned and not in 'all' mode profile", member_count);
                return;
            }
            tracing::info!(
                "User {} is mentioned in message (event ID: {})",
                user_id,
                event.event_id
            );
        } else {
            tracing::info!(
                "Group {} matches contact profile with 'all' mode, processing all messages",
                chat_name
            );
        }
    }
    // Skip error messages using pure function
    if is_error_message(&content) {
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
    // chat_name and sender_name already defined earlier for contact profile matching

    fn trim_for_sms(service: &str, sender: &str, content: &str) -> String {
        let prefix = format!("{} from ", capitalize(service));
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
        format!(
            "{}{}{}{}",
            prefix, sender_trimmed, separator, content_trimmed
        )
    }

    let service_cap = capitalize(&service);

    // Get user settings for default notification mode
    let user_settings = match state.user_core.get_user_settings(user_id) {
        Ok(settings) => settings,
        Err(e) => {
            tracing::error!("Failed to get user settings: {}", e);
            return;
        }
    };

    // Extract contact notes before the match consumes matching_profile
    let contact_notes = matching_profile.as_ref().and_then(|p| p.notes.clone());

    // Determine notification mode and type based on contact profile or default
    // Check for platform-specific exceptions if a profile matches
    let (notification_mode, notification_type_str, notify_on_call, profile_nickname) =
        match matching_profile {
            Some(profile) => {
                let profile_id = profile.id.unwrap_or(0);
                // Check for platform-specific exception
                let exception = state
                    .user_repository
                    .get_profile_exception_for_platform(profile_id, &service)
                    .ok()
                    .flatten();

                match exception {
                    Some(exc) => {
                        tracing::info!(
                            "Using {} exception for profile '{}': mode={}, type={}",
                            service,
                            profile.nickname,
                            exc.notification_mode,
                            exc.notification_type
                        );
                        (
                            exc.notification_mode.clone(),
                            exc.notification_type.clone(),
                            exc.notify_on_call != 0,
                            Some(profile.nickname.clone()),
                        )
                    }
                    None => (
                        profile.notification_mode.clone(),
                        profile.notification_type.clone(),
                        profile.notify_on_call != 0,
                        Some(profile.nickname.clone()),
                    ),
                }
            }
            None => {
                let is_phone_contact = is_phone_contact_from_room_name(room_name.as_str());
                // is_phone_contact:
                //   Some(true)  = phone contact (WA/Signal, has FullName) -> Tier 2
                //   Some(false) = not phone contact (WA~/Signal~) -> Tier 3
                //   None        = Telegram or unknown -> default to Tier 2

                if is_phone_contact.unwrap_or(true) {
                    // Tier 2: Phone contact (or Telegram where we can't distinguish)
                    let mode = user_settings
                        .phone_contact_notification_mode
                        .clone()
                        .unwrap_or_else(|| "critical".to_string());
                    let ntype = user_settings
                        .phone_contact_notification_type
                        .clone()
                        .unwrap_or_else(|| "sms".to_string());
                    let notify = user_settings.phone_contact_notify_on_call != 0;
                    (mode, ntype, notify, None)
                } else {
                    // Tier 3: Unknown person (not in phone contacts)
                    let mode = user_settings
                        .default_notification_mode
                        .clone()
                        .unwrap_or_else(|| "critical".to_string());
                    let ntype = user_settings
                        .default_notification_type
                        .clone()
                        .unwrap_or_else(|| "sms".to_string());
                    let notify = user_settings.default_notify_on_call != 0;
                    (mode, ntype, notify, None)
                }
            }
        };

    tracing::debug!("notification_mode: {}", notification_mode);
    tracing::debug!("notification_type: {}", notification_type_str);
    tracing::debug!("profile_nickname: {:?}", profile_nickname);

    // Handle incoming/missed calls using per-profile notify_on_call setting
    if content.contains("Incoming call") || content.contains("Missed call") {
        if notify_on_call {
            let suffix = match notification_type_str.as_str() {
                "call" => "_call",
                "call_sms" => "_call_sms",
                _ => "_sms",
            };
            let notification_type = format!("{}_incoming_call{}", service, suffix);
            let what_to_inform = format!(
                "You have an incoming {} call from {}",
                service_cap, chat_name
            );
            let first_message = format!(
                "Hello, you have an incoming {} call from {}.",
                service_cap, chat_name
            );

            let state_clone = state.clone();
            tokio::spawn(async move {
                crate::proactive::utils::send_notification(
                    &state_clone,
                    user_id,
                    &what_to_inform,
                    notification_type,
                    Some(first_message),
                )
                .await;
            });
        } else {
            tracing::debug!(
                "Skipping incoming call notification for user {} (notify_on_call=false)",
                user_id
            );
        }
        return;
    }

    // Handle based on notification mode
    match notification_mode.as_str() {
        "all" => {
            // Send immediate notification for ALL messages from this contact/group
            let suffix = match notification_type_str.as_str() {
                "call" => "_call",
                "call_sms" => "_call_sms",
                _ => "_sms",
            };
            let notification_type = format!("{}_profile{}", service, suffix);

            // Check if user has enough credits
            match crate::utils::usage::check_user_credits(&state, &user, "noti_msg", None).await {
                Ok(()) => {
                    let state_clone = state.clone();
                    let content_clone = content.clone();
                    let sender_display = profile_nickname.as_ref().unwrap_or(&chat_name);
                    let message = trim_for_sms(&service, sender_display, &content_clone);
                    let first_message = format!(
                        "Hello, you have a {} message from {}.",
                        service_cap, sender_display
                    );

                    tokio::spawn(async move {
                        crate::proactive::utils::send_notification(
                            &state_clone,
                            user_id,
                            &message,
                            notification_type,
                            Some(first_message),
                        )
                        .await;
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        "User {} does not have enough credits for profile notification: {}",
                        user_id,
                        e
                    );
                }
            }
        }
        "critical" => {
            // Only notify if AI deems the message critical
            if user_settings.critical_enabled.is_none() {
                tracing::debug!("Critical message checking disabled for user {}", user_id);
                return;
            }

            tracing::debug!("service: {}", service);
            tracing::debug!("chat_name: {}", chat_name);
            tracing::debug!("content: {}", content);

            // Use profile nickname for critical message detection if available
            let sender_display = profile_nickname.as_ref().unwrap_or(&chat_name);
            if let Ok((is_critical, message_opt, first_message_opt)) =
                crate::proactive::utils::check_message_importance(
                    &state,
                    user_id,
                    &format!("{} from {}: {}", service_cap, sender_display, content),
                    service_cap.as_str(),
                    sender_display.as_str(),
                    content.as_str(),
                    contact_notes.as_deref(),
                    "",
                )
                .await
            {
                tracing::debug!("is_critical: {}", is_critical);

                if is_critical {
                    // Check if we recently sent a critical notification to avoid duplicates
                    let suffix = match notification_type_str.as_str() {
                        "call" => "_call",
                        "call_sms" => "_call_sms",
                        _ => "_sms",
                    };
                    let notification_type = format!("{}_critical{}", service, suffix);
                    const NOTIFICATION_COOLDOWN: i32 = 600; // 10 minutes

                    if let Ok(has_recent) = state.user_repository.has_recent_notification(
                        user_id,
                        &notification_type,
                        NOTIFICATION_COOLDOWN,
                    ) {
                        if has_recent {
                            tracing::info!("Skipping notification - already sent {} notification within last {} seconds",
                                notification_type, NOTIFICATION_COOLDOWN);
                            return;
                        }
                    }

                    let message = message_opt.unwrap_or(format!("Critical {} message found, failed to get content, but you can check your {} to see it.", service_cap, service));
                    let first_message = first_message_opt.unwrap_or(format!(
                        "Hey, I found some critical {} message.",
                        service_cap
                    ));

                    // Spawn a new task for sending critical message notification
                    let state_clone = state.clone();
                    tokio::spawn(async move {
                        crate::proactive::utils::send_notification(
                            &state_clone,
                            user_id,
                            &message,
                            notification_type,
                            Some(first_message),
                        )
                        .await;
                    });
                }
            }
        }
        "mention" => {
            // @mention only mode: already handled by the group member check above
            // If we reach here, user was @mentioned in a group or this is a DM
            // For groups: user was mentioned, so notify like "all" mode
            if member_count > 3 {
                let suffix = match notification_type_str.as_str() {
                    "call" => "_call",
                    "call_sms" => "_call_sms",
                    _ => "_sms",
                };
                let notification_type = format!("{}_mention{}", service, suffix);

                match crate::utils::usage::check_user_credits(&state, &user, "noti_msg", None).await
                {
                    Ok(()) => {
                        let state_clone = state.clone();
                        let content_clone = content.clone();
                        let sender_display = profile_nickname.as_ref().unwrap_or(&chat_name);
                        let message = trim_for_sms(&service, sender_display, &content_clone);
                        let first_message = format!(
                            "Hello, you were mentioned in a {} group {}.",
                            service_cap, sender_display
                        );

                        tokio::spawn(async move {
                            crate::proactive::utils::send_notification(
                                &state_clone,
                                user_id,
                                &message,
                                notification_type,
                                Some(first_message),
                            )
                            .await;
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            "User {} does not have enough credits for mention notification: {}",
                            user_id,
                            e
                        );
                    }
                }
            }
            // For DMs (member_count <= 3), mention mode doesn't apply, skip
        }
        "digest" | "ignore" => {
            // For digest mode: message will be picked up by digest job, no immediate notification
            // For ignore mode: skip entirely (applies to both default settings and profile exceptions)
            if notification_mode == "ignore" {
                tracing::debug!("Message ignored (notification_mode=ignore)");
            } else {
                tracing::debug!("Message will be included in digest (notification_mode=digest)");
            }
            // Skip triage item creation for digest/ignore
        }
        _ => {
            // Unknown mode, treat as critical
            tracing::warn!(
                "Unknown notification mode: {}, treating as critical",
                notification_mode
            );
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
        return Err(anyhow!(
            "{} bridge is not connected. Please log in first.",
            capitalize(service)
        ));
    }
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
    // Sync to pick up rooms created/joined by the bridge via double puppet
    client
        .sync_once(matrix_sdk::config::SyncSettings::new())
        .await
        .ok();
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
    tracing::info!(
        "Found {} matching {} rooms",
        matching_rooms.len(),
        capitalize(service)
    );

    let mut results: Vec<BridgeRoom> = matching_rooms.into_iter().map(|(_, room)| room).collect();

    // Search Synapse Admin API for bridge ghost users (contacts without rooms).
    // The user directory API excludes appservice users by design, so we use the
    // admin API which searches the users table directly.
    if search_term.trim().len() >= 2 {
        let homeserver_url = std::env::var("MATRIX_HOMESERVER").unwrap_or_default();
        let access_token = client
            .matrix_auth()
            .session()
            .map(|s| s.tokens.access_token.clone())
            .unwrap_or_default();

        if !homeserver_url.is_empty() && !access_token.is_empty() {
            let admin_url = format!(
                "{}/_synapse/admin/v2/users?from=0&limit=50&name={}",
                homeserver_url.trim_end_matches('/'),
                urlencoding::encode(search_term.trim())
            );
            let http_client = reqwest::Client::new();
            match http_client
                .get(&admin_url)
                .header("Authorization", format!("Bearer {}", access_token))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(body) = resp.json::<serde_json::Value>().await {
                        let sender_prefix = get_sender_prefix(service);
                        let bot_names: [String; 2] =
                            [format!("{}bot", service), format!("{}-bridge", service)];

                        let mut existing_names: std::collections::HashSet<String> = results
                            .iter()
                            .map(|r| remove_bridge_suffix(&r.display_name).to_lowercase())
                            .collect();

                        if let Some(users) = body["users"].as_array() {
                            tracing::info!(
                                "Admin API returned {} users for '{}'",
                                users.len(),
                                search_term
                            );
                            for user in users {
                                let name = user["name"].as_str().unwrap_or_default();
                                // Extract localpart from @localpart:server
                                let localpart = name
                                    .trim_start_matches('@')
                                    .split(':')
                                    .next()
                                    .unwrap_or_default();

                                if !localpart.starts_with(&sender_prefix) {
                                    continue;
                                }
                                if bot_names.iter().any(|b| localpart == b.as_str()) {
                                    continue;
                                }

                                let raw_display = user["displayname"].as_str().unwrap_or_default();
                                let display_name = if !raw_display.is_empty() {
                                    remove_bridge_suffix(raw_display)
                                } else {
                                    let number = localpart.trim_start_matches(&sender_prefix);
                                    if number.chars().all(|c| c.is_ascii_digit())
                                        && number.len() > 5
                                    {
                                        format!("+{}", number)
                                    } else {
                                        continue;
                                    }
                                };

                                if existing_names.contains(&display_name.to_lowercase()) {
                                    continue;
                                }

                                let name_lower = display_name.to_lowercase();
                                let is_match = name_lower == search_term_lower
                                    || name_lower.contains(&search_term_lower)
                                    || strsim::jaro_winkler(&name_lower, &search_term_lower) >= 0.7;

                                if is_match {
                                    existing_names.insert(display_name.to_lowercase());
                                    results.push(BridgeRoom {
                                        room_id: String::new(),
                                        display_name,
                                        last_activity: 0,
                                        last_activity_formatted: "Contact".to_string(),
                                        is_group: false,
                                    });
                                }
                            }
                        }
                        tracing::info!(
                            "Total results after admin search: {} for {}",
                            results.len(),
                            capitalize(service)
                        );
                    }
                }
                Ok(resp) => {
                    tracing::warn!("Admin API returned status {}", resp.status());
                }
                Err(e) => {
                    tracing::warn!("Admin API search failed: {:?}", e);
                }
            }
        }
    }

    Ok(results)
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
