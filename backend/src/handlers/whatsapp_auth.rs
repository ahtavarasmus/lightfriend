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

#[derive(Serialize)]
pub struct WhatsappConnectionResponse {
    qr_code_url: String,
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

    // Check if user already has Matrix credentials
    if let Ok(Some((username, access_token, device_id))) = state.user_repository.get_matrix_credentials(user_id) {
        let full_user_id = format!("@{}:{}", username, homeserver_url.trim_start_matches("http://").trim_start_matches("https://"));
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
    let full_user_id = format!("@{}:{}", username, homeserver_url.trim_start_matches("http://").trim_start_matches("https://"));

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
    // First send help command to verify bot is responsive
    let help_command = "!wa login qr";
    println!("📤 Sending help command to verify bot responsiveness: {}", help_command);
    let help_result = room.send(RoomMessageEventContent::text_plain(help_command)).await;
    match help_result {
        Ok(event_id) => println!("✅ Help command sent successfully. Event ID: {:#?}", event_id),
        Err(e) => println!("❌ Failed to send help command: {}", e),
    };

    // Wait for bot to process help command
    println!("⏳ Waiting for bot to process help command...");
    sleep(Duration::from_secs(1)).await;

    // Send login command with correct prefix
    // Send login command and wait for response
    let login_command = "!wa login qr";
    println!("📤 Sending WhatsApp login command: {}", login_command);
    let message_result = room.send(RoomMessageEventContent::text_plain(login_command)).await;
    
    // Wait a bit longer for the bot to process the command
    println!("⏳ Waiting for bot to process login command...");
    sleep(Duration::from_secs(2)).await;
    match message_result {
        Ok(event_id) => println!("✅ Login command sent successfully. Event ID: {:#?}", event_id),
        Err(e) => println!("❌ Failed to send login command: {}", e),
    };
    
    // Wait longer for the bot to process the login command and generate QR
    println!("⏳ Waiting for bot to process login command and generate QR...");
    sleep(Duration::from_secs(1)).await;
    
    let mut qr_url = None;
    println!("⏳ Starting QR code monitoring");
    client.sync_once(MatrixSyncSettings::default()).await?;

    // Set up event handler for QR code
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(1));

