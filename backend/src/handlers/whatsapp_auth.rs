use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as AxumJson,
};
use matrix_sdk::{
    Client as MatrixClient,
    config::SyncSettings as MatrixSyncSettings,
    room::Room,
    ruma::{
        api::client::room::create_room::v3::Request as CreateRoomRequest,
        events::room::message::{RoomMessageEventContent, SyncRoomMessageEvent, MessageType},
        events::AnySyncTimelineEvent,
        OwnedRoomId, OwnedUserId, OwnedDeviceId,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use anyhow::{anyhow, Result};
use reqwest;
use base64;
use tokio::time::{sleep, Duration};
use crate::{
    AppState,
    handlers::auth_middleware::AuthUser,
    models::user_models::{NewBridge, Bridge},
    utils::matrix_auth::MatrixAuth,
};
use url::Url;

#[derive(Serialize)]
pub struct WhatsappConnectionResponse {
    pairing_code: String, 
}


async fn ensure_matrix_credentials(
    state: &AppState,
    user_id: i32,
) -> Result<(String, String, String, String), (StatusCode, AxumJson<serde_json::Value>)> {
    let homeserver_url = std::env::var("MATRIX_HOMESERVER")
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "MATRIX_HOMESERVER not set"})),
        ))?;
    // Parse the homeserver URL to extract the domain
    let parsed_url = Url::parse(&homeserver_url)
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Invalid MATRIX_HOMESERVER format"})),
        ))?;
    let domain = parsed_url.host_str()
        .ok_or_else(|| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "No host in MATRIX_HOMESERVER"})),
        ))?;

    // Check if user already has Matrix credentials
    if let Ok(Some((username, access_token, device_id))) = state.user_repository.get_matrix_credentials(user_id) {
        let full_user_id = format!("@{}:{}", username, domain);
        return Ok((username, access_token, full_user_id, device_id));
    }

    // Create new Matrix credentials
    let matrix_auth = MatrixAuth::new(
        homeserver_url.clone(),
        std::env::var("MATRIX_SHARED_SECRET").map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "MATRIX_SHARED_SECRET not set"})),
            )
        })?,
    );

    let (username, access_token, device_id) = matrix_auth.register_user().await
        .map_err(|e| {
            tracing::error!("Failed to register Matrix user: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to create Matrix credentials"})),
            )
        })?;
    let full_user_id = format!("@{}:{}", username, domain);

    let client = MatrixClient::builder()
        .homeserver_url(&homeserver_url)
        .build()
        .await
        .map_err(|e| {
            tracing::error!("Failed to build Matrix client: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to initialize Matrix client"})),
            )
        })?;
    client.restore_session(matrix_sdk::AuthSession::Matrix(matrix_sdk::authentication::matrix::MatrixSession {
        meta: matrix_sdk::SessionMeta {
            user_id: OwnedUserId::try_from(full_user_id.clone()).map_err(|e| {
                tracing::error!("Invalid user_id format: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": "Invalid user_id format"})),
                )
            })?,
            device_id: OwnedDeviceId::try_from(device_id.clone()).map_err(|e| {
                tracing::error!("Invalid device_id format: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": "Invalid device_id format"})),
                )
            })?,
        },
        tokens: matrix_sdk::authentication::matrix::MatrixSessionTokens {
            access_token: access_token.clone(),
            refresh_token: None,
        },
    }))
    .await
    .map_err(|e| {
        tracing::error!("Failed to restore session: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Failed to restore Matrix session"})),
        )
    })?;

    state.user_repository.set_matrix_credentials(user_id, &username, &access_token, &device_id)
        .map_err(|e| {
            tracing::error!("Failed to store Matrix credentials: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to store Matrix credentials"})),
            )
        })?;

    Ok((username, access_token, full_user_id, device_id))
}

