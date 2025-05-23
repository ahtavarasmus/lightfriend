use axum::{
    extract::State,
    http::StatusCode,
    response::Json as AxumJson,
};
use matrix_sdk::{
    Client as MatrixClient,
    config::SyncSettings as MatrixSyncSettings,
    ruma::{
        api::client::room::create_room::v3::Request as CreateRoomRequest,
        events::room::message::{RoomMessageEventContent, SyncRoomMessageEvent, MessageType},
        events::AnySyncTimelineEvent,
        OwnedRoomId, OwnedUserId,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use anyhow::{anyhow, Result};
use tokio::time::{sleep, Duration};
use regex::Regex;
use crate::{
    AppState,
    handlers::auth_middleware::AuthUser,
    models::user_models::{NewBridge},
    utils::matrix_auth,
};

#[derive(Serialize)]
pub struct TelegramConnectionResponse {
    login_link: String,
}

async fn connect_telegram(
    client: &MatrixClient,
    bridge_bot: &str,
) -> Result<(OwnedRoomId, String)> {
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;
    let request = CreateRoomRequest::new();
    let response = client.create_room(request).await?;
    let room_id = response.room_id();

    println!("🏠 Created room with ID: {}", room_id);
    let room = client.get_room(&room_id).ok_or(anyhow!("Room not found"))?;
    println!("🤖 Inviting bot user: {}", bot_user_id);
    room.invite_user_by_id(&bot_user_id).await?;
    client.sync_once(MatrixSyncSettings::default()).await?;

    // Wait for bot to join
    for _ in 0..30 {
        let members = room.members(matrix_sdk::RoomMemberships::JOIN).await?;
        if members.iter().any(|m| m.user_id() == bot_user_id) {
            println!("✅ Bot has joined the room");
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }
    if !room.members(matrix_sdk::RoomMemberships::empty()).await?.iter().any(|m| m.user_id() == bot_user_id) {
        return Err(anyhow!("Bot {} failed to join room", bot_user_id));
    }

    // Send login command
    println!("📤 Sending Telegram login command");
    room.send(RoomMessageEventContent::text_plain("login")).await?;

    // Wait for bot response with web login link
    let mut login_link = None;
    client.sync_once(MatrixSyncSettings::default()).await?;
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(5));

    for attempt in 1..=60 {
        println!("📡 Sync attempt #{}", attempt);
        client.sync_once(sync_settings.clone()).await?;
        if let Some(room) = client.get_room(&room_id) {
            let options = matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            let messages = room.messages(options).await?;
            for msg in messages.chunk {
                let raw_event = msg.raw();
                if let Ok(event) = raw_event.deserialize() {
                    if event.sender() == bot_user_id {
                        if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(sync_event)
                        ) = event.clone() {
                            let event_content: RoomMessageEventContent = match sync_event {
                                SyncRoomMessageEvent::Original(original_event) => original_event.content,
                                SyncRoomMessageEvent::Redacted(_) => continue,
                            };
                            println!("event_content: {:#?}", event_content);
                            if let MessageType::Notice(text_content) = event_content.msgtype {
                                if let Some(url) = extract_url(&text_content.body) {
                                    login_link = Some(url);
                                    println!("🔗 Found web login link");
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        if login_link.is_some() {
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }

    let login_link = login_link.ok_or(anyhow!("Web login link not received"))?;
    Ok((room_id.into(), login_link))
}

fn extract_url(text: &str) -> Option<String> {
    let re = Regex::new(r"https?://\S+").unwrap();
    re.find(text).map(|m| m.as_str().to_string())
}

pub async fn get_telegram_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    println!("📊 Checking Telegram status for user {}", auth_user.user_id);
    let bridge = state.user_repository.get_telegram_bridge(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get Telegram bridge status: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Telegram status"})),
            )
        })?;

    match bridge {
        Some(bridge) => Ok(AxumJson(json!({
            "connected": bridge.status == "connected",
            "status": bridge.status,
            "created_at": bridge.created_at.unwrap_or(0),
        }))),
        None => Ok(AxumJson(json!({
            "connected": false,
            "status": "not_connected",
            "created_at": 0,
        }))),
    }
}

pub async fn disconnect_telegram(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    println!("🔌 Starting Telegram disconnection process for user {}", auth_user.user_id);

    // Get the bridge information first
    let bridge = state.user_repository.get_telegram_bridge(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get Telegram bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Telegram bridge info"})),
            )
        })?;

    let Some(bridge) = bridge else {
        return Ok(AxumJson(json!({
            "message": "Telegram was not connected"
        })));
    };

    // Get Matrix client
    let client = matrix_auth::get_client(auth_user.user_id, &state.user_repository, false)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get Matrix client: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to initialize Matrix client: {}", e)})),
            )
        })?;

    // Get the room
    let room_id = OwnedRoomId::try_from(bridge.room_id.unwrap_or_default())
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Invalid room ID format"})),
        ))?;

    if let Some(room) = client.get_room(&room_id) {
        println!("📤 Sending Telegram logout command");
        // Send logout command
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("logout")).await {
            tracing::error!("Failed to send logout command: {}", e);
        }

        // Wait a moment for the logout to process
        sleep(Duration::from_secs(2)).await;
    }

    // Delete the bridge record
    state.user_repository.delete_telegram_bridge(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to delete Telegram bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to delete bridge record"})),
            )
        })?;

    println!("✅ Telegram disconnection completed for user {}", auth_user.user_id);
    Ok(AxumJson(json!({
        "message": "Telegram disconnected successfully"
    })))
}

