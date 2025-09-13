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
async fn connect_telegram_with_retry(
    client: &mut Arc<MatrixClient>,
    bridge_bot: &str,
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
        match connect_telegram(client, bridge_bot).await {
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
pub struct TelegramConnectionResponse {
    login_url: String, 
}


async fn connect_telegram(
    client: &MatrixClient,
    bridge_bot: &str,
) -> Result<(OwnedRoomId, String)> {
    tracing::debug!("🚀 Starting Telegram connection process");
    
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
    // Send cancel command to get rid of the previous login
    let cancel_command = format!("!tg cancel");
    room.send(RoomMessageEventContent::text_plain(&cancel_command)).await?;


    // Send login command
    let login_command = format!("!tg login");
    tracing::debug!("📤 Sending Telegram login command: {}", login_command);
    room.send(RoomMessageEventContent::text_plain(&login_command)).await?;

    // Optimized login url detection with event handler
    let mut login_url = None;
    tracing::debug!("⏳ Starting login url monitoring");
    
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

                            // More efficient login url extraction
                            if let Some(url) = extract_login_url(&message_body) {
                                login_url = Some(url);
                                tracing::debug!("🔑 Found login url");
                                break;
                            }
                        }
                    }
                }
            }
        }

        if login_url.is_some() {
            break;
        }
        
        // Balanced delay - fast enough for responsiveness, long enough for user input
        sleep(Duration::from_millis(500)).await; // 500ms gives good balance
    }

    let login_url = login_url.ok_or(anyhow!("Telegram login url not received within 30 seconds. Please try again."))?;
    Ok((room_id.into(), login_url))
}

// Helper function to extract login url more efficiently
fn extract_login_url(message: &str) -> Option<String> {
    // Remove backticks and other formatting that might interfere
    let clean_message = message.replace('`', "").replace("*", "");
    
    // Regex to match the full URL within Markdown [text](url) format
    let re = regex::Regex::new(r"\((https?://[^\)]+)\)").ok()?;
    
    if let Some(captures) = re.captures(&clean_message) {
        return Some(captures[1].to_string()); // Capture the URL inside the parentheses
    }
    
    None
}


pub async fn start_telegram_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<TelegramConnectionResponse>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::debug!("🚀 Starting Telegram connection process for user {}", auth_user.user_id);

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
    let bridge_bot = std::env::var("TELEGRAM_BRIDGE_BOT")
        .expect("TELEGRAM_BRIDGE_BOT not set");


    tracing::debug!("🔗 Connecting to Telegram bridge...");
    // Connect to Telegram bridge
    let mut client_clone = Arc::clone(&client);
    let (room_id, login_url) = connect_telegram_with_retry(
        &mut client_clone,
        &bridge_bot,
        auth_user.user_id,
        &state,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to connect to Telegram bridge: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": format!("Failed to connect to Telegram bridge: {}", e)})),
        )
    })?;


    // Debug: Log the login url
    tracing::info!("Generated login url");

    // Create bridge record
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
        match monitor_telegram_connection(
            &client_clone,
            &room_id_clone,
            &bridge_bot_clone,
            auth_user.user_id,
            state_clone,
        ).await {
            Ok(_) => {
                tracing::info!("Telegram connection monitoring completed successfully for user {}", auth_user.user_id);
            },
            Err(e) => {
                tracing::error!("Telegram connection monitoring failed for user {}: {}", auth_user.user_id, e);
            }
        }
    });

    Ok(AxumJson(TelegramConnectionResponse { login_url }))
}


pub async fn get_telegram_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::debug!("📊 Checking Telegram status for user {}", auth_user.user_id);
    let bridge = state.user_repository.get_bridge(auth_user.user_id, "telegram")
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
            "created_at": bridge.created_at.unwrap_or(0), // Remove millisecond conversion
        }))),
        None => Ok(AxumJson(json!({
            "connected": false,
            "status": "not_connected",
            "created_at": 0,
        }))),
    }
}