async fn connect_whatsapp(
    client: &MatrixClient,
    bridge_bot: &str,
    access_token: &str,
    phone_number: &str, // Add phone number parameter
) -> Result<(OwnedRoomId, String)> {
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;
    let request = CreateRoomRequest::new();
    let response = client.create_room(request).await?;
    let room_id = response.room_id();

    println!("🏠 Created room with ID: {}", room_id);
    let room = client.get_room(&room_id).ok_or(anyhow!("Room not found"))?;
    println!("🤖 Inviting bot user: {}", bot_user_id);
    room.invite_user_by_id(&bot_user_id).await?;
    println!("🤖 Waiting for bot to join...");
    for _ in 0..5 {
        let members = room.members(matrix_sdk::RoomMemberships::empty()).await?;
        if members.iter().any(|m| m.user_id() == bot_user_id) {
            println!("✅ Bot has joined the room");
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }
    if !room.members(matrix_sdk::RoomMemberships::empty()).await?.iter().any(|m| m.user_id() == bot_user_id) {
        return Err(anyhow!("Bot {} failed to join room", bot_user_id));
    }

    // Send login command with phone number
    let login_command = format!("!wa login phone {}", phone_number);
    println!("📤 Sending WhatsApp login command: {}", login_command);
    room.send(RoomMessageEventContent::text_plain(&login_command)).await?;

    // Wait for bot response with pairing code
    let mut pairing_code = None;
    println!("⏳ Starting pairing code monitoring");
    client.sync_once(MatrixSyncSettings::default()).await?;

    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(1));

    println!("🔄 Starting message polling loop");
    for attempt in 1..=60 {
        println!("📡 Sync attempt #{}", attempt);
        client.sync_once(sync_settings.clone()).await?;
        
        sleep(Duration::from_millis(500)).await;
        
        if let Some(room) = client.get_room(&room_id) {
            println!("🏠 Found room, fetching messages");
            let options = matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            let messages = room.messages(options).await?;
            println!("📨 Fetched {} messages", messages.chunk.len());
            for msg in messages.chunk {
                let raw_event = msg.raw();
                if let Ok(event) = raw_event.deserialize() {
                    println!("📝 Processing event from sender: {}", event.sender());
                    if event.sender() == bot_user_id {
                        if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(sync_event)
                        ) = event.clone() {
                            let event_content: RoomMessageEventContent = match sync_event {
                                SyncRoomMessageEvent::Original(original_event) => original_event.content,
                                SyncRoomMessageEvent::Redacted(_) => continue,
                            };

                            println!("📨 Processing message event: {:?}", event_content);
                            if let MessageType::Notice(text_content) = event_content.msgtype {
                                println!("📝 Text message from {}: {}", event.sender(), text_content.body);
                                // Check for pairing code in the message (e.g., "FQWG-FHKC")
                                if !text_content.body.contains("Input the pairing code") {
                                    // Extract the pairing code (assumes format like "FQWG-FHKC")
                                    let parts: Vec<&str> = text_content.body.split_whitespace().collect();
                                    if let Some(code) = parts.last() {
                                        if code.contains('-') { // Basic validation for code format
                                            pairing_code = Some(code.to_string());
                                            println!("🔑 Found pairing code: {}", code);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if pairing_code.is_some() {
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }

    let pairing_code = pairing_code.ok_or(anyhow!("Pairing code not received"))?;
    Ok((room_id.into(), pairing_code))
}

pub async fn start_whatsapp_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<WhatsappConnectionResponse>, (StatusCode, AxumJson<serde_json::Value>)> {
    println!("🚀 Starting WhatsApp connection process for user {}", auth_user.user_id);
    tracing::info!("Starting WhatsApp connection for user {}", auth_user.user_id);

    // Fetch user's phone number
    let phone_number = state
        .user_repository
        .find_by_id(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to fetch phone number: {}", e);
            (
                StatusCode::BAD_REQUEST,
                AxumJson(json!({"error": "Phone number not found"})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                AxumJson(json!({"error": "Phone number not set"})),
            )
        })?.phone_number;

    println!("📝 Ensuring Matrix credentials...");
    // TODO remove before prod
    // Ensure user has Matrix credentials
    let (username, access_token, full_user_id, device_id) = ensure_matrix_credentials(&state, auth_user.user_id).await?;
    println!("✅ Matrix credentials obtained for user: {}", username);

    // Create Matrix client
    let homeserver_url = std::env::var("MATRIX_HOMESERVER")
        .expect("MATRIX_HOMESERVER not set");
    let bridge_bot = std::env::var("WHATSAPP_BRIDGE_BOT")
        .expect("WHATSAPP_BRIDGE_BOT not set");

    let client = MatrixClient::builder()
        .homeserver_url(&homeserver_url)
        .build()
        .await
        .map_err(|e| {
            tracing::error!("Failed to build Matrix client: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to initialize Matrix client"})),
            )
        })?;

    client.restore_session(matrix_sdk::AuthSession::Matrix(matrix_sdk::authentication::matrix::MatrixSession {
        meta: matrix_sdk::SessionMeta {
            user_id: OwnedUserId::try_from(full_user_id).map_err(|e| {
                tracing::error!("Invalid user_id format: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": "Invalid user_id format"})),
                )
            })?,
            device_id: OwnedDeviceId::try_from(device_id).map_err(|e| {
                tracing::error!("Invalid device_id format: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": "Invalid device_id format"})),
                )
            })?,
        },
        tokens: matrix_sdk::authentication::matrix::MatrixSessionTokens {
            access_token: access_token.clone(),
            refresh_token: None,
        },
    }))
    .await
    .map_err(|e| {
        tracing::error!("Failed to restore session: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Failed to restore session"})),
        )
    })?;


    println!("🔗 Connecting to WhatsApp bridge...");
    // Connect to WhatsApp bridge
    let (room_id, pairing_code) = connect_whatsapp(&client, &bridge_bot, &access_token, &phone_number)
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to WhatsApp bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to connect to WhatsApp bridge"})),
            )
        })?;

    // Debug: Log the pairing code
    println!("Generated pairing code: {}", &pairing_code);
    tracing::info!("Generated pairing code: {}", &pairing_code);

    // Create bridge record
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let new_bridge = NewBridge {
        user_id: auth_user.user_id,
        bridge_type: "whatsapp".to_string(),
        status: "connecting".to_string(),
        room_id: Some(room_id.to_string()),
        data: None,
        created_at: Some(current_time),
    };

    // Store bridge information
    state.user_repository.create_bridge(new_bridge)
        .map_err(|e| {
            tracing::error!("Failed to store bridge information: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to store bridge information"})),
            )
        })?;

    // Spawn a task to monitor the connection status
    let state_clone = state.clone();
    let room_id_clone = room_id.clone();
    let bridge_bot_clone = bridge_bot.to_string();
    let client_clone = client.clone();
    
    tokio::spawn(async move {
        match monitor_whatsapp_connection(
            &client_clone,
            &room_id_clone,
            &bridge_bot_clone,
            auth_user.user_id,
            state_clone,
        ).await {
            Ok(_) => {
                tracing::info!("WhatsApp connection monitoring completed successfully for user {}", auth_user.user_id);
            },
            Err(e) => {
                tracing::error!("WhatsApp connection monitoring failed for user {}: {}", auth_user.user_id, e);
            }
        }
    });

    Ok(AxumJson(WhatsappConnectionResponse { pairing_code }))
}

