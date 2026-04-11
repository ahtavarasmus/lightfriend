use crate::{
    handlers::auth_middleware::AuthUser, pg_models::NewPgBridge, utils::matrix_auth, AppState,
};
use anyhow::{anyhow, Result};
use axum::{extract::State, http::StatusCode, response::Json as AxumJson};
use matrix_sdk::{
    config::SyncSettings as MatrixSyncSettings,
    ruma::{
        api::client::room::create_room::v3::Request as CreateRoomRequest,
        events::room::message::{MessageType, RoomMessageEventContent, SyncRoomMessageEvent},
        events::AnySyncTimelineEvent,
        OwnedRoomId, OwnedUserId,
    },
    Client as MatrixClient,
};
use serde::Serialize;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::time::{sleep, Duration};

/// Count Signal bridge rooms for a user (to detect stale state)
async fn count_signal_rooms(client: &MatrixClient, bridge_bot: &str) -> Result<u32> {
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;
    let mut count = 0;

    for room in client.joined_rooms() {
        let members = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
            Ok(m) => m,
            Err(_) => continue,
        };
        if members.iter().any(|m| m.user_id() == bot_user_id) {
            count += 1;
        }
    }

    Ok(count)
}

/// Clean up all Signal bridge rooms for a user
/// This includes the management room and all portal rooms created by the bridge
async fn cleanup_all_signal_rooms(client: &MatrixClient, bridge_bot: &str) -> Result<u32> {
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;
    let mut rooms_left = 0;

    // Get all rooms the user is in
    let rooms = client.joined_rooms();
    tracing::info!("🔍 Checking {} rooms for Signal cleanup", rooms.len());

    for room in rooms {
        // Check if this room has the Signal bridge bot
        let members = match room.members(matrix_sdk::RoomMemberships::JOIN).await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("Failed to get members for room {}: {}", room.room_id(), e);
                continue;
            }
        };

        let has_signal_bot = members.iter().any(|m| m.user_id() == bot_user_id);

        if has_signal_bot {
            tracing::info!("🧹 Leaving Signal room: {}", room.room_id());
            if let Err(e) = room.leave().await {
                tracing::warn!("Failed to leave room {}: {}", room.room_id(), e);
            } else {
                rooms_left += 1;
            }
        }
    }

    tracing::info!("✅ Left {} Signal-related rooms", rooms_left);
    Ok(rooms_left)
}

// Helper function to detect the one-time key conflict error
fn is_one_time_key_conflict(error: &anyhow::Error) -> bool {
    if let Some(http_err) = error.downcast_ref::<matrix_sdk::HttpError>() {
        let error_str = http_err.to_string();
        return error_str.contains("One time key") && error_str.contains("already exists");
    }
    false
}

/// Extract connected account (phone number) from bridge status message
fn extract_connected_account(message: &str) -> Option<String> {
    // Look for phone number pattern (starts with + followed by digits)
    let re = regex::Regex::new(r"\+\d{6,15}").ok()?;
    re.find(message).map(|m| m.as_str().to_string())
}

