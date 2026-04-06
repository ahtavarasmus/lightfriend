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

fn room_name_matches_service(room_name: &str, service: &str) -> bool {
    let room_name = room_name.trim().to_lowercase();
    match service {
        "whatsapp" => room_name.contains("(wa)") || room_name.contains("(wa~)"),
        "telegram" => room_name.contains("(telegram)") || room_name.contains("(tg)"),
        "signal" => room_name.contains("(signal)") || room_name.contains("(signal~)"),
        _ => false,
    }
}

fn has_service_member(service: &str, member_localparts: &[String]) -> bool {
    let sender_prefix = get_sender_prefix(service);
    member_localparts
        .iter()
        .any(|localpart| localpart.starts_with(&sender_prefix))
}

fn is_group_room(service: &str, member_localparts: &[String]) -> bool {
    let sender_prefix = get_sender_prefix(service);
    member_localparts
        .iter()
        .filter(|localpart| localpart.starts_with(&sender_prefix))
        .count()
        > 1
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

/// When `infer_service()` fails (sender is user, not a bridge ghost),
/// scan room members for ghost bot localparts to determine platform.
async fn infer_service_from_room_members(room: &Room) -> Option<String> {
    let members = room.members(matrix_sdk::RoomMemberships::JOIN).await.ok()?;
    for member in &members {
        let localpart = member.user_id().localpart();
        if localpart.starts_with("whatsapp_") {
            return Some("whatsapp".to_string());
        }
        if localpart.starts_with("telegram_") {
            return Some("telegram".to_string());
        }
        if localpart.starts_with("signal_") {
            return Some("signal".to_string());
        }
    }
    None
}

pub async fn get_service_rooms(client: &MatrixClient, service: &str) -> Result<Vec<BridgeRoom>> {
    let joined_rooms = client.joined_rooms();
    let service_cap = capitalize(service);
    let skip_terms = [
        format!("{}bot", service),
        format!("{}-bridge", service),
        format!("{} Bridge", service_cap),
        format!("{} bridge bot", service_cap),
    ];
    let mut futures = Vec::new();
    for room in joined_rooms {
        let skip_terms = skip_terms.clone();
        let service = service.to_string();
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
            let member_localparts: Vec<String> = members
                .iter()
                .map(|member| member.user_id().localpart().to_string())
                .collect();
            if !room_name_matches_service(&display_name, &service)
                && !has_service_member(&service, &member_localparts)
            {
                return None;
            }
            let is_group = is_group_room(&service, &member_localparts);
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
            let member_localparts: Vec<String> = members
                .iter()
                .map(|member| member.localpart.clone())
                .collect();

            if !room_name_matches_service(&room_info.display_name, service)
                && !has_service_member(service, &member_localparts)
            {
                continue;
            }

            let is_group = is_group_room(service, &member_localparts);
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
        "Starting {} message fetch from Postgres - User: {}, chat: {}, limit: {}",
        capitalize(service),
        user_id,
        chat_name,
        limit.unwrap_or(20)
    );

    // Find the room_id for this chat by searching ont_channels or matching room names
    let room_id = find_room_id_for_chat(service, state, user_id, chat_name).await?;
    let user_info = state.user_core.get_user_info(user_id)?;
    let msg_limit = limit.unwrap_or(20) as i64;

    let messages = state
        .ontology_repository
        .get_messages_for_room(user_id, &room_id, msg_limit)
        .map_err(|e| anyhow!("Failed to query messages: {}", e))?;

    let room_name = if !messages.is_empty() {
        messages[0].sender_name.clone()
    } else {
        chat_name.to_string()
    };

    let bridge_messages: Vec<BridgeMessage> = messages
        .into_iter()
        .map(|m| ont_message_to_bridge_message(&m, user_info.timezone.clone()))
        .collect();

    Ok((bridge_messages, room_name))
}

/// Resolve a chat name to a room_id. Checks ontology channels and stored messages first,
/// then uses Matrix room list for name-to-room-id resolution (not for reading messages).
async fn find_room_id_for_chat(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    chat_name: &str,
) -> Result<String> {
    // Try ontology Person lookup
    if let Ok(Some(person)) = state
        .ontology_repository
        .find_person_by_name(user_id, chat_name)
    {
        if let Some(ch) = person
            .channels
            .iter()
            .find(|c| c.platform == service && c.room_id.is_some())
        {
            return Ok(ch.room_id.clone().unwrap());
        }
    }

    // Search ont_messages for a room matching sender_name
    if let Ok(candidates) = state
        .ontology_repository
        .get_recent_messages(user_id, service, 0)
    {
        let chat_lower = chat_name.to_lowercase();
        if let Some(m) = candidates
            .iter()
            .find(|m| m.sender_name.to_lowercase().contains(&chat_lower))
        {
            return Ok(m.room_id.clone());
        }
    }

    Err(anyhow!(
        "No {} messages found for '{}'. They need to send you a message first, or create them as a contact.",
        capitalize(service),
        chat_name
    ))
}

/// Convert an OntMessage to a BridgeMessage for display.
fn ont_message_to_bridge_message(
    m: &crate::models::ontology_models::OntMessage,
    timezone: Option<String>,
) -> BridgeMessage {
    BridgeMessage {
        sender: m.sender_name.clone(),
        sender_display_name: m.sender_name.clone(),
        content: m.content.clone(),
        timestamp: m.created_at as i64,
        formatted_timestamp: format_timestamp(m.created_at as i64, timezone),
        message_type: "text".to_string(),
        room_name: m.sender_name.clone(),
        media_url: None,
        room_id: Some(m.room_id.clone()),
    }
}

use matrix_sdk::notification_settings::RoomNotificationMode;

pub async fn fetch_bridge_messages(
    service: &str,
    state: &Arc<AppState>,
    user_id: i32,
    start_time: i64,
    _unread_only: bool,
) -> Result<Vec<BridgeMessage>> {
    tracing::info!(
        "Fetching {} messages from Postgres for user {}",
        service,
        user_id
    );

    let user_info = state.user_core.get_user_info(user_id)?;
    let since_ts = start_time as i32;

    let messages = state
        .ontology_repository
        .get_recent_messages(user_id, service, since_ts)
        .map_err(|e| anyhow!("Failed to query messages: {}", e))?;

    // Group by room_id, take up to 5 per room (mirroring old behavior)
    let mut by_room: std::collections::HashMap<
        String,
        Vec<&crate::models::ontology_models::OntMessage>,
    > = std::collections::HashMap::new();
    for m in &messages {
        by_room.entry(m.room_id.clone()).or_default().push(m);
    }

    let mut bridge_messages: Vec<BridgeMessage> = Vec::new();
    // Take top 5 rooms by most recent message
    let mut rooms: Vec<_> = by_room.into_iter().collect();
    rooms.sort_by(|a, b| {
        let a_ts = a.1.first().map(|m| m.created_at).unwrap_or(0);
        let b_ts = b.1.first().map(|m| m.created_at).unwrap_or(0);
        b_ts.cmp(&a_ts)
    });
    rooms.truncate(5);

    for (_room_id, room_msgs) in rooms {
        for m in room_msgs.into_iter().take(5) {
            bridge_messages.push(ont_message_to_bridge_message(m, user_info.timezone.clone()));
        }
    }

    // Sort by timestamp (most recent first)
    bridge_messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    tracing::info!("Retrieved {} messages from Postgres", bridge_messages.len());
    Ok(bridge_messages)
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
    target_room_id: Option<&str>,
) -> Result<BridgeMessage> {
    tracing::info!(
        "SEND_FLOW_BRIDGE send_bridge_message ENTER: service={}, user={}, chat_name='{}', room_id={:?}, has_media={}",
        service, user_id, chat_name, target_room_id, media_url.is_some()
    );

    tracing::info!(
        "SEND_FLOW_BRIDGE Getting cached Matrix client for user={}",
        user_id
    );
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
    tracing::info!("SEND_FLOW_BRIDGE Got Matrix client OK");

    let bridge = state.user_repository.get_bridge(user_id, service)?;
    tracing::info!(
        "SEND_FLOW_BRIDGE Bridge status: found={}, status={:?}",
        bridge.is_some(),
        bridge.as_ref().map(|b| b.status.clone())
    );
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        tracing::error!("SEND_FLOW_BRIDGE Bridge not connected, aborting");
        return Err(anyhow!(
            "{} bridge is not connected. Please log in first.",
            capitalize(service)
        ));
    }

    // If we have a room_id from the initial lookup, use it directly (no re-search)
    let room = if let Some(rid) = target_room_id {
        tracing::info!("SEND_FLOW_BRIDGE Using target_room_id directly: {}", rid);
        let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(rid).map_err(|e| {
            tracing::error!("SEND_FLOW_BRIDGE Invalid room ID '{}': {}", rid, e);
            anyhow!("Invalid room ID '{}': {}", rid, e)
        })?;
        let r = client.get_room(&room_id);
        if r.is_none() {
            // Room not in cache - sync and retry (happens after deploy when client cache is cold)
            tracing::info!(
                "SEND_FLOW_BRIDGE Room not in cache, running sync_once to populate rooms..."
            );
            if let Err(e) = client
                .sync_once(matrix_sdk::config::SyncSettings::new())
                .await
            {
                tracing::error!("SEND_FLOW_BRIDGE sync_once failed: {}", e);
            } else {
                tracing::info!("SEND_FLOW_BRIDGE sync_once completed");
            }
        }
        let r = client.get_room(&room_id);
        tracing::info!(
            "SEND_FLOW_BRIDGE client.get_room({}) returned: found={}",
            rid,
            r.is_some()
        );
        r.ok_or_else(|| {
            tracing::error!(
                "SEND_FLOW_BRIDGE Room {} not found in Matrix client even after sync!",
                rid
            );
            anyhow!("Room {} not found in client", rid)
        })?
    } else {
        tracing::info!(
            "SEND_FLOW_BRIDGE No target_room_id, searching by name '{}'",
            chat_name
        );
        let service_rooms = get_service_rooms(&client, service).await?;
        tracing::info!("SEND_FLOW_BRIDGE Got {} service rooms", service_rooms.len());
        let exact_room = find_exact_room(&service_rooms, chat_name);
        tracing::info!(
            "SEND_FLOW_BRIDGE find_exact_room result: found={}",
            exact_room.is_some()
        );
        match exact_room {
            Some(room_info) => {
                tracing::info!(
                    "SEND_FLOW_BRIDGE Exact room: id={}, name={}",
                    room_info.room_id,
                    room_info.display_name
                );
                let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_info.room_id.as_str())
                    .map_err(|e| anyhow!("Invalid room ID: {}", e))?;
                client
                    .get_room(&room_id)
                    .ok_or_else(|| anyhow!("Room not found"))?
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
                tracing::error!("SEND_FLOW_BRIDGE No room found: {}", error_msg);
                return Err(anyhow!(error_msg));
            }
        }
    };
    tracing::info!(
        "SEND_FLOW_BRIDGE Got Matrix room object: room_id={}, display_name will be fetched after send",
        room.room_id()
    );
    use matrix_sdk::ruma::events::room::message::{
        ImageMessageEventContent, MessageType, RoomMessageEventContent,
    };
    if let Some(url) = media_url {
        tracing::info!(
            "SEND_FLOW_BRIDGE Sending IMAGE message with caption, downloading from URL..."
        );
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
        tracing::info!(
            "SEND_FLOW_BRIDGE Downloaded image: {} bytes, mime={}",
            size,
            mime
        );
        // ── 2. Upload to the homeserver ──────────────────────────────────────────
        let upload_resp = client.media().upload(&mime, bytes.to_vec(), None).await?;
        let mxc: matrix_sdk::ruma::OwnedMxcUri = upload_resp.content_uri;
        tracing::info!("SEND_FLOW_BRIDGE Uploaded to homeserver: mxc={}", mxc);
        // ── 4. Build the image-message content with caption in *one* event ──────
        let mut img = ImageMessageEventContent::plain(
            message.to_owned(), // the caption / body
            mxc,
        );
        // Optional but nice: add basic metadata so bridges & clients know the size
        let mut imageinfo = matrix_sdk::ruma::events::room::ImageInfo::new();
        imageinfo.size = Some(matrix_sdk::ruma::UInt::new(size as u64).unwrap_or_default());
        img.info = Some(Box::new(imageinfo));
        // Wrap it as a generic "m.room.message"
        let content = RoomMessageEventContent::new(MessageType::Image(img));
        // ── 5. Send it ───────────────────────────────────────────────────────────
        let rid_str = room.room_id().to_string();
        tracing::info!(
            "SEND_FLOW_BRIDGE Calling room.send for image message, room_id={}",
            rid_str
        );
        room.send(content).await?;
        tracing::info!(
            "SEND_FLOW_BRIDGE room.send for image returned OK, room_id={}",
            rid_str
        );
    } else {
        // plain text
        let rid_str = room.room_id().to_string();
        tracing::info!(
            "SEND_FLOW_BRIDGE Sending plain text message to room_id={}",
            rid_str
        );
        room.send(RoomMessageEventContent::text_plain(message))
            .await?;
        tracing::info!(
            "SEND_FLOW_BRIDGE room.send for text returned OK, room_id={}",
            rid_str
        );
    }
    let rid_str = room.room_id().to_string();
    tracing::info!(
        "SEND_FLOW_BRIDGE Message sent via {}, building response, room_id={}",
        service,
        rid_str
    );
    let user_info = state.user_core.get_user_info(user_id)?;
    let current_timestamp = chrono::Utc::now().timestamp();
    let display_name = room.display_name().await?.to_string();
    tracing::info!(
        "SEND_FLOW_BRIDGE send_bridge_message COMPLETE: room_id={}, display_name={}, service={}",
        rid_str,
        display_name,
        service
    );
    // Return the sent message details
    Ok(BridgeMessage {
        sender: "You".to_string(),
        sender_display_name: "You".to_string(),
        content: message.to_string(),
        timestamp: current_timestamp,
        formatted_timestamp: format_timestamp(current_timestamp, user_info.timezone),
        message_type: "text".to_string(),
        room_name: display_name,
        media_url: None,
        room_id: Some(room.room_id().to_string()),
    })
}