    println!("🔄 Starting message polling loop");
    for attempt in 1..=60 { // Increase number of attempts
        println!("📡 Sync attempt #{}", attempt);
        client.sync_once(sync_settings.clone()).await?;
        
        // Add a small delay between syncs
        sleep(Duration::from_millis(500)).await;
        
        // Check for new messages
        if let Some(room) = client.get_room(&room_id) {
            println!("🏠 Found room, fetching messages");
            let mut options = matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            let messages = room.messages(options).await?;
            println!("📨 Fetched {} messages", messages.chunk.len());
            for msg in messages.chunk {
                let raw_event = msg.raw();
                if let Ok(event) = raw_event.deserialize() {
                    println!("📝 Processing event from sender: {}", event.sender());
                    println!("🤖 Processing event from {}", event.sender());
                    if event.sender() == bot_user_id {
                        println!("✅ Event from bridge bot: {:?}", event);
                        println!("📝 Event type: {:?}", event.event_type());
                        if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(sync_event)
                        ) = event.clone() {
                            println!("📨 Room message event from {}: {:?}", event.sender(), sync_event);
                            let event_content: RoomMessageEventContent = match sync_event {
                                SyncRoomMessageEvent::Original(original_event) => {
                                    println!("📄 Original content: {:?}", original_event.content);
                                    original_event.content
                                },
                                SyncRoomMessageEvent::Redacted(_) => continue,
                            };

                            println!("📨 Processing message event: {:?}", event_content);
                            match event_content.msgtype {
                                MessageType::Image(img_content) => {
                                    println!("🖼️ Image message: {:?}", img_content);
                                    if let matrix_sdk::ruma::events::room::MediaSource::Plain(mxc_uri) = img_content.source {
                                        // Get the homeserver URL
                                        let homeserver = client.homeserver().to_string().trim_end_matches('/').to_string();
                                        
                                        // Use the Matrix client's media API to construct the download URL
                                        // Handle potential errors from server_name() and media_id()
                                        let server_name = mxc_uri.server_name()
                                            .map_err(|e| {
                                                tracing::error!("Failed to get server name from MXC URI: {}", e);
                                                anyhow!("Invalid MXC URI server name")
                                            })?;
                                        let media_id = mxc_uri.media_id()
                                            .map_err(|e| {
                                                tracing::error!("Failed to get media ID from MXC URI: {}", e);
                                                anyhow!("Invalid MXC URI media ID")
                                            })?;

                                        // Ensure homeserver URL is properly formatted
                                        let homeserver = if homeserver.starts_with("http://") || homeserver.starts_with("https://") {
                                            homeserver.trim_end_matches('/').to_string()
                                        } else {
                                            format!("http://{}", homeserver.trim_end_matches('/'))
                                        };

                                        // Try both v3 and r0 endpoints
                                        let v3_url = format!("{}/_matrix/media/v3/download/{}/{}",
                                            homeserver,
                                            server_name,
                                            media_id
                                        );
                                        
                                        let r0_url = format!("{}/_matrix/media/r0/download/{}/{}",
                                            homeserver,
                                            server_name,
                                            media_id
                                        );

                                        // Create a new HTTP client
                                        let http_client = reqwest::Client::new();
                                        
                                        // Try v3 endpoint first, then fallback to r0
                                        let url = match http_client.get(&v3_url)
                                            .header("Authorization", format!("Bearer {}", access_token))
                                            .send()
                                            .await 
                                        {
                                            Ok(response) if response.status().is_success() => v3_url,
                                            _ => {
                                                println!("⚠️ v3 endpoint failed, trying r0 endpoint");
                                                r0_url
                                            }
                                        };
                                        
                                        // Add more detailed logging
                                        tracing::info!(
                                            "Constructing media URL from MXC URI: mxc://{}/{}", 
                                            server_name, 
                                            media_id
                                        );
                                        
                                        println!("🎯 Fetching QR code from: {}", url);
                                        
                                        // Fetch the image
                                        // Add authorization header to the request using the Matrix client's access token
                                        let http_client = reqwest::Client::new();
                                        match http_client.get(&url)
                                            .header("Authorization", format!("Bearer {}", access_token))
                                            .send()
                                            .await 
                                        {
                                            Ok(response) => {
                                                let status = response.status();
                                                if status.is_success() {
                                                    println!("✅ Successfully connected to media endpoint: {}", url);
                                                    tracing::info!("Successfully fetched QR code image");
                                                    match response.bytes().await {
                                                        Ok(bytes) => {
                                                            // Convert to base64
                                                            let base64_image = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
                                                            let data_url = format!("data:image/png;base64,{}", base64_image);
                                                            println!("🖼️ Generated data URL (first 50 chars): {}...", &data_url[..50]);
                                                            qr_url = Some(data_url);
                                                        },
                                                        Err(e) => {
                                                            println!("❌ Failed to read QR code bytes: {}", e);
                                                            continue;
                                                        }
                                                    }
                                                } else {
                                                    let error_text = response.text().await.unwrap_or_default();
                                                    tracing::error!(
                                                        "Failed to fetch QR code from {}: HTTP {} - {}",
                                                        url,
                                                        status,
                                                        error_text
                                                    );
                                                    println!("❌ Failed to fetch QR code from {}: HTTP {} - {}", url, status, error_text);
                                                    continue;
                                                }
                                            },
                                            Err(e) => {
                                                println!("❌ Failed to fetch QR code: {}", e);
                                                continue;
                                            }
                                        }
                                    }
                                },
                                MessageType::Text(text_content) => {
                                    println!("📝 Text message from {}: {}", event.sender(), text_content.body);
                                    // Check if the text contains a URL or QR code data
                                    if text_content.body.contains("http") || text_content.body.contains("mxc://") {
                                        println!("🔗 Possible QR URL in text: {}", text_content.body);
                                        qr_url = Some(text_content.body.clone());
                                    }
                                },
                                _ => println!("ℹ️ Other message type: {:?}", event_content.msgtype),
                            }
                        }
                    }
                    
                }
            }
        }

        if qr_url.is_some() {
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }

    let qr_url = qr_url.ok_or(anyhow!("QR code not received"))?;
    Ok((room_id.into(), qr_url))
}

pub async fn start_whatsapp_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<WhatsappConnectionResponse>, (StatusCode, AxumJson<serde_json::Value>)> {
    println!("🚀 Starting WhatsApp connection process for user {}", auth_user.user_id);
    tracing::info!("Starting WhatsApp connection for user {}", auth_user.user_id);

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
    let (room_id, qr_url) = connect_whatsapp(&client, &bridge_bot, &access_token)
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to WhatsApp bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to connect to WhatsApp bridge"})),
            )
        })?;

    // Debug: Log the QR code URL
    println!("Generated QR code URL: {}", &qr_url);
    tracing::info!("Generated QR code URL: {}", &qr_url);

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

    Ok(AxumJson(WhatsappConnectionResponse {
        qr_code_url: qr_url,
    }))
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
    println!("👀 Starting WhatsApp connection monitoring for user {}", user_id);
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;

    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(30));

    for _ in 0..20 { // Try for about 10 minutes (20 * 30 seconds)
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
                            // Access the content field from SyncMessageLikeEvent
                            let event_content: RoomMessageEventContent = match sync_event {
                                SyncRoomMessageEvent::Original(original_event) => original_event.content,
                                SyncRoomMessageEvent::Redacted(_) => continue,
                            };

                            if let MessageType::Text(text_content) = event_content.msgtype {
                                let content = text_content.body;
        
                                // Check for successful connection messages
                                if content.contains("successfully logged in") || content.contains("connected") {
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
 
                                // Check for error messages
                                if content.contains("error") || content.contains("failed") || content.contains("timeout") {
                                    println!("❌ WhatsApp connection failed for user {}: {}", user_id, content);
                                    state.user_repository.delete_whatsapp_bridge(user_id)?;
                                    return Err(anyhow!("WhatsApp connection failed: {}", content));
                                }
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