// Helper function to get the store path
fn get_store_path(username: &str) -> Result<String> {
    let persistent_store_path = std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
        .map_err(|_| anyhow!("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"))?;
    Ok(format!("{}/{}", persistent_store_path, username))
}
// Wrapper function with retry logic
async fn connect_signal_with_retry(
    client: &mut Arc<MatrixClient>,
    bridge_bot: &str,
    user_id: i32,
    state: &Arc<AppState>,
) -> Result<(OwnedRoomId, String)> {
    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY: Duration = Duration::from_secs(2);

    let username = client
        .user_id()
        .ok_or_else(|| anyhow!("User ID not available"))?
        .localpart()
        .to_string();
    for retry_count in 0..MAX_RETRIES {
        match connect_signal(client, bridge_bot).await {
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

                    // Reinitialize client
                    match matrix_auth::get_cached_client(user_id, state).await {
                        Ok(new_client) => {
                            *client = new_client; // Update the client reference
                            tracing::info!("Client reinitialized, retrying operation");
                            continue;
                        }
                        Err(init_err) => {
                            tracing::error!("Failed to reinitialize client: {}", init_err);
                            return Err(init_err);
                        }
                    }
                } else if is_one_time_key_conflict(&e) {
                    return Err(anyhow!(
                        "Failed after {} attempts to resolve one-time key conflict: {}",
                        MAX_RETRIES,
                        e
                    ));
                } else {
                    return Err(e);
                }
            }
        }
    }

    Err(anyhow!("Exceeded maximum retry attempts ({})", MAX_RETRIES))
}
#[derive(Serialize)]
pub struct SignalConnectionResponse {
    qr_code_url: String,
}
async fn connect_signal(client: &MatrixClient, bridge_bot: &str) -> Result<(OwnedRoomId, String)> {
    tracing::debug!("🚀 Starting Signal connection process");

    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;

    let request = CreateRoomRequest::new();
    let response = client.create_room(request).await?;
    let room_id = response.room_id();
    tracing::debug!("🏠 Created room with ID: {}", room_id);

    let room = client.get_room(room_id).ok_or(anyhow!("Room not found"))?;

    tracing::debug!("🤖 Inviting bot user: {}", bot_user_id);
    room.invite_user_by_id(&bot_user_id).await?;

    // Single sync to get the invitation processed
    client
        .sync_once(MatrixSyncSettings::default().timeout(Duration::from_secs(5)))
        .await?;

    // Reduced wait time and more frequent checks
    let mut attempt = 0;
    for _ in 0..15 {
        // Reduced from 30 to 15
        attempt += 1;
        tracing::debug!("Check attempt {}/15 for bot join status", attempt);
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
        tracing::debug!("Bot failed to join room after all attempts");
        return Err(anyhow!("Bot {} failed to join room", bot_user_id));
    }
    // Send login command
    let login_command = "!signal login".to_string();
    tracing::info!("📤 Sending Signal login command: {}", login_command);
    room.send(RoomMessageEventContent::text_plain(&login_command))
        .await?;
    // Optimized QR code detection with event handler
    let mut qr_code_url = None;
    let mut saw_scan_text = false;
    tracing::info!("⏳ Starting QR code monitoring");

    // Use shorter sync timeout for more frequent checks (matching WhatsApp's proven approach)
    // More frequent syncs catch the image message more reliably
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_millis(1500));
    for attempt in 1..=60 {
        // 60 attempts * ~2s = ~2 minutes max wait
        tracing::debug!("Sync attempt #{}/60", attempt);

        // Don't fail on sync errors - just log and retry
        match client.sync_once(sync_settings.clone()).await {
            Ok(_) => tracing::debug!("Sync completed successfully"),
            Err(e) => {
                tracing::warn!("Sync attempt {} failed: {}, retrying...", attempt, e);
                sleep(Duration::from_secs(1)).await;
                continue;
            }
        }

        if let Some(room) = client.get_room(room_id) {
            // Get only the most recent messages to reduce processing time
            let mut options =
                matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            options.limit = matrix_sdk::ruma::UInt::new(10).unwrap(); // Check more messages
            let messages = match room.messages(options).await {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("Failed to get messages: {}, retrying...", e);
                    continue;
                }
            };

            for msg in messages.chunk.iter() {
                let raw_event = msg.raw();
                if let Ok(event) = raw_event.deserialize() {
                    if event.sender() == bot_user_id {
                        if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(
                                sync_event,
                            ),
                        ) = event.clone()
                        {
                            let event_content: RoomMessageEventContent = match sync_event {
                                SyncRoomMessageEvent::Original(original_event) => {
                                    original_event.content
                                }
                                SyncRoomMessageEvent::Redacted(_) => {
                                    continue;
                                }
                            };
                            match event_content.msgtype {
                                MessageType::Image(image_content) => {
                                    tracing::info!("Received Image message (Signal QR code)");
                                    if let matrix_sdk::ruma::events::room::MediaSource::Plain(url) =
                                        &image_content.source
                                    {
                                        qr_code_url = Some(url.to_string());
                                        tracing::info!("Found Signal QR code URL for linking");
                                        break;
                                    } else {
                                        // Handle unexpected encrypted case, e.g., log an error and continue
                                        tracing::error!("Unexpected encrypted QR code");
                                    }
                                }
                                MessageType::Notice(text_content) => {
                                    tracing::info!(
                                        "Received Notice message: {}",
                                        text_content.body
                                    );
                                    // Check for errors or other notices
                                    if text_content.body.contains("error") {
                                        return Err(anyhow!(
                                            "Error from bot: {}",
                                            text_content.body
                                        ));
                                    }
                                }
                                MessageType::Text(text_content) => {
                                    tracing::info!(
                                        "📝 Received Text from bot: {}",
                                        text_content.body
                                    );
                                    // Track when we see the QR code prompt - image should follow
                                    if text_content.body.contains("Scan the QR code") {
                                        saw_scan_text = true;
                                        tracing::info!(
                                            "✅ Detected QR prompt, image should follow..."
                                        );
                                    }
                                    // Check for errors or other texts
                                    if text_content.body.contains("error") {
                                        return Err(anyhow!(
                                            "Error from bot: {}",
                                            text_content.body
                                        ));
                                    }
                                }
                                _ => {
                                    tracing::info!(
                                        "Received other message type: {:#?}",
                                        event_content.msgtype
                                    );
                                    continue;
                                }
                            };
                        }
                    }
                }
            }
            if qr_code_url.is_some() {
                break;
            }
        }
        // Balanced delay
        sleep(Duration::from_millis(500)).await;
    }

    // Provide more specific error message if we saw the text prompt but never got the image
    let qr_code_url = if let Some(url) = qr_code_url {
        url
    } else if saw_scan_text {
        return Err(anyhow!(
            "Signal bridge sent QR prompt but image was not received. Please try again."
        ));
    } else {
        return Err(anyhow!(
            "Signal QR code not received within timeout. Please try again."
        ));
    };
    Ok((room_id.into(), qr_code_url))
}

