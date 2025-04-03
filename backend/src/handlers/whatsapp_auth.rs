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
        OwnedRoomId, OwnedUserId,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use anyhow::{anyhow, Result};
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

    let (username, access_token) = matrix_auth.register_user().await
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
    client.restore_session(matrix_sdk::Session {
        meta: matrix_sdk::SessionMeta {
            user_id: OwnedUserId::try_from(&full_user_id).map_err(|e| {
                tracing::error!("Invalid user_id format: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": "Invalid user_id format"})),
                )
            })?,
            device_id: None,
        },
        tokens: matrix_sdk::SessionTokens {
            access_token: access_token.clone(),
            refresh_token: None,
        },
    }).await.map_err(|e| {
        tracing::error!("Failed to restore session: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Failed to restore Matrix session"})),
        )
    })?;
    let device_id = client.device_id().ok_or(anyhow!("No device_id")).map_err(|e| {
        tracing::error!("No device_id available: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "No device_id available"})),
        )
    })?.to_string();

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
) -> Result<(OwnedRoomId, String)> {
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;
    let request = CreateRoomRequest::new();
    let response = client.create_room(request).await?;
    let room_id = response.room_id();

    let room = client.get_room(&room_id).ok_or(anyhow!("Room not found"))?;
    room.invite_user_by_id(&bot_user_id).await?;
    room.send(RoomMessageEventContent::text_plain("login qr")).await?;

    let mut qr_url = None;
    client.sync_once(MatrixSyncSettings::default()).await?;

    // Set up event handler for QR code
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(1));

    for _ in 0..30 {
        client.sync_once(sync_settings.clone()).await?;
        
        // Check for new messages
        if let Some(room) = client.get_room(&room_id) {
            let options = matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            let messages = room.messages(options).await?;
            for msg in messages.chunk {
                let raw_event = msg.raw();
                if let Ok(event) = raw_event.deserialize() {
                    if event.sender() == bot_user_id {
                        if let AnySyncTimelineEvent::MessageLike(
                            AnySyncMessageLikeEvent::RoomMessage(sync_event)
                        ) = event {
                            // Access the content field from SyncMessageLikeEvent
                            let content: SyncMessageLikeEvent<RoomMessageEventContent> = sync_event.into();
                            let event_content: RoomMessageEventContent = match content {
                                SyncMessageLikeEvent::Original(original_event) => original_event.content,
                                SyncMessageLikeEvent::Redacted(_) => panic!("Event was redacted, expected original content"),
                            };
                            // Check if the message is an image
                            if let MessageType::Image(img_content) = event_content.msgtype {
                                // Get the MXC URI from the MediaSource
                                if let matrix_sdk::ruma::events::room::MediaSource::Plain(mxc_uri) = img_content.source {
                                    qr_url = Some(format!(
                                        "{}/_matrix/media/r0/download/{}", 
                                        client.homeserver(), 
                                        mxc_uri
                                    ));
                                    break;
                                }
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
    tracing::info!("Starting WhatsApp connection for user {}", auth_user.user_id);

    // Ensure user has Matrix credentials
    let (username, access_token) = ensure_matrix_credentials(&state, auth_user.user_id).await?;

    // Create Matrix client
    let homeserver_url = std::env::var("MATRIX_HOMESERVER")
        .expect("MATRIX_HOMESERVER not set");
    let bridge_bot = std::env::var("WHATSAPP_BRIDGE_BOT")
        .expect("WHATSAPP_BRIDGE_BOT not set");

    let client = MatrixClient::builder()
        .homeserver_url(homeserver_url)
        .build()
        .map_err(|e| {
            tracing::error!("Failed to create Matrix client: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to create Matrix client"})),
            )
        })?;
    use matrix_sdk::authentication::matrix::MatrixSession;
    use matrix_sdk::authentication::AuthSession;
    use matrix_sdk::SessionMeta;
    use matrix_sdk::authentication::matrix::MatrixSessionTokens;
    // Set the access token by restoring a session
    let session = AuthSession::Matrix(MatrixSession {
        meta: MatrixSession {
            user_id: 
            device_id:
        },
        tokens: MatrixSessionTokens {
            access_token: access_token.to_string(),
            refresh_token: None, // Optional, if you have one
        },
    });

    client.restore_session(session).await.map_err(|e| {
        tracing::error!("Failed to restore session: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Failed to restore session"})),
        )
    })?;


    // Connect to WhatsApp bridge
    let (room_id, qr_url) = connect_whatsapp(&client, &bridge_bot)
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to WhatsApp bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to connect to WhatsApp bridge"})),
            )
        })?;

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
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;
    let mut sync_settings = MatrixSyncSettings::default();
    sync_settings.timeout = Some(Duration::from_secs(30));

    for _ in 0..20 { // Try for about 10 minutes (20 * 30 seconds)
        client.sync_once(sync_settings.clone()).await?;
        
        if let Some(room) = client.get_room(room_id) {
            let messages = room.messages().await?;
            for msg in messages.chunk {
                if msg.sender == bot_user_id {
                    if let Some(SyncRoomMessageEvent::Text(text_msg)) = msg.event.as_message() {
                        let content = text_msg.content.body.to_lowercase();
                        
                        // Check for successful connection messages
                        if content.contains("successfully logged in") || content.contains("connected") {
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
                            state.user_repository.delete_whatsapp_bridge(user_id)?;
                            return Err(anyhow!("WhatsApp connection failed: {}", content));
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

