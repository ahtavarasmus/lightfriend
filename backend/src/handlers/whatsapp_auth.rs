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
use crate::{
    AppState,
    handlers::auth_middleware::AuthUser,
    models::user_models::{NewBridge},
    utils::matrix_auth,
};

use tokio::fs;
use std::path::Path;


// Helper function to detect the one-time key conflict error
fn is_one_time_key_conflict(error: &anyhow::Error) -> bool {
    if let Some(http_err) = error.downcast_ref::<matrix_sdk::HttpError>() {
        let error_str = http_err.to_string();
        return error_str.contains("One time key") && error_str.contains("already exists");
    }
    false
}

// Helper function to get the store path
fn get_store_path(username: &str) -> Result<String> {
    let persistent_store_path = std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
        .map_err(|_| anyhow!("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"))?;
    Ok(format!("{}/{}", persistent_store_path, username))
}

// Wrapper function with retry logic
async fn connect_whatsapp_with_retry(
    client: &mut Arc<MatrixClient>,
    bridge_bot: &str,
    phone_number: &str,
    user_id: i32,
    state: &Arc<AppState>,
) -> Result<(OwnedRoomId, String)> {
    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY: Duration = Duration::from_secs(2);
    
    let username = client.user_id()
        .ok_or_else(|| anyhow!("User ID not available"))?
        .localpart()
        .to_string();

    for retry_count in 0..MAX_RETRIES {
        match connect_whatsapp(client, bridge_bot, phone_number).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if retry_count < MAX_RETRIES - 1 && is_one_time_key_conflict(&e) {
                    tracing::warn!(
                        "One-time key conflict detected for user {} (attempt {}/{}), resetting client store", 
                        user_id, 
                        retry_count + 1, 
                        MAX_RETRIES
                    );
                    
                    // Clear the store
                    let store_path = get_store_path(&username)?;
                    if Path::new(&store_path).exists() {
                        fs::remove_dir_all(&store_path).await?;
                        sleep(Duration::from_millis(500)).await; // Small delay before recreation
                        fs::create_dir_all(&store_path).await?;
                        tracing::info!("Cleared store directory: {}", store_path);
                    }
                    
                    // Add delay before retry
                    sleep(RETRY_DELAY).await;
                    
                   // Reinitialize client (bypass cache since we're recovering from an error)
                    match matrix_auth::get_client(user_id, &state).await {
                        Ok(new_client) => {
                            *client = new_client.into(); // Update the client reference
                            tracing::info!("Client reinitialized, retrying operation");
                            continue;
                        },
                        Err(init_err) => {
                            tracing::error!("Failed to reinitialize client: {}", init_err);
                            return Err(init_err);
                        }
                    }
                } else {
                    if is_one_time_key_conflict(&e) {
                        return Err(anyhow!("Failed after {} attempts to resolve one-time key conflict: {}", MAX_RETRIES, e));
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }
    
    Err(anyhow!("Exceeded maximum retry attempts ({})", MAX_RETRIES))
}

#[derive(Serialize)]
pub struct WhatsappConnectionResponse {
    pairing_code: String, 
}


async fn connect_whatsapp(
    client: &MatrixClient,
    bridge_bot: &str,
    phone_number: &str,
) -> Result<(OwnedRoomId, String)> {
    tracing::debug!("🚀 Starting WhatsApp connection process");
    
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;
    
    let request = CreateRoomRequest::new();
    let response = client.create_room(request).await?;
    let room_id = response.room_id();

    tracing::debug!("🏠 Created room with ID: {}", room_id);
    
    let room = client.get_room(&room_id).ok_or(anyhow!("Room not found"))?;
    
    tracing::debug!("🤖 Inviting bot user: {}", bot_user_id);
    room.invite_user_by_id(&bot_user_id).await?;
    
    // Single sync to get the invitation processed
    client.sync_once(MatrixSyncSettings::default().timeout(Duration::from_secs(5))).await?;
    
    // Reduced wait time and more frequent checks
    let mut attempt = 0;
    for _ in 0..15 { // Reduced from 30 to 15
        attempt += 1;
        println!("🔍 Check attempt {}/15 for bot join status", attempt);
        let members = room.members(matrix_sdk::RoomMemberships::JOIN).await?;
        if members.iter().any(|m| m.user_id() == bot_user_id) {
            tracing::debug!("✅ Bot has joined the room");
            break;
        }
        sleep(Duration::from_millis(500)).await; // Reduced from 1 second to 500ms
    }
    
    // Quick membership check
    let members = room.members(matrix_sdk::RoomMemberships::empty()).await?;
    if !members.iter().any(|m| m.user_id() == bot_user_id) {
        println!("❌ Bot failed to join room after all attempts");
        return Err(anyhow!("Bot {} failed to join room", bot_user_id));
    }
    // Send cancel command  to get rid of hte previous login
    let cancel_command = format!("!wa cancel");
    room.send(RoomMessageEventContent::text_plain(&cancel_command)).await?;


    // Send login command with phone number
    let login_command = format!("!wa login phone {}", phone_number);
    tracing::debug!("📤 Sending WhatsApp login command: {}", login_command);
    room.send(RoomMessageEventContent::text_plain(&login_command)).await?;

    // Optimized pairing code detection with event handler
    let mut pairing_code = None;
    tracing::debug!("⏳ Starting pairing code monitoring");
    
    // Use shorter sync timeout for faster response
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_millis(1500));

    for attempt in 1..=60 { // Increased to 60 attempts for longer user input time
        println!("📡 Sync attempt #{}/60", attempt);
        tracing::debug!("📡 Sync attempt #{}", attempt);
        client.sync_once(sync_settings.clone()).await?;


        if let Some(room) = client.get_room(&room_id) {
            // Get only the most recent messages to reduce processing time
            let mut options = matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            options.limit = matrix_sdk::ruma::UInt::new(5).unwrap(); // Reduced from 10 to 5
            let messages = room.messages(options).await?;
            
            for (i, msg) in messages.chunk.iter().enumerate() {
                let raw_event = msg.raw();
                if let Ok(event) = raw_event.deserialize() {
                    if event.sender() == bot_user_id {
                        if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(sync_event)
                        ) = event.clone() {
                            let event_content: RoomMessageEventContent = match sync_event {
                                SyncRoomMessageEvent::Original(original_event) => {
                                    original_event.content
                                },
                                SyncRoomMessageEvent::Redacted(_) => {
                                    continue;
                                },
                            };

                            let message_body = match event_content.msgtype {
                                MessageType::Notice(text_content) => {
                                    text_content.body
                                },
                                MessageType::Text(text_content) => {
                                    text_content.body
                                },
                                _ => {
                                    continue;
                                },
                            };

                            
                            // More efficient pairing code extraction
                            if !message_body.contains("Input the pairing code") {
                                // Look for pattern like "XXXX-XXXX" in the message
                                if let Some(code) = extract_pairing_code(&message_body) {
                                    pairing_code = Some(code);
                                    tracing::debug!("🔑 Found pairing code");
                                    break;
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
        
        // Balanced delay - fast enough for responsiveness, long enough for user input
        sleep(Duration::from_millis(500)).await; // 500ms gives good balance
    }

    let pairing_code = pairing_code.ok_or(anyhow!("WhatsApp pairing code not received within 30 seconds. Please try again."))?;
    Ok((room_id.into(), pairing_code))
}

// Helper function to extract pairing code more efficiently
fn extract_pairing_code(message: &str) -> Option<String> {
    // Remove backticks and other formatting
    let clean_message = message.replace('`', "").replace("*", "");
    
    // Look for pattern: 4 alphanumeric characters, dash, 4 alphanumeric characters
    let re = regex::Regex::new(r"([A-Z0-9]{4}-[A-Z0-9]{4})").ok()?;
    
    if let Some(captures) = re.captures(&clean_message) {
        return Some(captures[1].to_string());
    }
    
    // Fallback: look for any sequence that looks like a pairing code
    let re_flexible = regex::Regex::new(r"([A-Z0-9]{4}-[A-Z0-9]{4})").ok()?;
    if let Some(captures) = re_flexible.captures(&clean_message) {
        return Some(captures[1].to_string());
    }
    
    None
}


pub async fn start_whatsapp_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<WhatsappConnectionResponse>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::debug!("🚀 Starting WhatsApp connection process for user {}", auth_user.user_id);

    // Fetch user's phone number
    let phone_number = state
        .user_core
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

    tracing::debug!("📝 Getting Matrix client...");
    // Get or create Matrix client using the centralized function
    let client = matrix_auth::get_cached_client(auth_user.user_id, &state)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get or create Matrix client: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to initialize Matrix client: {}", e)})),
            )
        })?;
    tracing::debug!("✅ Matrix client obtained for user: {}", client.user_id().unwrap());

    // Get bridge bot from environment
    let bridge_bot = std::env::var("WHATSAPP_BRIDGE_BOT")
        .expect("WHATSAPP_BRIDGE_BOT not set");


    tracing::debug!("🔗 Connecting to WhatsApp bridge...");
    // Connect to WhatsApp bridge
    let mut client_clone = Arc::clone(&client);
    let (room_id, pairing_code) = connect_whatsapp_with_retry(
        &mut client_clone,
        &bridge_bot,
        &phone_number,
        auth_user.user_id,
        &state,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to connect to WhatsApp bridge: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": format!("Failed to connect to WhatsApp bridge: {}", e)})),
        )
    })?;


    // Debug: Log the pairing code
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
        cooldown_seconds: 30,
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
    tracing::debug!("📊 Checking WhatsApp status for user {}", auth_user.user_id);
    let bridge = state.user_repository.get_bridge(auth_user.user_id, "whatsapp")
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
                       

async fn monitor_whatsapp_connection(
    client: &MatrixClient,
    room_id: &OwnedRoomId,
    bridge_bot: &str,
    user_id: i32,
    state: Arc<AppState>,
) -> Result<(), anyhow::Error> {
    tracing::debug!("👀 Starting optimized WhatsApp connection monitoring for user {} in room {}", user_id, room_id);
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;

    // Shorter sync timeout for faster response
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(10));

    // Reduced monitoring duration but more frequent checks
    for attempt in 1..60 { // Try for about 5 minutes (60 * 5 seconds)
        tracing::debug!("🔄 Monitoring attempt #{} for user {}", attempt, user_id);

        let _ = client.sync_once(sync_settings.clone()).await?;
        

        if let Some(room) = client.get_room(room_id) {
            // Get only recent messages to reduce processing time
            let mut options = matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            options.limit = matrix_sdk::ruma::UInt::new(5).unwrap(); // Reduced from default to 5
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

                            let content = match event_content.msgtype {
                                MessageType::Text(text_content) => text_content.body,
                                MessageType::Notice(notice_content) => notice_content.body,
                                _ => continue,
                            };



                            // Check for successful login message first
                            if content.contains("Successfully logged in as") {
                                tracing::debug!("🎉 WhatsApp successfully connected for user {} with phone number confirmation", user_id);
                                
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
                                    cooldown_seconds: 30,
                                };

                                state.user_repository.delete_bridge(user_id, "whatsapp")?;
                                state.user_repository.create_bridge(new_bridge)?;

                                // Add client to app state and start sync
                                let mut matrix_clients = state.matrix_clients.lock().await;
                                let mut sync_tasks = state.matrix_sync_tasks.lock().await;

                                // Add event handlers before storing/cloning the client
                                use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;
                                use matrix_sdk::room::Room;
                                
                                let state_for_handler = Arc::clone(&state);
                                client.add_event_handler(move |ev: OriginalSyncRoomMessageEvent, room: Room, client| {
                                    let state = Arc::clone(&state_for_handler);
                                    async move {
                                        tracing::debug!("📨 Received message in room {}: {:?}", room.room_id(), ev);
                                        crate::utils::bridge::handle_bridge_message(ev, room, client, state).await;
                                    }
                                });

                                // Store the client
                                let client_arc = Arc::new(client.clone());
                                matrix_clients.insert(user_id, client_arc.clone());

                                // Create sync task
                                let sync_settings = MatrixSyncSettings::default()
                                    .timeout(Duration::from_secs(30))
                                    .full_state(true);

                                let handle = tokio::spawn(async move {
                                    loop {
                                        match client_arc.sync(sync_settings.clone()).await {
                                            Ok(_) => {
                                                tracing::debug!("Sync completed normally for user {}", user_id);
                                                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                                            },
                                            Err(e) => {
                                                tracing::error!("Matrix sync error for user {}: {}", user_id, e);
                                                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                                            }
                                        }
                                    }
                                });

                                sync_tasks.insert(user_id, handle);

                                // Send sync commands with reduced delays
                                if let Some(room) = client.get_room(&room_id) {
                                    // Send both commands quickly without long waits
                                    room.send(RoomMessageEventContent::text_plain("!wa sync contacts --create-portals")).await?;
                                    tracing::debug!("Sent contacts sync command for user {}", user_id);
                                    
                                    // Shorter wait time
                                    sleep(Duration::from_millis(500)).await;
                                    
                                    room.send(RoomMessageEventContent::text_plain("!wa sync groups --create-portals")).await?;
                                    tracing::debug!("Sent groups sync command for user {}", user_id);
                                } else {
                                    tracing::error!("WhatsApp room not found for sync commands");
                                }


                                return Ok(());
                            }


                            // Check for error messages with more specific patterns
                            let error_patterns = [
                                "error",
                                "failed",
                                "timeout",
                                "disconnected",
                                "invalid code",
                                "connection lost",
                                "authentication failed",
                                "login failed"
                            ];

                            if error_patterns.iter().any(|&pattern| content.to_lowercase().contains(pattern)) {
                                tracing::error!("❌ WhatsApp connection failed for user {}: {}", user_id, content);
                                state.user_repository.delete_bridge(user_id, "whatsapp")?;
                                return Err(anyhow!("WhatsApp connection failed: {}", content));
                            }
                        }
                    }
                }
            }
        }

        // Shorter sleep between checks for faster response
        sleep(Duration::from_secs(3)).await; // Reduced from 5 to 3 seconds
    }

    // If we reach here, connection timed out
    state.user_repository.delete_bridge(user_id, "whatsapp")?;
    Err(anyhow!("WhatsApp connection timed out after 3 minutes"))
}