use base64::engine::general_purpose::STANDARD as Base64Engine;
use base64::Engine;
use matrix_sdk::media::MediaFormat;
use matrix_sdk::media::MediaRequestParameters;
use matrix_sdk::ruma::events::room::MediaSource;

pub async fn start_signal_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<SignalConnectionResponse>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!(
        "🚀 Signal connect request received for user {}",
        auth_user.user_id
    );

    // Check if there's an existing bridge - block if cleanup is in progress
    if let Ok(Some(existing_bridge)) = state
        .user_repository
        .get_bridge(auth_user.user_id, "signal")
    {
        if existing_bridge.status == "cleaning_up" {
            tracing::info!(
                "⏳ Signal cleanup in progress for user {}, blocking connect",
                auth_user.user_id
            );
            return Err((
                StatusCode::CONFLICT,
                AxumJson(json!({
                    "error": "cleanup_in_progress",
                    "message": "Please wait, cleaning up previous connection..."
                })),
            ));
        }
        // If there's an existing connected/connecting bridge, delete it first for fresh start
        if existing_bridge.status == "connected" || existing_bridge.status == "connecting" {
            tracing::info!("🧹 Removing existing Signal bridge for fresh start");
            let _ = state
                .user_repository
                .delete_bridge(auth_user.user_id, "signal");
        }
    }

    // Remove any cached Matrix client to ensure fresh state
    {
        let mut matrix_clients = state.matrix_clients.lock().await;
        let mut sync_tasks = state.matrix_sync_tasks.lock().await;

        if let Some(task) = sync_tasks.remove(&auth_user.user_id) {
            task.abort();
            tracing::debug!("Aborted existing sync task for fresh connect");
        }
        if matrix_clients.remove(&auth_user.user_id).is_some() {
            tracing::debug!("Removed cached Matrix client for fresh connect");
        }
    }

    tracing::info!("📝 Getting fresh Matrix client...");
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
    tracing::debug!(
        "✅ Matrix client obtained for user: {}",
        client.user_id().unwrap()
    );

    // Get bridge bot from environment
    let bridge_bot = std::env::var("SIGNAL_BRIDGE_BOT").expect("SIGNAL_BRIDGE_BOT not set");

    // Check for stale Signal rooms (rooms with Signal bot but no bridge record)
    // This can happen if a previous connection failed or was partially cleaned up
    let stale_room_count = count_signal_rooms(&client, &bridge_bot).await.unwrap_or(0);
    if stale_room_count > 0 {
        tracing::info!(
            "🧹 Found {} stale Signal rooms for user {}, starting cleanup",
            stale_room_count,
            auth_user.user_id
        );

        // Create a temporary "cleaning_up" bridge record to track state
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;
        let new_bridge = NewPgBridge {
            user_id: auth_user.user_id,
            bridge_type: "signal".to_string(),
            status: "cleaning_up".to_string(),
            room_id: None,
            data: None,
            created_at: Some(current_time),
        };
        let _ = state.user_repository.create_bridge(new_bridge);

        // Spawn cleanup task
        let state_clone = state.clone();
        let user_id = auth_user.user_id;
        let bridge_bot_clone = bridge_bot.clone();
        let client_clone = Arc::clone(&client);
        tokio::spawn(async move {
            tracing::info!("🧹 Starting stale room cleanup for user {}", user_id);

            // Clean up all Signal rooms
            match cleanup_all_signal_rooms(&client_clone, &bridge_bot_clone).await {
                Ok(count) => tracing::info!("Cleaned up {} stale Signal rooms", count),
                Err(e) => tracing::error!("Failed to cleanup stale Signal rooms: {}", e),
            }

            // Remove client from cache
            {
                let mut matrix_clients = state_clone.matrix_clients.lock().await;
                let mut sync_tasks = state_clone.matrix_sync_tasks.lock().await;
                if let Some(task) = sync_tasks.remove(&user_id) {
                    task.abort();
                }
                matrix_clients.remove(&user_id);
            }

            // Check if we should clear the store
            let has_active_bridges = state_clone
                .user_repository
                .has_active_bridges(user_id)
                .unwrap_or(false);
            if !has_active_bridges {
                if let Some(user_id_matrix) = client_clone.user_id() {
                    let username = user_id_matrix.localpart().to_string();
                    if let Ok(store_path) = get_store_path(&username) {
                        if Path::new(&store_path).exists() {
                            let _ = fs::remove_dir_all(&store_path).await;
                            tracing::info!("Cleared Matrix store for user {}", user_id);
                        }
                    }
                }
            }

            // Delete the cleanup bridge record
            let _ = state_clone.user_repository.delete_bridge(user_id, "signal");
            tracing::info!("🧹 Stale room cleanup completed for user {}", user_id);
        });

        // Return cleanup_in_progress - frontend will retry
        return Err((
            StatusCode::CONFLICT,
            AxumJson(json!({
                "error": "cleanup_in_progress",
                "message": "Cleaning up stale data from previous connection..."
            })),
        ));
    }

    tracing::debug!("🔗 Connecting to Signal bridge...");
    // Connect to Signal bridge
    let mut client_clone = Arc::clone(&client);
    let (room_id, qr_code_url) =
        connect_signal_with_retry(&mut client_clone, &bridge_bot, auth_user.user_id, &state)
            .await
            .map_err(|e| {
                tracing::error!("Failed to connect to Signal bridge: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(
                        json!({"error": format!("Failed to connect to Signal bridge: {}", e)}),
                    ),
                )
            })?;

    tracing::info!("📥 Fetching QR code media bytes via SDK...");
    let mxc: matrix_sdk::ruma::OwnedMxcUri = qr_code_url.as_str().into();

    let request = MediaRequestParameters {
        source: MediaSource::Plain(mxc.to_owned()),
        format: MediaFormat::File,
    };

    let bytes = client
        .media()
        .get_media_content(&request, false) // false = don't download if already cached, but irrelevant here
        .await
        .map_err(|e| {
            tracing::error!("Failed to download QR code media: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to fetch QR code image: {}", e)})),
            )
        })?;

    let base64_str = Base64Engine.encode(&bytes);
    let data_url = format!("data:image/png;base64,{}", base64_str);

    if auth_user.user_id == 1 {
        tracing::info!(
            "Generated data URL for QR code (preview: {}...)",
            &data_url[0..50]
        );
    }

    // Replace qr_code_url with data_url for the response
    let qr_code_url = data_url;

    // Create bridge record
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    let new_bridge = NewPgBridge {
        user_id: auth_user.user_id,
        bridge_type: "signal".to_string(),
        status: "connecting".to_string(),
        room_id: Some(room_id.to_string()),
        data: None,
        created_at: Some(current_time),
    };
    // Store bridge information
    state
        .user_repository
        .create_bridge(new_bridge)
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
        match monitor_signal_connection(
            &client_clone,
            &room_id_clone,
            &bridge_bot_clone,
            auth_user.user_id,
            state_clone,
        )
        .await
        {
            Ok(_) => {
                tracing::info!(
                    "Signal connection monitoring completed successfully for user {}",
                    auth_user.user_id
                );
            }
            Err(e) => {
                tracing::error!(
                    "Signal connection monitoring failed for user {}: {}",
                    auth_user.user_id,
                    e
                );
            }
        }
    });
    Ok(AxumJson(SignalConnectionResponse { qr_code_url }))
}

