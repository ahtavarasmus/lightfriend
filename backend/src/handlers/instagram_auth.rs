use axum::{
    extract::{State, Json as AxumJson},
    http::StatusCode,
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
// Wrapper function with retry logic (similar to Messenger)
async fn connect_instagram_with_retry(
    client: &mut Arc<MatrixClient>,
    bridge_bot: &str,
    curl_paste: &str,
    user_id: i32,
    state: &Arc<AppState>,
) -> Result<OwnedRoomId> {
    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY: Duration = Duration::from_secs(2);
 
    let username = client.user_id()
        .ok_or_else(|| anyhow!("User ID not available"))?
        .localpart()
        .to_string();
    for retry_count in 0..MAX_RETRIES {
        tracing::debug!("Attempt {}/{} to connect Instagram for user {}", retry_count + 1, MAX_RETRIES, user_id);
        match connect_instagram(client, bridge_bot, curl_paste).await {
            Ok(result) => {
                tracing::info!("Successfully connected Instagram on attempt {}/{} for user {}", retry_count + 1, MAX_RETRIES, user_id);
                return Ok(result);
            },
            Err(e) => {
                tracing::error!("Error during Instagram connection attempt {}/{} for user {}: {}", retry_count + 1, MAX_RETRIES, user_id, e);
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
                        tracing::debug!("Clearing store directory: {}", store_path);
                        if let Err(clear_err) = fs::remove_dir_all(&store_path).await {
                            tracing::error!("Failed to clear store directory {}: {}", store_path, clear_err);
                        }
                        sleep(Duration::from_millis(500)).await; // Small delay before recreation
                        if let Err(create_err) = fs::create_dir_all(&store_path).await {
                            tracing::error!("Failed to recreate store directory {}: {}", store_path, create_err);
                        }
                        tracing::info!("Cleared store directory: {}", store_path);
                    }
                 
                    // Add delay before retry
                    sleep(RETRY_DELAY).await;
                 
                    // Reinitialize client (bypass cache since we're recovering from an error)
                    tracing::debug!("Reinitializing client for user {}", user_id);
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
async fn connect_instagram(
    client: &MatrixClient,
    bridge_bot: &str,
    curl_paste: &str,
) -> Result<OwnedRoomId> {
    tracing::debug!("🚀 Starting Instagram connection process");
 
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;
 
    let request = CreateRoomRequest::new();
    tracing::debug!("Creating new room");
    let response = client.create_room(request).await?;
    let room_id = response.room_id();
    tracing::debug!("🏠 Created room with ID: {}", room_id);
 
    let room = client.get_room(&room_id).ok_or(anyhow!("Room not found"))?;
 
    tracing::debug!("🤖 Inviting bot user: {}", bot_user_id);
    room.invite_user_by_id(&bot_user_id).await?;
    tracing::info!("Bot invited to room {}", room_id);
 
    // Single sync to get the invitation processed
    tracing::debug!("Performing single sync to process invitation");
    client.sync_once(MatrixSyncSettings::default().timeout(Duration::from_secs(5))).await?;
 
    // Reduced wait time and more frequent checks
    let mut attempt = 0;
    for _ in 0..15 { // Reduced from 30 to 15
        attempt += 1;
        tracing::debug!("🔍 Check attempt {}/15 for bot join status", attempt);
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
        tracing::error!("❌ Bot failed to join room after all attempts");
        return Err(anyhow!("Bot {} failed to join room", bot_user_id));
    }
    // Send login command
    let login_command = "login instagram".to_string();
    tracing::info!("📤 Sending Instagram login command: {}", login_command);
    room.send(RoomMessageEventContent::text_plain(&login_command)).await?;
 
    // Small delay to ensure the bot processes the login command
    sleep(Duration::from_secs(1)).await;
 
    // Send the cURL paste
    tracing::info!("📤 Sending cURL paste for authentication (length: {})", curl_paste.len());
    room.send(RoomMessageEventContent::text_plain(curl_paste)).await?;
 
    Ok(room_id.into())
}
#[derive(Deserialize)]
pub struct InstagramLoginRequest {
    curl_paste: String,
}
#[derive(Serialize)]
pub struct InstagramConnectionResponse {
    message: String,
}
pub async fn start_instagram_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    AxumJson(req): AxumJson<InstagramLoginRequest>,
) -> Result<AxumJson<InstagramConnectionResponse>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!("Received POST request to start Instagram connection for user {}", auth_user.user_id);
    tracing::debug!("Request body: curl_paste length = {}", req.curl_paste.len());
    tracing::debug!("🚀 Starting Instagram connection process for user {}", auth_user.user_id);
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
    let bridge_bot = std::env::var("INSTAGRAM_BRIDGE_BOT")
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Server configuration error: missing INSTAGRAM_BRIDGE_BOT"})),
        ))?;
    tracing::debug!("🔗 Connecting to Instagram bridge...");
    // Connect to Instagram bridge
    let mut client_clone = Arc::clone(&client);
    let room_id = connect_instagram_with_retry(
        &mut client_clone,
        &bridge_bot,
        &req.curl_paste,
        auth_user.user_id,
        &state,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to connect to Instagram bridge: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": format!("Failed to connect to Instagram bridge: {}", e)})),
        )
    })?;
    // Create bridge record
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    let new_bridge = NewBridge {
        user_id: auth_user.user_id,
        bridge_type: "instagram".to_string(),
        status: "connecting".to_string(),
        room_id: Some(room_id.to_string()),
        data: None,
        created_at: Some(current_time),
    };
    tracing::debug!("Storing new bridge record for Instagram");
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
        tracing::debug!("Spawning monitor task for Instagram connection for user {}", auth_user.user_id);
        match monitor_instagram_connection(
            &client_clone,
            &room_id_clone,
            &bridge_bot_clone,
            auth_user.user_id,
            state_clone,
        ).await {
            Ok(_) => {
                tracing::info!("Instagram connection monitoring completed successfully for user {}", auth_user.user_id);
            },
            Err(e) => {
                tracing::error!("Instagram connection monitoring failed for user {}: {}", auth_user.user_id, e);
            }
        }
    });
    tracing::info!("Instagram connection process initiated successfully for user {}", auth_user.user_id);
    Ok(AxumJson(InstagramConnectionResponse { message: "Login process started, awaiting confirmation".to_string() }))
}
pub async fn get_instagram_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::debug!("📊 Checking Instagram status for user {}", auth_user.user_id);
    let bridge = state.user_repository.get_bridge(auth_user.user_id, "instagram")
        .map_err(|e| {
            tracing::error!("Failed to get Instagram bridge status: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Instagram status"})),
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
async fn monitor_instagram_connection(
    client: &MatrixClient,
    room_id: &OwnedRoomId,
    bridge_bot: &str,
    user_id: i32,
    state: Arc<AppState>,
) -> Result<(), anyhow::Error> {
    tracing::debug!("👀 Starting optimized Instagram connection monitoring for user {} in room {}", user_id, room_id);
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;
    // Shorter sync timeout for faster response
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(10));
    // Reduced monitoring duration but more frequent checks
    for attempt in 1..60 { // Try for about 5 minutes (60 * 5 seconds)
        tracing::debug!("🔄 Monitoring attempt #{} for user {}", attempt, user_id);
        match client.sync_once(sync_settings.clone()).await {
            Ok(_) => tracing::debug!("Sync successful on attempt {} for user {}", attempt, user_id),
            Err(e) => tracing::warn!("Sync failed on attempt {} for user {}: {}", attempt, user_id, e),
        }
     
        if let Some(room) = client.get_room(room_id) {
            // Get only recent messages to reduce processing time
            let mut options = matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            options.limit = matrix_sdk::ruma::UInt::new(5).unwrap(); // Reduced from default to 5
            tracing::debug!("Fetching recent messages from room {}", room_id);
            let messages = room.messages(options).await?;
            tracing::debug!("Fetched {} messages from room {}", messages.chunk.len(), room_id);
         
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
                            tracing::debug!("Received message content from bot: {}", content);
                            // Check for successful login message first
                            if content.to_lowercase().contains("successful login") {
                                tracing::info!("🎉 Instagram successfully connected for user {}", user_id);
                             
                                // Update bridge status to connected
                                let current_time = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs() as i32;
                                let new_bridge = NewBridge {
                                    user_id,
                                    bridge_type: "instagram".to_string(),
                                    status: "connected".to_string(),
                                    room_id: Some(room_id.to_string()),
                                    data: None,
                                    created_at: Some(current_time),
                                };
                                tracing::debug!("Deleting existing bridge record for Instagram");
                                state.user_repository.delete_bridge(user_id, "instagram")?;
                                tracing::debug!("Creating new connected bridge record for Instagram");
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
                                // No specific sync commands for Instagram, as portals are created on message receipt
                                return Ok(());
                            }
                            // Check for error messages with more specific patterns
                            let error_patterns = [
                                "error",
                                "failed",
                                "timeout",
                                "disconnected",
                                "invalid",
                                "connection lost",
                                "authentication failed",
                                "login failed"
                            ];
                            if error_patterns.iter().any(|&pattern| content.to_lowercase().contains(pattern)) {
                                tracing::error!("❌ Instagram connection failed for user {}: {}", user_id, content);
                                state.user_repository.delete_bridge(user_id, "instagram")?;
                                return Err(anyhow!("Instagram connection failed: {}", content));
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
    tracing::error!("Instagram connection timed out for user {} after 3 minutes", user_id);
    state.user_repository.delete_bridge(user_id, "instagram")?;
    Err(anyhow!("Instagram connection timed out after 3 minutes"))
}
pub async fn resync_instagram(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!("🔄 Starting Instagram resync process for user {}", auth_user.user_id);
    // Get the bridge information first
    let bridge = state.user_repository.get_bridge(auth_user.user_id, "instagram")
        .map_err(|e| {
            tracing::error!("Failed to get Instagram bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Instagram bridge info"})),
            )
        })?;
    let Some(bridge) = bridge else {
        return Err((
            StatusCode::BAD_REQUEST,
            AxumJson(json!({"error": "Instagram is not connected"})),
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
        tracing::debug!("📱 Setting up Matrix event handler");
     
        // Set up event handler for the Matrix client
        client.add_event_handler(|ev: SyncRoomMessageEvent| async move {
            match ev {
                SyncRoomMessageEvent::Original(msg) => {
                    tracing::debug!("Received original message event: {:?}", msg);
                    // Add more specific message handling logic here if needed
                },
                SyncRoomMessageEvent::Redacted(_) => {
                    tracing::debug!("🗑️ Received redacted message event");
                }
            }
        });
        // Start continuous sync in the background
        let sync_client = client.clone();
        tokio::spawn(async move {
            tracing::info!("🔄 Starting continuous Matrix sync for Instagram bridge");
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
        tracing::debug!("📱 No specific resync commands for Instagram, as portals are created dynamically");
     
        tracing::debug!("✅ Instagram resync process completed for user {}", auth_user.user_id);
        Ok(AxumJson(json!({
            "message": "Instagram resync initiated successfully (no-op)"
        })))
    } else {
        tracing::error!("Instagram bridge room not found for user {}", auth_user.user_id);
        Err((
            StatusCode::NOT_FOUND,
            AxumJson(json!({"error": "Instagram bridge room not found"})),
        ))
    }
}
pub async fn disconnect_instagram(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::debug!("🔌 Starting Instagram disconnection process for user {}", auth_user.user_id);
    // Get the bridge information first
    let bridge = state.user_repository.get_bridge(auth_user.user_id, "instagram")
        .map_err(|e| {
            tracing::error!("Failed to get Instagram bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Instagram bridge info"})),
            )
        })?;
    let Some(bridge) = bridge else {
        tracing::info!("Instagram was not connected for user {}", auth_user.user_id);
        return Ok(AxumJson(json!({
            "message": "Instagram was not connected"
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
        tracing::debug!("📤 Sending Instagram logout command");
        // Send logout command
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("logout")).await {
            tracing::error!("Failed to send logout command: {}", e);
        }
        // Wait a moment for the logout to process
        sleep(Duration::from_secs(5)).await;
        // Attempt delete-all-portals if applicable
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("delete-all-portals")).await {
            tracing::warn!("Failed to send delete-portals command (may not exist): {}", e);
        }
        // Wait a moment for the cleanup to process
        sleep(Duration::from_secs(5)).await;
        tracing::debug!("🗑️ Sending delete-session command");
        // Send delete-session command as a final cleanup
        if let Err(e) = room.send(RoomMessageEventContent::text_plain("delete-session")).await {
            tracing::warn!("Failed to send delete-session command (may not exist): {}", e);
        }
        sleep(Duration::from_secs(5)).await;
    } else {
        tracing::warn!("Instagram bridge room not found during disconnection for user {}", auth_user.user_id);
    }
    // Delete the bridge record
    tracing::debug!("Deleting Instagram bridge record for user {}", auth_user.user_id);
    state.user_repository.delete_bridge(auth_user.user_id, "instagram")
        .map_err(|e| {
            tracing::error!("Failed to delete Instagram bridge: {}", e);
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
    tracing::debug!("✅ Instagram disconnection completed for user {}", auth_user.user_id);
    Ok(AxumJson(json!({
        "message": "Instagram disconnected successfully"
    })))
}
