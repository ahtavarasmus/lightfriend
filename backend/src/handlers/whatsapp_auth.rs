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
use tokio::time::{sleep, Duration, Instant};
use crate::{
    AppState,
    handlers::auth_middleware::AuthUser,
    models::user_models::{NewBridge, Bridge},
    utils::matrix_auth,
};

// Using the centralized function from matrix_auth.rs instead

#[derive(Serialize)]
pub struct WhatsappConnectionResponse {
    pairing_code: String, 
}


async fn connect_whatsapp(
    client: &MatrixClient,
    bridge_bot: &str,

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
                    if event.sender() == bot_user_id {
                        if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(sync_event)
                        ) = event.clone() {
                            let event_content: RoomMessageEventContent = match sync_event {
                                SyncRoomMessageEvent::Original(original_event) => original_event.content,
                                SyncRoomMessageEvent::Redacted(_) => continue,
                            };

                            println!("📨 Processing message event");
                            if let MessageType::Notice(text_content) = event_content.msgtype {
                                println!("📝 Text message found from bot");
                                // Check for pairing code in the message (e.g., "FQWG-FHKC")
                                if !text_content.body.contains("Input the pairing code") {
                                    // Extract the pairing code (assumes format like "FQWG-FHKC")
                                    let parts: Vec<&str> = text_content.body.split_whitespace().collect();
                                    if let Some(code) = parts.last() {
                                        if code.contains('-') { // Basic validation for code format
                                            pairing_code = Some(code.to_string());
                                            println!("🔑 Found pairing code");
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

    println!("📝 Getting Matrix client...");
    // Get or create Matrix client using the centralized function
    let client = matrix_auth::get_client(auth_user.user_id, &state.user_repository, true)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get or create Matrix client: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to initialize Matrix client: {}", e)})),
            )
        })?;
    println!("✅ Matrix client obtained for user: {}", client.user_id().unwrap());

    // Get bridge bot from environment
    let bridge_bot = std::env::var("WHATSAPP_BRIDGE_BOT")
        .expect("WHATSAPP_BRIDGE_BOT not set");


    println!("🔗 Connecting to WhatsApp bridge...");
    // Connect to WhatsApp bridge
    let (room_id, pairing_code) = connect_whatsapp(&client, &bridge_bot, &phone_number)
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to WhatsApp bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to connect to WhatsApp bridge: {}", e)})),
            )
        })?;

    // Debug: Log the pairing code
    println!("Generated pairing code");
    tracing::info!("Generated pairing code");

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
            "created_at": bridge.created_at.unwrap_or(0), // Remove millisecond conversion
        }))),
        None => Ok(AxumJson(json!({
            "connected": false,
            "status": "not_connected",
            "created_at": 0,
        }))),
    }
}

/*
pub async fn accept_room_invitations(client: MatrixClient, duration: Duration) -> Result<()> {
    println!("🔄 Starting room invitation acceptance loop");
    // Enforce maximum duration of 15 minutes
    let max_duration = Duration::from_secs(900); // 15 minutes
    let actual_duration = if duration > max_duration {
        println!("⚠️ Requested duration exceeds maximum, capping at 15 minutes");
        max_duration
    } else {
        duration
    };
    let end_time = Instant::now() + actual_duration;
    
    // Ensure we have a recent sync before starting
    println!("🔄 Performing initial sync to get current room state");
    client.sync_once(MatrixSyncSettings::default()).await?;
    println!("😴 Waiting a little for room invitations to come in...");
    sleep(Duration::from_secs(15)).await;

    while Instant::now() < end_time {
        println!("👀 Checking for room invitations...");

        let invited_rooms = client.invited_rooms();
        let invitation_count = invited_rooms.len();

        if invitation_count == 0 {
            // Add a longer delay when no invitations are found to prevent tight loops
            sleep(Duration::from_secs(5)).await;
            continue;
        }

        println!("📬 Found {} room invitations", invitation_count);
        for (index, room) in invited_rooms.into_iter().enumerate() {
            let room_id = room.room_id();
            println!("🚪 Attempting to join room {}/{}", index + 1, invitation_count);

            match client.join_room_by_id(room_id).await {
                Ok(_) => {
                    println!("✅ Successfully joined room");
                }
                Err(e) => {
                    println!("❌ Failed to join room {}", e);
                    
                    // If we hit a rate limit or server error, add a longer delay
                    if e.to_string().contains("M_LIMIT_EXCEEDED") || e.to_string().contains("500") {
                        println!("⏳ Rate limit or server error detected, waiting longer...");
                        sleep(Duration::from_secs(5)).await;
                    }
                }
            }

            // Add a delay between each room join attempt
            println!("⏳ Taking a small breath before next room join...");
            sleep(Duration::from_millis(10)).await;
        }

        // Add a small delay between invitation check cycles
        println!("😴 Resting before next invitation check...");
        sleep(Duration::from_secs(15)).await;
    }

    println!("🏁 Room invitation acceptance loop completed");
    Ok(())
}
*/

                        