pub async fn get_signal_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::debug!("📊 Checking Signal status for user {}", auth_user.user_id);
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "signal")
        .map_err(|e| {
            tracing::error!("Failed to get Signal bridge status: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Signal status"})),
            )
        })?;
    match bridge {
        Some(bridge) => Ok(AxumJson(json!({
            "connected": bridge.status == "connected",
            "status": bridge.status,
            "created_at": bridge.created_at.unwrap_or(0),
            "connected_account": bridge.data,
        }))),
        None => Ok(AxumJson(json!({
            "connected": false,
            "status": "not_connected",
            "created_at": 0,
            "connected_account": null,
        }))),
    }
}

async fn monitor_signal_connection(
    client: &MatrixClient,
    room_id: &OwnedRoomId,
    bridge_bot: &str,
    user_id: i32,
    state: Arc<AppState>,
) -> Result<(), anyhow::Error> {
    tracing::debug!(
        "👀 Starting optimized Signal connection monitoring for user {} in room {}",
        user_id,
        room_id
    );
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;
    // Shorter sync timeout for faster response
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(10));
    // Reduced monitoring duration but more frequent checks
    for attempt in 1..60 {
        // Try for about 5 minutes (60 * 5 seconds)
        tracing::debug!("🔄 Monitoring attempt #{} for user {}", attempt, user_id);
        let _ = client.sync_once(sync_settings.clone()).await?;

        if let Some(room) = client.get_room(room_id) {
            // Get only recent messages to reduce processing time
            let mut options =
                matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            options.limit = matrix_sdk::ruma::UInt::new(5).unwrap(); // Reduced from default to 5
            let messages = room.messages(options).await?;

            for msg in messages.chunk {
                let raw_event = msg.raw();
                if let Ok(event) = raw_event.deserialize() {
                    if event.sender() == bot_user_id {
                        if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(
                                sync_event,
                            ),
                        ) = event
                        {
                            let event_content: RoomMessageEventContent = match sync_event {
                                SyncRoomMessageEvent::Original(original_event) => {
                                    original_event.content
                                }
                                SyncRoomMessageEvent::Redacted(_) => continue,
                            };
                            let content = match event_content.msgtype {
                                MessageType::Text(text_content) => text_content.body,
                                MessageType::Notice(notice_content) => notice_content.body,
                                _ => continue,
                            };
                            // Check for successful login message first
                            if content.contains("Successfully logged in") {
                                tracing::info!(
                                    "🎉 Signal successfully connected for user {}",
                                    user_id
                                );

                                // Extract the connected account (phone number)
                                let connected_account = extract_connected_account(&content);
                                if connected_account.is_some() {
                                    tracing::info!("📱 Connected as: [redacted]");
                                }

                                // Update bridge status to connected
                                let current_time = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs()
                                    as i32;
                                let new_bridge = NewPgBridge {
                                    user_id,
                                    bridge_type: "signal".to_string(),
                                    status: "connected".to_string(),
                                    room_id: Some(room_id.to_string()),
                                    data: connected_account,
                                    created_at: Some(current_time),
                                };
                                state.user_repository.delete_bridge(user_id, "signal")?;
                                state.user_repository.create_bridge(new_bridge)?;

                                // Bridge-level E2EE is unnecessary: both the Matrix server and bridges run
                                // inside the enclave, so data never leaves unencrypted.

                                // Add client to app state and start sync
                                let mut matrix_clients = state.matrix_clients.lock().await;
                                let mut sync_tasks = state.matrix_sync_tasks.lock().await;
                                // Add event handlers before storing/cloning the client
                                use matrix_sdk::room::Room;
                                use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;

                                let state_for_handler = Arc::clone(&state);
                                client.add_event_handler(
                                    move |ev: OriginalSyncRoomMessageEvent, room: Room, client| {
                                        let state = Arc::clone(&state_for_handler);
                                        async move {
                                            tracing::debug!(
                                                "📨 Received message in room {}: {:?}",
                                                room.room_id(),
                                                ev
                                            );
                                            crate::utils::bridge::handle_bridge_message(
                                                ev, room, client, state,
                                            )
                                            .await;
                                        }
                                    },
                                );
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
                                                tracing::debug!(
                                                    "Sync completed normally for user {}",
                                                    user_id
                                                );
                                                tokio::time::sleep(
                                                    tokio::time::Duration::from_secs(1),
                                                )
                                                .await;
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "Matrix sync error for user {}: {}",
                                                    user_id,
                                                    e
                                                );
                                                tokio::time::sleep(
                                                    tokio::time::Duration::from_secs(30),
                                                )
                                                .await;
                                            }
                                        }
                                    }
                                });
                                // Abort old sync task to prevent duplicate message processing
                                if let Some(old_task) = sync_tasks.remove(&user_id) {
                                    old_task.abort();
                                }
                                sync_tasks.insert(user_id, handle);
                                // No specific sync commands for Signal, as portals are created on message receipt
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
                                "login failed",
                            ];
                            if error_patterns
                                .iter()
                                .any(|&pattern| content.to_lowercase().contains(pattern))
                            {
                                tracing::error!(
                                    "❌ Signal connection failed for user {}: {}",
                                    user_id,
                                    content
                                );
                                state.user_repository.delete_bridge(user_id, "signal")?;
                                return Err(anyhow!("Signal connection failed: {}", content));
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
    state.user_repository.delete_bridge(user_id, "signal")?;
    Err(anyhow!("Signal connection timed out after 3 minutes"))
}