async fn monitor_telegram_connection(
    client: &MatrixClient,
    room_id: &OwnedRoomId,
    bridge_bot: &str,
    user_id: i32,
    state: Arc<AppState>,
) -> Result<(), anyhow::Error> {
    tracing::info!("👀 Starting Telegram connection monitoring for user {} in room {}", user_id, room_id);
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;

    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(10));


    for attempt in 1..=120 { // Increase to 10 minutes (120 * 5 seconds)
        tracing::info!("🔄 Monitoring attempt #{} for user {}", attempt, user_id);

        // Send login command to trigger a response
        if let Some(room) = client.get_room(room_id) {
            tracing::debug!("📤 Sending login command to verify connection");
            room.send(RoomMessageEventContent::text_plain("login")).await?;
        }

        let _ = client.sync_once(sync_settings.clone()).await?;

        if let Some(room) = client.get_room(room_id) {
            let mut options = matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            options.limit = matrix_sdk::ruma::UInt::new(20).unwrap(); // Increase to 20 messages
            let messages = room.messages(options).await?;

            for msg in messages.chunk {
                let raw_event = msg.raw();
                if let Ok(event) = raw_event.deserialize() {
                    if event.sender() == bot_user_id {
                        if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(sync_event),
                        ) = event {
                            let event_content = match sync_event {
                                SyncRoomMessageEvent::Original(original_event) => original_event.content,
                                SyncRoomMessageEvent::Redacted(_) => continue,
                            };

                            let content = match event_content.msgtype {
                                MessageType::Text(text_content) => text_content.body,
                                MessageType::Notice(notice_content) => notice_content.body,
                                _ => continue,
                            };

                            // Check for successful login or already logged in
                            if content.contains("Logged in") || content.contains("You are already logged in") {
                                tracing::debug!("🎉 Telegram successfully connected for user {}", user_id);
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
                                    cooldown_seconds: 30,
                                };
                                state.user_repository.delete_bridge(user_id, "telegram")?;
                                state.user_repository.create_bridge(new_bridge)?;

                                // Add client to app state and start sync
                                let mut matrix_clients = state.matrix_clients.lock().await;
                                let mut sync_tasks = state.matrix_sync_tasks.lock().await;

                                let state_for_handler = Arc::clone(&state);
                                client.add_event_handler(move |ev: matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent, room: matrix_sdk::room::Room, client| {
                                    let state = Arc::clone(&state_for_handler);
                                    async move {
                                        tracing::debug!("📨 Received message in room {}: {:?}", room.room_id(), ev);
                                        crate::utils::bridge::handle_bridge_message(ev, room, client, state).await;
                                    }
                                });

                                let client_arc = Arc::new(client.clone());
                                matrix_clients.insert(user_id, client_arc.clone());

                                let sync_settings = MatrixSyncSettings::default()
                                    .timeout(Duration::from_secs(30))
                                    .full_state(true);

                                let handle = tokio::spawn(async move {
                                    loop {
                                        match client_arc.sync(sync_settings.clone()).await {
                                            Ok(_) => {
                                                tracing::debug!("Sync completed normally for user {}", user_id);
                                                tokio::time::sleep(Duration::from_secs(1)).await;
                                            }
                                            Err(e) => {
                                                tracing::error!("Matrix sync error for user {}: {}", user_id, e);
                                                tokio::time::sleep(Duration::from_secs(30)).await;
                                            }
                                        }
                                    }
                                });

                                sync_tasks.insert(user_id, handle);

                                if let Some(room) = client.get_room(&room_id) {
                                    room.send(RoomMessageEventContent::text_plain("sync contacts")).await?;
                                    tracing::debug!("Sent contacts sync command for user {}", user_id);
                                    sleep(Duration::from_millis(500)).await;
                                    room.send(RoomMessageEventContent::text_plain("sync chats")).await?;
                                    tracing::debug!("Sent chats sync command for user {}", user_id);
                                } else {
                                    tracing::error!("Telegram room not found for sync commands");
                                }

                                return Ok(());
                            }

                            let error_patterns = [
                                "error", "failed", "timeout", "disconnected", "invalid code",
                                "connection lost", "authentication failed", "login failed",
                            ];
                            if error_patterns.iter().any(|&pattern| content.to_lowercase().contains(pattern)) {
                                tracing::error!("❌ Telegram connection failed for user {}: {}", user_id, content);
                                state.user_repository.delete_bridge(user_id, "telegram")?;
                                return Err(anyhow!("Telegram connection failed: {}", content));
                            }
                        }
                    }
                }
            }
        }

        sleep(Duration::from_secs(5)).await; // Increase to 5 seconds for stability
    }

    state.user_repository.delete_bridge(user_id, "telegram")?;
    Err(anyhow!("Telegram connection timed out after 10 minutes"))
}

pub async fn resync_telegram(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    println!("🔄 Starting Telegram resync process for user {}", auth_user.user_id);

    // Get the bridge information first
    let bridge = state.user_repository.get_bridge(auth_user.user_id, "telegram")
        .map_err(|e| {
            tracing::error!("Failed to get Telegram bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Telegram bridge info"})),
            )
        })?;

    let Some(bridge) = bridge else {
        return Err((
            StatusCode::BAD_REQUEST,
            AxumJson(json!({"error": "Telegram is not connected"})),
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
            tracing::info!("🔄 Starting continuous Matrix sync for Telegram bridge");
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

        tracing::debug!("📱 Sending Telegram sync commands");
        
        // First sync all contacts
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("sync contacts")).await {
            tracing::error!("Failed to send contacts sync command: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to send contacts sync command"})),
            ));
        }
        tracing::debug!("✅ Sent contacts sync command");
        
        // Wait a bit for contacts to sync
        sleep(Duration::from_secs(2)).await;
        
        // Then sync all chats
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("sync chats")).await {
            tracing::error!("Failed to send chats sync command: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to send chats sync command"})),
            ));
        }
        tracing::debug!("✅ Sent chats sync command");

        tracing::debug!("✅ Telegram resync process completed for user {}", auth_user.user_id);
        Ok(AxumJson(json!({
            "message": "Telegram resync initiated successfully"
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            AxumJson(json!({"error": "Telegram bridge room not found"})),
        ))
    }
}

pub async fn disconnect_telegram(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::debug!("🔌 Starting Telegram disconnection process for user {}", auth_user.user_id);

    // Get the bridge information first
    let bridge = state.user_repository.get_bridge(auth_user.user_id, "telegram")
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
        tracing::debug!("📤 Sending Telegram logout command");
        // Send logout command
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("logout")).await {
            tracing::error!("Failed to send logout command: {}", e);
        }

        // Wait a moment for the logout to process
        sleep(Duration::from_secs(5)).await;

        tracing::debug!("🧹 Cleaning up Telegram portals");
        // Send command to clean rooms
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("clean-rooms")).await {
            tracing::error!("Failed to send clean-rooms command: {}", e);
        }

        // Wait a moment for the cleanup to process
        sleep(Duration::from_secs(5)).await;
    }

    // Remove client and sync task from app state
    {
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
    }

    // Delete the bridge record
    state.user_repository.delete_bridge(auth_user.user_id, "telegram")
        .map_err(|e| {
            tracing::error!("Failed to delete Telegram bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to delete bridge record"})),
            )
        })?;

    tracing::debug!("✅ Telegram disconnection completed for user {}", auth_user.user_id);
    Ok(AxumJson(json!({
        "message": "Telegram disconnected successfully"
    })))
}