async fn monitor_whatsapp_connection(
    client: &MatrixClient,
    room_id: &OwnedRoomId,
    bridge_bot: &str,
    user_id: i32,
    state: Arc<AppState>,
) -> Result<(), anyhow::Error> {
    println!("👀 Starting WhatsApp connection monitoring for user {} in room {}", user_id, room_id);
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;

    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(30));

    // Increase monitoring duration and frequency
    for attempt in 1..120 { // Try for about 10 minutes (120 * 5 seconds)
        println!("🔄 Monitoring attempt #{} for user {}", attempt, user_id);

        let _= client.sync_once(sync_settings.clone()).await?;

        
        println!("🔍 Checking messages in room");
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
                                // Send the sync command for groups
                                if let Some(room) = client.get_room(&room_id) {

                                    // First sync all contacts
                                    room.send(RoomMessageEventContent::text_plain("!wa sync contacts --create-portals")).await?;
                                    println!("Sent !wa sync contacts --create-portals for user {}", user_id);
                                    
                                    // Wait a bit for contacts to sync
                                    sleep(Duration::from_secs(2)).await;
                                    
                                    // Then sync all groups
                                    room.send(RoomMessageEventContent::text_plain("!wa sync groups --create-portals")).await?;
                                    println!("Sent !wa sync groups --create-portals for user {}", user_id);
                                    
                                    // Wait a bit for groups to sync
                                    sleep(Duration::from_secs(2)).await;
                                    
                                } else {
                                    println!("NO WHATSAPP ROOM WAS FOUND AND NOT SYNC COMMANDS WERE SENT");
                                }
                                    

                                println!("Starting continuous sync and room invitation acceptance");
                                
                                // First start the continuous sync so we can receive invitations
                                let sync_client = client.clone();
                                tokio::spawn(async move {
                                    // continuous sync so the bridge can deliver invites and room-keys
                                    tracing::info!("Starting continuous sync for WhatsApp bridge");
                                    let _ = sync_client.sync(matrix_sdk::config::SyncSettings::default()).await;
                                    tracing::info!("Continuous sync ended");
                                });
                                
                                // Give the sync a moment to start up
                                sleep(Duration::from_secs(2)).await;
                                
                                /*
                                // Then start accepting invitations
                                let client_clone = client.clone();
                                tokio::spawn(async move {
                                    // Wait a bit for initial invitations to arrive
                                    sleep(Duration::from_secs(5)).await;
                                    
                                    // Run the invitation acceptance loop for 10 minutes
                                    if let Err(e) = accept_room_invitations(client_clone, Duration::from_secs(600)).await {
                                        tracing::error!("Error in accept_room_invitations: {}", e);
                                    }
                                });
                                */


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
                                state.user_repository.delete_whatsapp_bridge(user_id)?;
                                return Err(anyhow!("WhatsApp connection failed: {}", content));
                            }
                        }
                    }
                }
            }
        }

        sleep(Duration::from_secs(5)).await;
    }

    // If we reach here, connection timed out
    state.user_repository.delete_whatsapp_bridge(user_id)?;
    Err(anyhow!("WhatsApp connection timed out"))
}