pub async fn resync_signal(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::debug!(
        "Starting Signal resync process for user {}",
        auth_user.user_id
    );
    // Get the bridge information first
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "signal")
        .map_err(|e| {
            tracing::error!("Failed to get Signal bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Signal bridge info"})),
            )
        })?;
    let Some(bridge) = bridge else {
        return Err((
            StatusCode::BAD_REQUEST,
            AxumJson(json!({"error": "Signal is not connected"})),
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
    let room_id = OwnedRoomId::try_from(bridge.room_id.unwrap_or_default()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Invalid room ID format"})),
        )
    })?;
    if let Some(room) = client.get_room(&room_id) {
        tracing::debug!("Setting up Matrix event handler");

        // Set up event handler for the Matrix client
        client.add_event_handler(|ev: SyncRoomMessageEvent| async move {
            match ev {
                SyncRoomMessageEvent::Original(_msg) => {
                    // Add more specific message handling logic here if needed
                }
                SyncRoomMessageEvent::Redacted(_) => {
                    tracing::debug!("Received redacted message event");
                }
            }
        });

        // Send the clean-rooms command to the management room
        let content = RoomMessageEventContent::text_plain("!signal clean-rooms");
        room.send(content).await.map_err(|e| {
            tracing::error!("Failed to send clean-rooms command: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to send clean-rooms command"})),
            )
        })?;
        tracing::info!("🧹 Sent clean-rooms command to Signal bridge management room");

        // Start continuous sync in the background
        let sync_client = client.clone();
        tokio::spawn(async move {
            tracing::info!("🔄 Starting continuous Matrix sync for Signal bridge");
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
        tracing::debug!(
            "📱 No specific resync commands for Signal, as portals are created dynamically"
        );

        tracing::debug!(
            "✅ Signal resync process completed for user {}",
            auth_user.user_id
        );
        Ok(AxumJson(json!({
            "message": "Signal resync initiated successfully (no-op)"
        })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            AxumJson(json!({"error": "Signal bridge room not found"})),
        ))
    }
}