use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;
use matrix_sdk::RoomMemberships;
use strsim;

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
    let connecting_bridge_types = vec!["signal", "telegram", "whatsapp"];
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

            // Record disconnection event
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

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
            // Not a ghost sender - try room members (user's own message in bridge room)
            match infer_service_from_room_members(&room).await {
                Some(s) => s,
                None => {
                    tracing::debug!("Could not infer service, skipping");
                    return;
                }
            }
        }
    };

    // Check if sender is a bridge ghost or the user themselves
    let sender_prefix = get_sender_prefix(&service);
    if !sender_localpart.starts_with(&sender_prefix) {
        // User's own outgoing message - store in ontology for context, skip AI processing
        let content = match &event.content.msgtype {
            MessageType::Text(t) => t.body.clone(),
            MessageType::Notice(n) => n.body.clone(),
            MessageType::Emote(t) => t.body.clone(),
            _ => return, // Skip user media (no useful text for LLM)
        };
        if is_error_message(&content) {
            return;
        }
        let current_room_id = room.room_id().to_string();
        let is_group = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
            Ok(members) => {
                let localparts: Vec<String> = members
                    .iter()
                    .map(|m| m.user_id().localpart().to_string())
                    .collect();
                is_group_room(&service, &localparts)
            }
            Err(_) => false,
        };
        let msg = crate::models::ontology_models::NewOntMessage {
            user_id,
            room_id: current_room_id.clone(),
            platform: service.clone(),
            sender_name: "You".to_string(),
            content: content.clone(),
            person_id: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i32,
        };
        let state_clone = state.clone();
        tokio::spawn(async move {
            match state_clone.ontology_repository.insert_message(&msg) {
                Ok(created) => {
                    let snapshot = serde_json::json!({
                        "message_id": created.id,
                        "platform": msg.platform,
                        "sender_name": "You",
                        "content": msg.content,
                        "room_id": msg.room_id,
                        "is_group": is_group,
                        "is_outgoing": true,
                    });
                    crate::proactive::rules::emit_ontology_change(
                        &state_clone,
                        user_id,
                        "Message",
                        created.id as i32,
                        "created",
                        snapshot,
                    )
                    .await;

                    // Auto-resolve: user replied, so clear pending digests and resolve urgency
                    let now = msg.created_at;
                    if let Err(e) = state_clone.ontology_repository.mark_room_digest_delivered(
                        user_id,
                        &current_room_id,
                        now,
                    ) {
                        tracing::warn!("Failed to mark room digest delivered: {}", e);
                    }
                    if let Err(e) = state_clone
                        .ontology_repository
                        .resolve_high_urgency_for_room(user_id, &current_room_id, now)
                    {
                        tracing::warn!("Failed to resolve high urgency for room: {}", e);
                    }

                    // Check if outgoing message completes any tracked events
                    crate::proactive::system_behaviors::check_outgoing_event_resolution(
                        &state_clone,
                        user_id,
                        &current_room_id,
                        &msg.content,
                        created.id,
                    )
                    .await;
                }
                Err(e) => tracing::warn!("Failed to store user message: {}", e),
            }
        });
        return;
    }

    // Check subscription
    let has_valid_sub = state
        .user_repository
        .has_valid_subscription_tier(user_id, "tier 2")
        .unwrap_or(false);
    if !has_valid_sub {
        tracing::debug!(
            "User {} does not have valid subscription, skipping",
            user_id
        );
        return;
    }
    let user_plan = state.user_repository.get_plan_type(user_id).unwrap_or(None);
    if !crate::utils::plan_features::has_auto_features(user_plan.as_deref()) {
        tracing::debug!(
            "User {} on {:?} plan - no auto features",
            user_id,
            user_plan
        );
        return;
    }

    // Extract message content and estimate message size for bandwidth tracking
    let (content, bytes_estimate) = match event.content.msgtype {
        MessageType::Text(ref t) => (t.body.clone(), t.body.len() as i32),
        MessageType::Notice(ref n) => (n.body.clone(), n.body.len() as i32),
        MessageType::Image(ref img) => {
            let size = img
                .info
                .as_ref()
                .and_then(|i| i.size)
                .map(|s| u64::from(s).min(i32::MAX as u64) as i32)
                .unwrap_or(50_000i32);
            ("IMAGE".into(), size)
        }
        MessageType::Video(ref vid) => {
            let size = vid
                .info
                .as_ref()
                .and_then(|i| i.size)
                .map(|s| u64::from(s).min(i32::MAX as u64) as i32)
                .unwrap_or(500_000i32);
            ("VIDEO".into(), size)
        }
        MessageType::File(ref f) => {
            let size = f
                .info
                .as_ref()
                .and_then(|i| i.size)
                .map(|s| u64::from(s).min(i32::MAX as u64) as i32)
                .unwrap_or(100_000i32);
            ("FILE".into(), size)
        }
        MessageType::Audio(ref a) => {
            let size = a
                .info
                .as_ref()
                .and_then(|i| i.size)
                .map(|s| u64::from(s).min(i32::MAX as u64) as i32)
                .unwrap_or(100_000i32);
            ("AUDIO".into(), size)
        }
        MessageType::Location(_) => ("LOCATION".into(), 200i32),
        MessageType::Emote(ref t) => (t.body.clone(), t.body.len() as i32),
        _ => return,
    };

    // Log bandwidth estimate for bridge traffic tracking
    if let Err(e) =
        state
            .bandwidth_repository
            .log_bandwidth(user_id, &service, "inbound", bytes_estimate)
    {
        tracing::warn!("Failed to log bandwidth for user {}: {}", user_id, e);
    }

    // Skip error messages
    if is_error_message(&content) {
        tracing::debug!("Skipping error message");
        return;
    }

    let chat_name = remove_bridge_suffix(room_name.as_str());
    let current_room_id = room.room_id().to_string();

    // Check if this is a group chat (same heuristic as get_service_rooms)
    let is_group = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
        Ok(members) => {
            let member_localparts: Vec<String> = members
                .iter()
                .map(|member| member.user_id().localpart().to_string())
                .collect();
            is_group_room(&service, &member_localparts)
        }
        Err(_) => false,
    };

    // Ontology Person lookup (for person_id on the message)
    let matching_person = state
        .ontology_repository
        .find_person_by_room_id(user_id, &current_room_id)
        .unwrap_or_else(|e| {
            tracing::warn!("Ontology lookup failed for room {}: {}", current_room_id, e);
            None
        });

    // Auto-create Person for recognized phone contacts (WhatsApp/Signal only).
    // Telegram returns None from is_phone_contact_from_room_name - no auto-creation.
    let matching_person = if matching_person.is_none() && !is_group {
        match is_phone_contact_from_room_name(&room_name) {
            Some(true) => {
                tracing::info!(
                    "Auto-creating person '{}' on {} for user {} (phone contact)",
                    chat_name,
                    service,
                    user_id
                );
                match state.ontology_repository.upsert_person(
                    user_id,
                    &chat_name,
                    &service,
                    None,
                    Some(&current_room_id),
                ) {
                    Ok(person) => {
                        tracing::info!("Created person '{}' (id={})", person.name, person.id);
                        // Fetch the full PersonWithChannels for consistency
                        state
                            .ontology_repository
                            .get_person_with_channels(user_id, person.id)
                            .ok()
                    }
                    Err(e) => {
                        tracing::warn!("Failed to auto-create person '{}': {}", chat_name, e);
                        None
                    }
                }
            }
            _ => None, // Not a contact (Some(false)) or can't determine (None/Telegram)
        }
    } else {
        matching_person
    };

    // Store message in ont_messages + emit ontology change.
    // Rules handle all notification logic from here.
    let person_id = matching_person.as_ref().map(|p| p.person.id);
    let msg = crate::models::ontology_models::NewOntMessage {
        user_id,
        room_id: current_room_id.clone(),
        platform: service.clone(),
        sender_name: chat_name.clone(),
        content: content.clone(),
        person_id,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32,
    };
    let state_clone = state.clone();
    tokio::spawn(async move {
        match state_clone.ontology_repository.insert_message(&msg) {
            Ok(created) => {
                let snapshot = serde_json::json!({
                    "message_id": created.id,
                    "platform": msg.platform,
                    "sender_name": msg.sender_name,
                    "content": msg.content,
                    "room_id": msg.room_id,
                    "is_group": is_group,
                });
                crate::proactive::rules::emit_ontology_change(
                    &state_clone,
                    user_id,
                    "Message",
                    created.id as i32,
                    "created",
                    snapshot,
                )
                .await;
            }
            Err(e) => {
                tracing::warn!("Failed to store bridge message: {}", e);
            }
        }
    });
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

    // Search Matrix user directory for bridge ghost users (contacts without rooms).
    // Uses the standard Matrix user directory search API which, unlike Synapse,
    // may include appservice-managed users on Tuwunel/Conduit-based homeservers.
    if search_term.trim().len() >= 2 {
        let homeserver_url = std::env::var("MATRIX_HOMESERVER").unwrap_or_default();
        let access_token = client
            .matrix_auth()
            .session()
            .map(|s| s.tokens.access_token.clone())
            .unwrap_or_default();

        if !homeserver_url.is_empty() && !access_token.is_empty() {
            let search_url = format!(
                "{}/_matrix/client/v3/user_directory/search",
                homeserver_url.trim_end_matches('/')
            );
            let http_client = reqwest::Client::new();
            match http_client
                .post(&search_url)
                .header("Authorization", format!("Bearer {}", access_token))
                .json(&serde_json::json!({
                    "search_term": search_term.trim(),
                    "limit": 50
                }))
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

                        if let Some(users) = body["results"].as_array() {
                            tracing::info!(
                                "User directory returned {} users for '{}'",
                                users.len(),
                                search_term
                            );
                            for user in users {
                                let user_id = user["user_id"].as_str().unwrap_or_default();
                                // Extract localpart from @localpart:server
                                let localpart = user_id
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

                                let raw_display = user["display_name"].as_str().unwrap_or_default();
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
                            "Total results after directory search: {} for {}",
                            results.len(),
                            capitalize(service)
                        );
                    }
                }
                Ok(resp) => {
                    tracing::warn!("User directory search returned status {}", resp.status());
                }
                Err(e) => {
                    tracing::warn!("User directory search failed: {:?}", e);
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