pub async fn resync_whatsapp(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    println!("🔄 Starting WhatsApp resync process for user {}", auth_user.user_id);

    // Get the bridge information first
    let bridge = state.user_repository.get_whatsapp_bridge(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get WhatsApp bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get WhatsApp bridge info"})),
            )
        })?;

    let Some(bridge) = bridge else {
        return Err((
            StatusCode::BAD_REQUEST,
            AxumJson(json!({"error": "WhatsApp is not connected"})),
        ));
    };

    // Get Matrix client using the centralized function
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
        println!("📱 Sending WhatsApp sync commands");
        
        // First sync all contacts
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("!wa sync contacts --create-portals")).await {
            tracing::error!("Failed to send contacts sync command: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to send contacts sync command"})),
            ));
        }
        println!("✅ Sent contacts sync command");
        
        // Wait a bit for contacts to sync
        sleep(Duration::from_secs(2)).await;
        
        // Then sync all groups
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("!wa sync groups --create-portals")).await {
            tracing::error!("Failed to send groups sync command: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to send groups sync command"})),
            ));
        }
        println!("✅ Sent groups sync command");

        /*
        // Start accepting invitations for new rooms
        let client_clone = client.clone();
        tokio::spawn(async move {
            // Wait a bit for initial invitations to arrive
            sleep(Duration::from_secs(5)).await;
            
            // Run the invitation acceptance loop for 15 minutes
            if let Err(e) = accept_room_invitations(client_clone, Duration::from_secs(900)).await {
                tracing::error!("Error in accept_room_invitations: {}", e);
            }
        });
        */

        println!("✅ WhatsApp resync process completed for user {}", auth_user.user_id);
        Ok(AxumJson(json!({
            "message": "WhatsApp resync initiated successfully"
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            AxumJson(json!({"error": "WhatsApp bridge room not found"})),
        ))
    }
}

pub async fn disconnect_whatsapp(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    println!("🔌 Starting WhatsApp disconnection process for user {}", auth_user.user_id);

    // Get the bridge information first
    let bridge = state.user_repository.get_whatsapp_bridge(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get WhatsApp bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get WhatsApp bridge info"})),
            )
        })?;

    let Some(bridge) = bridge else {
        return Ok(AxumJson(json!({
            "message": "WhatsApp was not connected"
        })));
    };

    // Get or create Matrix client using the centralized function
    let client = matrix_auth::get_client(auth_user.user_id, &state.user_repository, false)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get or create Matrix client: {}", e);
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
        println!("📤 Sending WhatsApp logout command");
        // Send logout command
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("!wa logout")).await {
            tracing::error!("Failed to send logout command: {}", e);
        }

        // Wait a moment for the logout to process
        sleep(Duration::from_secs(2)).await;

        println!("🧹 Cleaning up WhatsApp portals");
        // Send command to delete all portals
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("!wa delete-all-portals")).await {
            tracing::error!("Failed to send delete-portals command: {}", e);
        }

        // Wait a moment for the cleanup to process
        sleep(Duration::from_secs(2)).await;

        println!("🗑️ Sending delete-session command");
        // Send delete-session command as a final cleanup
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("!wa delete-session")).await {
            tracing::error!("Failed to send delete-session command: {}", e);
        }
    }

    // Delete the bridge record
    state.user_repository.delete_whatsapp_bridge(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to delete WhatsApp bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to delete bridge record"})),
            )
        })?;

    println!("✅ WhatsApp disconnection completed for user {}", auth_user.user_id);
    Ok(AxumJson(json!({
        "message": "WhatsApp disconnected successfully"
    })))
}