pub async fn get_whatsapp_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    println!("📊 Checking WhatsApp status for user {}", auth_user.user_id);
    let bridge = state.user_repository.get_whatsapp_bridge(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get WhatsApp bridge status: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get WhatsApp status"})),
            )
        })?;

    match bridge {
        Some(bridge) => Ok(AxumJson(json!({
            "connected": bridge.status == "connected",
            "status": bridge.status,
        }))),
        None => Ok(AxumJson(json!({
            "connected": false,
            "status": "not_connected",
        }))),
    }
}
                        

async fn monitor_whatsapp_connection(
    client: &MatrixClient,
    room_id: &OwnedRoomId,
    bridge_bot: &str,
    user_id: i32,
    state: Arc<AppState>,
) -> Result<(), anyhow::Error> {
    println!("👀 Starting WhatsApp connection monitoring for user {} in room {}", user_id, room_id);
    println!("🤖 Monitoring messages from bridge bot: {}", bridge_bot);
    println!("👀 Starting WhatsApp connection monitoring for user {}", user_id);
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;

    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(30));

    // Increase monitoring duration and frequency
    for attempt in 1..40 { // Try for about 20 minutes (40 * 30 seconds)
        println!("🔄 Monitoring attempt #{} for user {}", attempt, user_id);
        client.sync_once(sync_settings.clone()).await?;
        
        println!("🔍 Checking messages in room {}", room_id);
        if let Some(room) = client.get_room(room_id) {
            println!("📬 Found room, fetching messages...");
            let options = matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            let messages = room.messages(options).await?;
            for msg in messages.chunk {
                let raw_event = msg.raw();
                if let Ok(event) = raw_event.deserialize() {
                    if event.sender() == bot_user_id {
                        if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(sync_event)
                        ) = event {
                            // Access the content field from SyncMessageLikeEvent
                            let event_content: RoomMessageEventContent = match sync_event {
                                SyncRoomMessageEvent::Original(original_event) => original_event.content,
                                SyncRoomMessageEvent::Redacted(_) => continue,
                            };

                            let content = match event_content.msgtype {
                                MessageType::Text(text_content) => text_content.body,
                                MessageType::Notice(notice_content) => notice_content.body,
                                _ => continue,
                            };
        
                            // Check for successful connection messages
                            // Debug log the message content
                            println!("📱 WhatsApp bot message: {}", content);

                            println!("message contains Successfully logged in as: {}",content.contains("Successfully logged in as"));
                            // Check for successful login message first
                            if content.contains("Successfully logged in as") {
                                println!("🎉 WhatsApp successfully connected for user {} with phone number confirmation", user_id);
                                // Update bridge status to connected
                                let current_time = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs() as i32;

                                let new_bridge = NewBridge {
                                    user_id,
                                    bridge_type: "whatsapp".to_string(),
                                    status: "connected".to_string(),
                                    room_id: Some(room_id.to_string()),
                                    data: None,
                                    created_at: Some(current_time),
                                };

                                state.user_repository.delete_whatsapp_bridge(user_id)?;
                                state.user_repository.create_bridge(new_bridge)?;
                                return Ok(());
                            }
                            // Check for other success messages as fallback
                            else if content.contains("successfully logged in") || 
                               content.contains("connected") ||
                               content.contains("WhatsApp connection successful") ||
                               content.contains("Successfully connected to WhatsApp") ||
                               content.contains("WhatsApp Web is running") {
                                println!("🎉 WhatsApp successfully connected for user {}", user_id);
                                // Update bridge status to connected
                                let current_time = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs() as i32;

                                let new_bridge = NewBridge {
                                    user_id,
                                    bridge_type: "whatsapp".to_string(),
                                    status: "connected".to_string(),
                                    room_id: Some(room_id.to_string()),
                                    data: None,
                                    created_at: Some(current_time),
                                };

                                state.user_repository.delete_whatsapp_bridge(user_id)?;
                                state.user_repository.create_bridge(new_bridge)?;
                                return Ok(());
                            }

                            // Check for various error messages
                            let error_patterns = [
                                "error",
                                "failed",
                                "timeout",
                                "disconnected",
                                "invalid code",
                                "connection lost",
                                "authentication failed"
                            ];

                            if error_patterns.iter().any(|&pattern| content.to_lowercase().contains(pattern)) {
                                println!("❌ WhatsApp connection failed for user {}", user_id);
                                println!("📄 Error message: {}", content);
                                state.user_repository.delete_whatsapp_bridge(user_id)?;
                                return Err(anyhow!("WhatsApp connection failed: {}", content));
                            }
                        }
                    }
                }
            }
        }

        sleep(Duration::from_secs(30)).await;
    }

    // If we reach here, connection timed out
    state.user_repository.delete_whatsapp_bridge(user_id)?;
    Err(anyhow!("WhatsApp connection timed out"))
}

pub async fn disconnect_whatsapp(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    println!("🔌 Disconnecting WhatsApp for user {}", auth_user.user_id);
    // Delete the bridge record
    state.user_repository.delete_whatsapp_bridge(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to delete WhatsApp bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to disconnect WhatsApp"})),
            )
        })?;

    Ok(AxumJson(json!({
        "message": "WhatsApp disconnected successfully"
    })))
}