pub async fn disconnect_signal(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!(
        "🔌 Starting Signal disconnection process for user {}",
        auth_user.user_id
    );

    // Get the bridge information first
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "signal")
        .map_err(|e| {
            tracing::error!("Failed to get Signal bridge info: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Signal bridge info"})),
            )
        })?;

    let Some(bridge) = bridge else {
        return Ok(AxumJson(json!({
            "message": "Signal was not connected"
        })));
    };

    // If already cleaning up, just return success
    if bridge.status == "cleaning_up" {
        return Ok(AxumJson(json!({
            "message": "Signal cleanup already in progress"
        })));
    }

    let room_id_str = bridge.room_id.clone().unwrap_or_default();

    // Set bridge status to "cleaning_up" instead of deleting immediately
    // This allows us to block reconnection until cleanup is complete
    state
        .user_repository
        .update_bridge_status(auth_user.user_id, "signal", "cleaning_up")
        .map_err(|e| {
            tracing::error!("Failed to update Signal bridge status: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to update bridge status"})),
            )
        })?;

    tracing::info!(
        "✅ Signal bridge status set to 'cleaning_up' for user {}",
        auth_user.user_id
    );

    // Spawn background task for cleanup - won't be cancelled on page refresh
    let state_clone = state.clone();
    let user_id = auth_user.user_id;
    tokio::spawn(async move {
        tracing::info!("🧹 Starting background cleanup for Signal user {}", user_id);

        // Get Matrix client for cleanup
        let client = match matrix_auth::get_cached_client(user_id, &state_clone).await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Background cleanup: Failed to get Matrix client: {}", e);
                // Still delete the bridge record so user can retry
                let _ = state_clone.user_repository.delete_bridge(user_id, "signal");
                return;
            }
        };

        // Get the room and send cleanup commands
        if let Ok(room_id) = OwnedRoomId::try_from(room_id_str.as_str()) {
            if let Some(room) = client.get_room(&room_id) {
                // Send logout command
                if let Err(e) = room
                    .send(RoomMessageEventContent::text_plain("!signal logout"))
                    .await
                {
                    tracing::error!("Background cleanup: Failed to send logout command: {}", e);
                }
                sleep(Duration::from_secs(2)).await;

                // Send delete-all-portals
                if let Err(e) = room
                    .send(RoomMessageEventContent::text_plain(
                        "!signal delete-all-portals",
                    ))
                    .await
                {
                    tracing::error!(
                        "Background cleanup: Failed to send delete-all-portals command: {}",
                        e
                    );
                }
                sleep(Duration::from_secs(2)).await;

                // Send clean-rooms for thorough cleanup
                if let Err(e) = room
                    .send(RoomMessageEventContent::text_plain("!signal clean-rooms"))
                    .await
                {
                    tracing::error!(
                        "Background cleanup: Failed to send clean-rooms command: {}",
                        e
                    );
                }
                sleep(Duration::from_secs(2)).await;

                // Send delete-session command as a final cleanup
                if let Err(e) = room
                    .send(RoomMessageEventContent::text_plain(
                        "!signal delete-session",
                    ))
                    .await
                {
                    tracing::error!(
                        "Background cleanup: Failed to send delete-session command: {}",
                        e
                    );
                }
                sleep(Duration::from_secs(2)).await;

                // Explicitly leave the management room
                if let Err(e) = room.leave().await {
                    tracing::error!(
                        "Background cleanup: Failed to leave Signal management room: {}",
                        e
                    );
                } else {
                    tracing::info!("Background cleanup: Left Signal bridge management room");
                }
            }
        }

        // Clean up ALL Signal-related rooms (portal rooms created by the bridge)
        let bridge_bot = std::env::var("SIGNAL_BRIDGE_BOT").unwrap_or_default();
        if !bridge_bot.is_empty() {
            // Wait for bridge cleanup commands to complete
            sleep(Duration::from_secs(3)).await;
            match cleanup_all_signal_rooms(&client, &bridge_bot).await {
                Ok(count) => {
                    tracing::info!("Background cleanup: Left {} Signal portal rooms", count)
                }
                Err(e) => {
                    tracing::error!("Background cleanup: Failed to cleanup Signal rooms: {}", e)
                }
            }
        }

        // Remove client and sync task BEFORE checking other bridges
        // This ensures fresh state for reconnection
        {
            let mut matrix_clients = state_clone.matrix_clients.lock().await;
            let mut sync_tasks = state_clone.matrix_sync_tasks.lock().await;

            if let Some(task) = sync_tasks.remove(&user_id) {
                task.abort();
                tracing::debug!("Background cleanup: Aborted sync task for user {}", user_id);
            }
            if matrix_clients.remove(&user_id).is_some() {
                tracing::debug!(
                    "Background cleanup: Removed Matrix client for user {}",
                    user_id
                );
            }
        }

        // Check for remaining active bridges and cleanup store if none left
        let has_active_bridges = state_clone
            .user_repository
            .has_active_bridges(user_id)
            .unwrap_or(false);

        if !has_active_bridges {
            // Clear user store if no other bridges
            if let Some(user_id_matrix) = client.user_id() {
                let username = user_id_matrix.localpart().to_string();
                let store_path = match get_store_path(&username) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!("Background cleanup: Failed to get store path: {}", e);
                        // Still delete the bridge record
                        let _ = state_clone.user_repository.delete_bridge(user_id, "signal");
                        return;
                    }
                };
                if Path::new(&store_path).exists() {
                    if let Err(e) = fs::remove_dir_all(&store_path).await {
                        tracing::error!("Background cleanup: Failed to clear user store: {}", e);
                    } else {
                        tracing::info!(
                            "Background cleanup: Cleared Matrix store for user {}",
                            user_id
                        );
                    }
                }
            }
        }

        // Finally, delete the bridge record - cleanup is complete
        if let Err(e) = state_clone.user_repository.delete_bridge(user_id, "signal") {
            tracing::error!("Background cleanup: Failed to delete bridge record: {}", e);
        } else {
            tracing::info!(
                "🧹 Background cleanup completed for Signal user {}",
                user_id
            );
        }
    });

    Ok(AxumJson(json!({
        "message": "Signal disconnecting"
    })))
}