pub async fn resync_whatsapp(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    println!("🔄 Starting WhatsApp resync process for user {}", auth_user.user_id);

    // Get the bridge information first
    let bridge = state.user_repository.get_bridge(auth_user.user_id, "whatsapp")
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

    // Get Matrix client using the cached version
    let client = matrix_auth::get_cached_client(auth_user.user_id, &state)
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
        println!("📱 Setting up Matrix event handler");
        
        // Set up event handler for the Matrix client
        client.add_event_handler(|ev: SyncRoomMessageEvent| async move {
            match ev {
                SyncRoomMessageEvent::Original(msg) => {
                    // Add more specific message handling logic here if needed
                },
                SyncRoomMessageEvent::Redacted(_) => {
                    println!("🗑️ Received redacted message event");
                }
            }
        });

        // Start continuous sync in the background
        let sync_client = client.clone();
        tokio::spawn(async move {
            tracing::info!("🔄 Starting continuous Matrix sync for WhatsApp bridge");
            let sync_settings = MatrixSyncSettings::default()
                .timeout(Duration::from_secs(30))
                .full_state(true);
            
            if let Err(e) = sync_client.sync(sync_settings).await {
                tracing::error!("❌ Matrix sync error: {}", e);
            }
            tracing::info!("🛑 Continuous sync ended");
        });

        // Give the sync a moment to start up
        sleep(Duration::from_secs(2)).await;

        tracing::debug!("📱 Sending WhatsApp sync commands");
        
        // First sync all contacts
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("!wa sync contacts --create-portals")).await {
            tracing::error!("Failed to send contacts sync command: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to send contacts sync command"})),
            ));
        }
        tracing::debug!("✅ Sent contacts sync command");
        
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
        tracing::debug!("✅ Sent groups sync command");

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

        tracing::debug!("✅ WhatsApp resync process completed for user {}", auth_user.user_id);
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
    tracing::debug!("🔌 Starting WhatsApp disconnection process for user {}", auth_user.user_id);

    // Get the bridge information first
    let bridge = state.user_repository.get_bridge(auth_user.user_id, "whatsapp")
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

    // Get or create Matrix client using the cached version
    let client = matrix_auth::get_cached_client(auth_user.user_id, &state)
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
        tracing::debug!("📤 Sending WhatsApp logout command");
        // Send logout command
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("!wa logout")).await {
            tracing::error!("Failed to send logout command: {}", e);
        }

        // Wait a moment for the logout to process
        sleep(Duration::from_secs(5)).await;

        tracing::debug!("🧹 Cleaning up WhatsApp portals");
        // Send command to delete all portals
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("!wa delete-all-portals")).await {
            tracing::error!("Failed to send delete-portals command: {}", e);
        }

        // Wait a moment for the cleanup to process
        sleep(Duration::from_secs(5)).await;

        tracing::debug!("🗑️ Sending delete-session command");
        // Send delete-session command as a final cleanup
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("!wa delete-session")).await {
            tracing::error!("Failed to send delete-session command: {}", e);
        }
        sleep(Duration::from_secs(5)).await;
    }

    // Delete the bridge record
    state.user_repository.delete_bridge(auth_user.user_id, "whatsapp")
        .map_err(|e| {
            tracing::error!("Failed to delete WhatsApp bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to delete bridge record"})),
            )
        })?;

    // Check if there are any remaining active bridges
    let has_active_bridges = state.user_repository.has_active_bridges(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to check active bridges: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to check active bridges"})),
            )
        })?;

    if !has_active_bridges {
        // No active bridges left, remove client and sync task
        let mut matrix_clients = state.matrix_clients.lock().await;
        let mut sync_tasks = state.matrix_sync_tasks.lock().await;

        // Remove and abort the sync task if it exists
        if let Some(task) = sync_tasks.remove(&auth_user.user_id) {
            task.abort();
            tracing::debug!("Aborted sync task for user {}", auth_user.user_id);
        }

        // Remove the client if it exists
        if matrix_clients.remove(&auth_user.user_id).is_some() {
            tracing::debug!("Removed Matrix client for user {}", auth_user.user_id);
        }
    } else {
        tracing::debug!("Other active bridges exist for user {}, keeping Matrix client", auth_user.user_id);
    }

    tracing::debug!("✅ WhatsApp disconnection completed for user {}", auth_user.user_id);
    Ok(AxumJson(json!({
        "message": "WhatsApp disconnected successfully"
    })))
}