pub async fn start_telegram_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<TelegramConnectionResponse>, (StatusCode, AxumJson<serde_json::Value>)> {
    println!("🚀 Starting Telegram connection process for user {}", auth_user.user_id);

    let mut client = matrix_auth::get_client(auth_user.user_id, &state.user_repository, true)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get Matrix client: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to initialize Matrix client: {}", e)})),
            )
        })?;

    let bridge_bot = std::env::var("TELEGRAM_BRIDGE_BOT")
        .expect("TELEGRAM_BRIDGE_BOT not set");

    let (room_id, login_link) = connect_telegram(&client, &bridge_bot)
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to Telegram bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to connect to Telegram bridge: {}", e)})),
            )
        })?;

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let new_bridge = NewBridge {
        user_id: auth_user.user_id,
        bridge_type: "telegram".to_string(),
        status: "connecting".to_string(),
        room_id: Some(room_id.to_string()),
        data: None,
        created_at: Some(current_time),
    };

    state.user_repository.create_bridge(new_bridge)
        .map_err(|e| {
            tracing::error!("Failed to store bridge information: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to store bridge information"})),
            )
        })?;

    let state_clone = state.clone();
    let room_id_clone = room_id.clone();
    let bridge_bot_clone = bridge_bot.to_string();
    let client_clone = client.clone();

    tokio::spawn(async move {
        if let Err(e) = monitor_telegram_connection(
            &client_clone,
            &room_id_clone,
            &bridge_bot_clone,
            auth_user.user_id,
            state_clone,
        ).await {
            tracing::error!("Telegram connection monitoring failed for user {}: {}", auth_user.user_id, e);
        }
    });

    Ok(AxumJson(TelegramConnectionResponse { login_link }))
}

async fn monitor_telegram_connection(
    client: &MatrixClient,
    room_id: &OwnedRoomId,
    bridge_bot: &str,
    user_id: i32,
    state: Arc<AppState>,
) -> Result<(), anyhow::Error> {
    println!("👀 Starting Telegram connection monitoring for user {} in room {}", user_id, room_id);
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(30));

    for attempt in 1..120 {
        println!("🔄 Monitoring attempt #{} for user {}", attempt, user_id);
        client.sync_once(sync_settings.clone()).await?;
        if let Some(room) = client.get_room(room_id) {
            let options = matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            let messages = room.messages(options).await?;
            for msg in messages.chunk {
                let raw_event = msg.raw();
                if let Ok(event) = raw_event.deserialize() {
                    if event.sender() == bot_user_id {
                        if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(sync_event)
                        ) = event {
                            let event_content: RoomMessageEventContent = match sync_event {
                                SyncRoomMessageEvent::Original(original_event) => original_event.content,
                                SyncRoomMessageEvent::Redacted(_) => continue,
                            };
                            if let MessageType::Notice(text_content) = event_content.msgtype {
                                println!("📱 Telegram bot message: {}", text_content.body);
                                if text_content.body.contains("logged in") || text_content.body.contains("login successful") {
                                    println!("🎉 Telegram successfully connected for user {}", user_id);
                                    let current_time = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs() as i32;
                                    let new_bridge = NewBridge {
                                        user_id,
                                        bridge_type: "telegram".to_string(),
                                        status: "connected".to_string(),
                                        room_id: Some(room_id.to_string()),
                                        data: None,
                                        created_at: Some(current_time),
                                    };
                                    state.user_repository.delete_telegram_bridge(user_id)?;
                                    state.user_repository.create_bridge(new_bridge)?;
                                    return Ok(());
                                }
                                let error_patterns = [
                                    "error",
                                    "failed",
                                    "timeout",
                                    "disconnected",
                                    "invalid code",
                                    "connection lost",
                                    "authentication failed"
                                ];
                                if error_patterns.iter().any(|&pattern| text_content.body.to_lowercase().contains(pattern)) {
                                    println!("❌ Telegram connection failed for user {}", user_id);
                                    state.user_repository.delete_telegram_bridge(user_id)?;
                                    return Err(anyhow!("Telegram connection failed: {}", text_content.body));
                                }
                            }
                        }
                    }
                }
            }
        }
        sleep(Duration::from_secs(5)).await;
    }
    state.user_repository.delete_telegram_bridge(user_id)?;
    Err(anyhow!("Telegram connection timed out"))
}