/// Health check endpoint using !signal ping command
/// Returns the actual Signal connection status from the bridge
pub async fn check_signal_health(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!("🏥 Checking Signal health for user {}", auth_user.user_id);

    // Get the bridge information first
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "signal")
        .map_err(|e| {
            tracing::error!("Failed to get Signal bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get Signal bridge info"})),
            )
        })?;

    let Some(bridge) = bridge else {
        return Ok(AxumJson(json!({
            "healthy": false,
            "message": "Signal is not connected"
        })));
    };

    // Get Matrix client
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
    let room_id = OwnedRoomId::try_from(bridge.room_id.unwrap_or_default()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Invalid room ID format"})),
        )
    })?;

    let room = client.get_room(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            AxumJson(json!({"error": "Signal bridge room not found"})),
        )
    })?;

    // Send login command to check status - if already logged in, bridge will respond with status
    tracing::info!(
        "📤 Sending login command for health check for user {}",
        auth_user.user_id
    );
    room.send(RoomMessageEventContent::text_plain("login"))
        .await
        .map_err(|e| {
            tracing::error!("Failed to send ping command: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to send ping command"})),
            )
        })?;

    // Wait for response with timeout
    let bridge_bot = std::env::var("SIGNAL_BRIDGE_BOT").expect("SIGNAL_BRIDGE_BOT not set");
    let bot_user_id = OwnedUserId::try_from(bridge_bot.as_str()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Invalid bridge bot user ID"})),
        )
    })?;

    // Simple approach: check the MOST RECENT message from the bot
    let mut options =
        matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
    options.limit = matrix_sdk::ruma::UInt::new(50).unwrap();

    let messages = match room.messages(options).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("⚠️ Failed to get messages: {}", e);
            return Ok(AxumJson(json!({
                "healthy": false,
                "message": format!("Failed to get room messages: {}", e)
            })));
        }
    };

    tracing::info!(
        "📨 Got {} messages from room for health check",
        messages.chunk.len()
    );

    for msg in messages.chunk {
        let raw_event = msg.raw();
        if let Ok(event) = raw_event.deserialize() {
            if event.sender() == bot_user_id {
                if let AnySyncTimelineEvent::MessageLike(
                    matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(sync_event),
                ) = event
                {
                    let event_content: RoomMessageEventContent = match sync_event {
                        SyncRoomMessageEvent::Original(original_event) => original_event.content,
                        SyncRoomMessageEvent::Redacted(_) => continue,
                    };

                    let content = match event_content.msgtype {
                        MessageType::Text(text_content) => text_content.body,
                        MessageType::Notice(notice_content) => notice_content.body,
                        _ => continue,
                    };

                    let content_lower = content.to_lowercase();
                    tracing::info!("🔍 Most recent bot message ({} chars)", content.len());

                    // Skip non-status messages
                    if content_lower.contains("scan")
                        || content_lower.contains("qr code")
                        || content_lower.contains("queued sync")
                        || content_lower.contains("unknown command")
                    {
                        continue;
                    }

                    // Unhealthy patterns
                    if content_lower.contains("not logged in")
                        || content_lower.contains("not connected")
                        || content_lower.contains("disconnected")
                        || content_lower.contains("logged out")
                        || content_lower.contains("no session")
                        || content_lower.contains("not registered")
                    {
                        tracing::warn!(
                            "❌ Signal health check failed for user {}: {}",
                            auth_user.user_id,
                            content
                        );

                        if let Err(e) = state
                            .user_repository
                            .delete_bridge(auth_user.user_id, "signal")
                        {
                            tracing::error!("Failed to delete stale bridge: {}", e);
                        } else {
                            tracing::info!(
                                "🧹 Deleted stale Signal bridge for user {}",
                                auth_user.user_id
                            );
                        }

                        return Ok(AxumJson(json!({
                            "healthy": false,
                            "message": content
                        })));
                    }

                    // Healthy patterns
                    if content_lower.contains("successfully logged in")
                        || content_lower.contains("logged in as")
                        || content_lower.contains("already logged in")
                    {
                        tracing::info!(
                            "✅ Signal health check passed for user {}: {}",
                            auth_user.user_id,
                            content
                        );

                        // Extract and save the connected account (phone number)
                        if let Some(account) = extract_connected_account(&content) {
                            tracing::info!("📱 Extracted connected account: {}", account);
                            if let Err(e) = state.user_repository.update_bridge_data(
                                auth_user.user_id,
                                "signal",
                                &account,
                            ) {
                                tracing::warn!("Failed to save connected account: {}", e);
                            }
                        }

                        return Ok(AxumJson(json!({
                            "healthy": true,
                            "message": content
                        })));
                    }
                }
            }
        }
    }

    tracing::info!("ℹ️ No clear status message found, assuming healthy based on bridge record");
    Ok(AxumJson(json!({
        "healthy": true,
        "message": "Connection appears healthy (no recent status changes)"
    })))
}
