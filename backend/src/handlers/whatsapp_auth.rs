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
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

use base64::engine::general_purpose::STANDARD as Base64Engine;
use base64::Engine;
use matrix_sdk::media::MediaFormat;
use matrix_sdk::media::MediaRequestParameters;
use matrix_sdk::ruma::events::room::MediaSource;
use std::path::Path;
use tokio::fs;

// Helper function to detect the one-time key conflict error
fn is_one_time_key_conflict(error: &anyhow::Error) -> bool {
    if let Some(http_err) = error.downcast_ref::<matrix_sdk::HttpError>() {
        let error_str = http_err.to_string();
        return error_str.contains("One time key") && error_str.contains("already exists");
    }
    false
}

/// Extract connected account (phone number) from bridge status message
/// Examples:
/// - "Successfully logged in as +358442105886"
/// - "Already logged in as +358442105886"
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

//Helper function to clear the user's Matrix store (reusable for login/logout)
async fn clear_user_store(username: &str) -> Result<()> {
    let store_path = get_store_path(username)?;
    if Path::new(&store_path).exists() {
        fs::remove_dir_all(&store_path).await?;
        sleep(Duration::from_millis(500)).await; // Small delay to ensure filesystem sync
        fs::create_dir_all(&store_path).await?;
        tracing::info!("Cleared Matrix store directory: {}", store_path);
    } else {
        // Create if it doesn't exist (for fresh users)
        fs::create_dir_all(&store_path).await?;
        tracing::info!("Created fresh Matrix store directory: {}", store_path);
    }
    Ok(())
}

// Wrapper function with retry logic
async fn connect_whatsapp_with_retry(
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
        match connect_whatsapp(client, bridge_bot).await {
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
                    clear_user_store(&username).await?; // Use new helper for consistency

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
pub struct WhatsappConnectionResponse {
    qr_code_url: String,
}

async fn connect_whatsapp(
    client: &MatrixClient,
    bridge_bot: &str,
) -> Result<(OwnedRoomId, String)> {
    tracing::debug!("🚀 Starting WhatsApp connection process");

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
    let cancel_command = "!wa cancel".to_string();
    room.send(RoomMessageEventContent::text_plain(&cancel_command))
        .await?;

    // Send login command for QR code
    let login_command = "!wa login qr".to_string();
    tracing::info!("📤 Sending WhatsApp login command: {}", login_command);
    room.send(RoomMessageEventContent::text_plain(&login_command))
        .await?;

    // QR code detection - look for image message from bot
    let mut qr_code_url = None;
    tracing::info!("⏳ Starting QR code monitoring");

    // Use shorter sync timeout for faster response
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_millis(1500));

    for attempt in 1..=60 {
        // Try for about 30 seconds
        println!("📡 Sync attempt #{}/60", attempt);
        client.sync_once(sync_settings.clone()).await?;

        if let Some(room) = client.get_room(room_id) {
            // Get only the most recent messages to reduce processing time
            let mut options =
                matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            options.limit = matrix_sdk::ruma::UInt::new(5).unwrap();
            let messages = room.messages(options).await?;

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
                                    tracing::info!("Received Image message from bot");
                                    if let matrix_sdk::ruma::events::room::MediaSource::Plain(url) =
                                        &image_content.source
                                    {
                                        qr_code_url = Some(url.to_string());
                                        tracing::info!("🔑 Found QR code URL");
                                        break;
                                    } else {
                                        tracing::error!("Unexpected encrypted QR code");
                                    }
                                }
                                MessageType::Notice(text_content) => {
                                    tracing::info!(
                                        "Received Notice message ({} chars)",
                                        text_content.body.len()
                                    );
                                    if text_content.body.to_lowercase().contains("error") {
                                        return Err(anyhow!(
                                            "Error from bot: {}",
                                            text_content.body
                                        ));
                                    }
                                }
                                MessageType::Text(text_content) => {
                                    tracing::info!(
                                        "Received Text message ({} chars)",
                                        text_content.body.len()
                                    );
                                    if text_content.body.to_lowercase().contains("error") {
                                        return Err(anyhow!(
                                            "Error from bot: {}",
                                            text_content.body
                                        ));
                                    }
                                }
                                _ => {
                                    tracing::info!("Received other message type");
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

    let qr_code_url = qr_code_url.ok_or(anyhow!(
        "WhatsApp QR code not received within timeout. Please try again."
    ))?;
    Ok((room_id.into(), qr_code_url))
}

#[derive(Deserialize)]
pub struct WhatsappPhoneRequest {
    pub phone_number: String,
}

#[derive(Serialize)]
pub struct WhatsappPhoneResponse {
    pub pairing_code: String,
}

/// Similar to connect_whatsapp but sends `!wa login phone <number>` and waits for a text pairing code
async fn connect_whatsapp_phone(
    client: &MatrixClient,
    bridge_bot: &str,
    phone_number: &str,
) -> Result<(OwnedRoomId, String)> {
    tracing::debug!("Starting WhatsApp phone pairing process");

    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;

    let request = CreateRoomRequest::new();
    let response = client.create_room(request).await?;
    let room_id = response.room_id();

    tracing::debug!("Created room with ID: {}", room_id);

    let room = client.get_room(room_id).ok_or(anyhow!("Room not found"))?;

    tracing::debug!("Inviting bot user: {}", bot_user_id);
    room.invite_user_by_id(&bot_user_id).await?;

    // Single sync to get the invitation processed
    client
        .sync_once(MatrixSyncSettings::default().timeout(Duration::from_secs(5)))
        .await?;

    // Wait for bot to join
    for _ in 0..15 {
        let members = room.members(matrix_sdk::RoomMemberships::JOIN).await?;
        if members.iter().any(|m| m.user_id() == bot_user_id) {
            tracing::debug!("Bot has joined the room");
            break;
        }
        sleep(Duration::from_millis(500)).await;
    }

    let members = room.members(matrix_sdk::RoomMemberships::empty()).await?;
    if !members.iter().any(|m| m.user_id() == bot_user_id) {
        return Err(anyhow!("Bot {} failed to join room", bot_user_id));
    }

    // Cancel any previous login attempt
    room.send(RoomMessageEventContent::text_plain("!wa cancel"))
        .await?;

    // Send phone login command
    let login_command = format!("!wa login phone {}", phone_number);
    tracing::info!("Sending WhatsApp phone login command");
    room.send(RoomMessageEventContent::text_plain(&login_command))
        .await?;

    // Wait for pairing code text response from bot
    let mut pairing_code = None;
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_millis(1500));

    for attempt in 1..=60 {
        tracing::debug!("Phone pairing sync attempt #{}/60", attempt);
        client.sync_once(sync_settings.clone()).await?;

        if let Some(room) = client.get_room(room_id) {
            let mut options =
                matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            options.limit = matrix_sdk::ruma::UInt::new(5).unwrap();
            let messages = room.messages(options).await?;

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
                                SyncRoomMessageEvent::Redacted(_) => continue,
                            };

                            let body = match &event_content.msgtype {
                                MessageType::Text(t) => &t.body,
                                MessageType::Notice(n) => &n.body,
                                _ => continue,
                            };

                            // Check for error messages
                            if body.to_lowercase().contains("error") {
                                return Err(anyhow!("Error from bot: {}", body));
                            }

                            // The bridge returns the pairing code as a short alphanumeric string
                            // like "A1B2-C3D4" or similar pattern
                            if let Some(code) = extract_pairing_code(body) {
                                pairing_code = Some(code);
                                break;
                            }
                        }
                    }
                }
            }

            if pairing_code.is_some() {
                break;
            }
        }

        sleep(Duration::from_millis(500)).await;
    }

    let pairing_code = pairing_code.ok_or(anyhow!(
        "WhatsApp pairing code not received within timeout. Please try again."
    ))?;

    Ok((room_id.into(), pairing_code))
}

/// Extract pairing code from bot message.
/// The mautrix-whatsapp bridge returns something like "Your pairing code: ABCD-EFGH" or just the code.
/// Pairing codes are 8 alphanumeric characters, sometimes with a hyphen in the middle.
fn extract_pairing_code(message: &str) -> Option<String> {
    // Look for an 8-character alphanumeric code pattern (with optional hyphen)
    let re = regex::Regex::new(r"[A-Z0-9]{4}-[A-Z0-9]{4}").ok()?;
    if let Some(m) = re.find(message) {
        return Some(m.as_str().to_string());
    }
    // Also try without hyphen (8 consecutive alphanumeric chars that aren't part of a longer word)
    let re2 = regex::Regex::new(r"\b[A-Z0-9]{8}\b").ok()?;
    if let Some(m) = re2.find(message) {
        return Some(m.as_str().to_string());
    }
    None
}

/// Wrapper with retry logic for phone pairing (mirrors connect_whatsapp_with_retry)
async fn connect_whatsapp_phone_with_retry(
    client: &mut Arc<MatrixClient>,
    bridge_bot: &str,
    phone_number: &str,
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
        match connect_whatsapp_phone(client, bridge_bot, phone_number).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if retry_count < MAX_RETRIES - 1 && is_one_time_key_conflict(&e) {
                    tracing::warn!(
                        "One-time key conflict for user {} (attempt {}/{}), resetting client store",
                        user_id,
                        retry_count + 1,
                        MAX_RETRIES
                    );
                    clear_user_store(&username).await?;
                    sleep(RETRY_DELAY).await;
                    match matrix_auth::get_cached_client(user_id, state).await {
                        Ok(new_client) => {
                            *client = new_client;
                            continue;
                        }
                        Err(init_err) => return Err(init_err),
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

pub async fn start_whatsapp_phone_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    AxumJson(payload): AxumJson<WhatsappPhoneRequest>,
) -> Result<AxumJson<WhatsappPhoneResponse>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::debug!(
        "Starting WhatsApp phone connection for user {}",
        auth_user.user_id
    );

    // Validate phone number (basic check: starts with + and has digits)
    let phone = payload.phone_number.trim().to_string();
    if !phone.starts_with('+') || phone.len() < 7 || !phone[1..].chars().all(|c| c.is_ascii_digit())
    {
        return Err((
            StatusCode::BAD_REQUEST,
            AxumJson(
                json!({"error": "Invalid phone number. Must start with + and country code (e.g. +14155551234)"}),
            ),
        ));
    }

    // Delete any existing bridge
    if let Err(e) = state
        .user_repository
        .delete_bridge(auth_user.user_id, "whatsapp")
    {
        tracing::warn!("No existing bridge to delete or error deleting: {}", e);
    }

    let client = matrix_auth::get_cached_client(auth_user.user_id, &state)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to initialize Matrix client: {}", e)})),
            )
        })?;

    // Auto-cleanup if leftovers detected
    if let Err(e) = cleanup_whatsapp_if_needed(&state, &client, auth_user.user_id).await {
        tracing::warn!("Auto-cleanup had issues but continuing: {}", e);
    }

    let bridge_bot = std::env::var("WHATSAPP_BRIDGE_BOT").expect("WHATSAPP_BRIDGE_BOT not set");

    let mut client_clone = Arc::clone(&client);
    let (room_id, pairing_code) = connect_whatsapp_phone_with_retry(
        &mut client_clone,
        &bridge_bot,
        &phone,
        auth_user.user_id,
        &state,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": format!("Failed to get pairing code: {}", e)})),
        )
    })?;

    // Create bridge record
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let new_bridge = NewPgBridge {
        user_id: auth_user.user_id,
        bridge_type: "whatsapp".to_string(),
        status: "connecting".to_string(),
        room_id: Some(room_id.to_string()),
        data: None,
        created_at: Some(current_time),
    };

    state
        .user_repository
        .create_bridge(new_bridge)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to store bridge information: {}", e)})),
            )
        })?;

    // Spawn monitor task (same as QR code flow)
    let state_clone = state.clone();
    let room_id_clone = room_id.clone();
    let bridge_bot_clone = bridge_bot.to_string();
    let client_for_monitor = client.clone();

    tokio::spawn(async move {
        match monitor_whatsapp_connection(
            &client_for_monitor,
            &room_id_clone,
            &bridge_bot_clone,
            auth_user.user_id,
            state_clone,
        )
        .await
        {
            Ok(_) => {
                tracing::info!(
                    "WhatsApp phone connection monitoring completed for user {}",
                    auth_user.user_id
                );
            }
            Err(e) => {
                tracing::error!(
                    "WhatsApp phone connection monitoring failed for user {}: {}",
                    auth_user.user_id,
                    e
                );
            }
        }
    });

    Ok(AxumJson(WhatsappPhoneResponse { pairing_code }))
}

// Internal cleanup helper (callable from connect; no response needed)
async fn cleanup_whatsapp_if_needed(
    state: &Arc<AppState>,
    client: &MatrixClient,
    user_id: i32,
) -> Result<Option<OwnedRoomId>> {
    // Returns old room_id if cleaned
    let bridge = state.user_repository.get_bridge(user_id, "whatsapp")?;
    let Some(bridge) = bridge else {
        tracing::debug!("No existing WhatsApp bridge; skipping cleanup");
        return Ok(None);
    };

    tracing::debug!("🧹 Detected stale WhatsApp bridge; starting cleanup");

    // Parse old room_id
    let old_room_id = OwnedRoomId::try_from(bridge.room_id.unwrap_or_default())
        .map_err(|_| anyhow!("Invalid old room ID"))?;

    if let Some(old_room) = client.get_room(&old_room_id) {
        // Parallel send cleanup commands (faster: ~5s total vs 15s sequential)
        let logout_cmd = old_room.send(RoomMessageEventContent::text_plain("!wa logout"));
        let portals_cmd = old_room.send(RoomMessageEventContent::text_plain(
            "!wa delete-all-portals",
        ));
        let session_cmd = old_room.send(RoomMessageEventContent::text_plain("!wa delete-session"));
        let (logout_res, portals_res, session_res) =
            tokio::join!(logout_cmd, portals_cmd, session_cmd);

        if let Err(e) = logout_res {
            tracing::warn!("Logout send failed: {}", e);
        }
        if let Err(e) = portals_res {
            tracing::warn!("Delete-portals send failed: {}", e);
        }
        if let Err(e) = session_res {
            tracing::warn!("Delete-session send failed: {}", e);
        }

        sleep(Duration::from_secs(3)).await; // Brief wait for bridge to process (reduced from 5s*3)
    }

    // Delete DB record
    state.user_repository.delete_bridge(user_id, "whatsapp")?;

    // Conditional store clear (only if no other bridges)
    let has_active_bridges = state.user_repository.has_active_bridges(user_id)?;
    if !has_active_bridges {
        let username = client
            .user_id()
            .ok_or(anyhow!("User ID unavailable"))?
            .localpart()
            .to_string();
        clear_user_store(&username).await.map_err(|e| {
            tracing::error!("Store clear failed during auto-reset: {}", e);
            anyhow!("Cleanup incomplete: {}", e)
        })?;
        tracing::info!("Auto-cleared store for user {} (no other bridges)", user_id);

        // Clear caches
        let mut matrix_clients = state.matrix_clients.lock().await;
        let mut sync_tasks = state.matrix_sync_tasks.lock().await;
        if let Some(task) = sync_tasks.remove(&user_id) {
            task.abort();
        }
        let _ = matrix_clients.remove(&user_id);
    } else {
        tracing::debug!("Other bridges active; skipping store clear during auto-reset");
    }

    tracing::debug!("✅ WhatsApp cleanup complete");
    Ok(Some(old_room_id))
}

pub async fn start_whatsapp_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<WhatsappConnectionResponse>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::debug!(
        "🚀 Starting WhatsApp connection process for user {}",
        auth_user.user_id
    );

    // Check for and delete any existing WhatsApp bridge to start fresh
    if let Err(e) = state
        .user_repository
        .delete_bridge(auth_user.user_id, "whatsapp")
    {
        tracing::warn!("No existing bridge to delete or error deleting: {}", e);
    }

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
    tracing::debug!(
        "✅ Matrix client obtained for user: {}",
        client.user_id().unwrap()
    );

    // Auto-cleanup if leftovers detected
    if let Err(e) = cleanup_whatsapp_if_needed(&state, &client, auth_user.user_id).await {
        // Don't fail on cleanup error (proceed to fresh connect/retry)
        tracing::warn!("Auto-cleanup had issues but continuing: {}", e);
    }

    // Get bridge bot from environment
    let bridge_bot = std::env::var("WHATSAPP_BRIDGE_BOT").expect("WHATSAPP_BRIDGE_BOT not set");

    tracing::debug!("🔗 Connecting to WhatsApp bridge...");
    // Connect to WhatsApp bridge
    let mut client_clone = Arc::clone(&client);
    let (room_id, qr_code_mxc) =
        connect_whatsapp_with_retry(&mut client_clone, &bridge_bot, auth_user.user_id, &state)
            .await
            .map_err(|e| {
                tracing::error!("Failed to connect to WhatsApp bridge: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(
                        json!({"error": format!("Failed to connect to WhatsApp bridge: {}", e)}),
                    ),
                )
            })?;

    // Fetch QR code media bytes and convert to base64 data URL
    tracing::info!("📥 Fetching QR code media bytes via SDK...");
    let mxc: matrix_sdk::ruma::OwnedMxcUri = qr_code_mxc.as_str().into();

    let request = MediaRequestParameters {
        source: MediaSource::Plain(mxc.to_owned()),
        format: MediaFormat::File,
    };

    let bytes = client
        .media()
        .get_media_content(&request, false)
        .await
        .map_err(|e| {
            tracing::error!("Failed to download QR code media: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to fetch QR code image: {}", e)})),
            )
        })?;

    let base64_str = Base64Engine.encode(&bytes);
    let qr_code_url = format!("data:image/png;base64,{}", base64_str);

    tracing::info!("Generated QR code data URL for user {}", auth_user.user_id);

    // Create bridge record
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let new_bridge = NewPgBridge {
        user_id: auth_user.user_id,
        bridge_type: "whatsapp".to_string(),
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
        match monitor_whatsapp_connection(
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
                    "WhatsApp connection monitoring completed successfully for user {}",
                    auth_user.user_id
                );
            }
            Err(e) => {
                tracing::error!(
                    "WhatsApp connection monitoring failed for user {}: {}",
                    auth_user.user_id,
                    e
                );
            }
        }
    });

    Ok(AxumJson(WhatsappConnectionResponse { qr_code_url }))
}

pub async fn get_whatsapp_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::debug!("📊 Checking WhatsApp status for user {}", auth_user.user_id);
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "whatsapp")
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

async fn monitor_whatsapp_connection(
    client: &MatrixClient,
    room_id: &OwnedRoomId,
    bridge_bot: &str,
    user_id: i32,
    state: Arc<AppState>,
) -> Result<(), anyhow::Error> {
    tracing::info!(
        "👀 Starting WhatsApp connection monitoring for user {} in room {}",
        user_id,
        room_id
    );
    let bot_user_id = OwnedUserId::try_from(bridge_bot)?;

    // Shorter sync timeout for faster response (like the old working version)
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(10));

    // Monitor for about 5 minutes (60 * 5 seconds)
    for attempt in 1..60 {
        tracing::info!("🔄 Monitor attempt #{} for user {}", attempt, user_id);

        // Sync with error handling
        if let Err(e) = client.sync_once(sync_settings.clone()).await {
            tracing::warn!("⚠️ Sync error on attempt #{}: {}", attempt, e);
            sleep(Duration::from_secs(5)).await;
            continue;
        }

        // Check ONLY the specific room where we sent the command (like the old working version)
        if let Some(room) = client.get_room(room_id) {
            // Get only recent messages
            let mut options =
                matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
            options.limit = matrix_sdk::ruma::UInt::new(5).unwrap();
            let messages = match room.messages(options).await {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("⚠️ Failed to get messages: {}", e);
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            tracing::info!("📬 Found {} messages in room", messages.chunk.len());

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

                            tracing::info!("📨 Bot message ({} chars)", content.len());

                            // Check for successful login first
                            if content.contains("Successfully logged in as") {
                                tracing::info!(
                                    "🎉 WhatsApp successfully connected for user {}",
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
                                    bridge_type: "whatsapp".to_string(),
                                    status: "connected".to_string(),
                                    room_id: Some(room_id.to_string()),
                                    data: connected_account,
                                    created_at: Some(current_time),
                                };

                                state.user_repository.delete_bridge(user_id, "whatsapp")?;
                                state.user_repository.create_bridge(new_bridge)?;

                                // Bridge-level E2EE is unnecessary: both the Matrix server and bridges run
                                // inside the enclave, so data never leaves unencrypted.

                                // Add client to app state and start sync
                                let mut matrix_clients = state.matrix_clients.lock().await;
                                let mut sync_tasks = state.matrix_sync_tasks.lock().await;

                                use matrix_sdk::room::Room;
                                use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;

                                let state_for_handler = Arc::clone(&state);
                                client.add_event_handler(
                                    move |ev: OriginalSyncRoomMessageEvent, room: Room, client| {
                                        let state = Arc::clone(&state_for_handler);
                                        async move {
                                            crate::utils::bridge::handle_bridge_message(
                                                ev, room, client, state,
                                            )
                                            .await;
                                        }
                                    },
                                );

                                let client_arc = Arc::new(client.clone());
                                matrix_clients.insert(user_id, client_arc.clone());

                                let sync_settings = MatrixSyncSettings::default()
                                    .timeout(Duration::from_secs(30))
                                    .full_state(true);

                                let handle = tokio::spawn(async move {
                                    loop {
                                        match client_arc.sync(sync_settings.clone()).await {
                                            Ok(_) => {
                                                tokio::time::sleep(
                                                    tokio::time::Duration::from_secs(1),
                                                )
                                                .await
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

                                // Send sync commands
                                let _ = room
                                    .send(RoomMessageEventContent::text_plain(
                                        "!wa sync contacts --create-portals",
                                    ))
                                    .await;
                                sleep(Duration::from_millis(500)).await;
                                let _ = room
                                    .send(RoomMessageEventContent::text_plain(
                                        "!wa sync groups --create-portals",
                                    ))
                                    .await;

                                return Ok(());
                            }

                            // Check for error messages
                            let error_patterns = [
                                "error",
                                "failed",
                                "timeout",
                                "disconnected",
                                "connection lost",
                                "authentication failed",
                                "login failed",
                            ];
                            if error_patterns
                                .iter()
                                .any(|&p| content.to_lowercase().contains(p))
                            {
                                tracing::error!(
                                    "❌ WhatsApp connection failed for user {}: {}",
                                    user_id,
                                    content
                                );
                                state.user_repository.delete_bridge(user_id, "whatsapp")?;
                                return Err(anyhow!("WhatsApp connection failed: {}", content));
                            }
                        }
                    }
                }
            }
        } else {
            tracing::warn!("⚠️ Room {} not found", room_id);
        }

        sleep(Duration::from_secs(5)).await;
    }

    // Timeout - connection didn't complete
    tracing::warn!("⏰ WhatsApp connection timed out for user {}", user_id);
    state.user_repository.delete_bridge(user_id, "whatsapp")?;
    Err(anyhow!("WhatsApp connection timed out after 5 minutes"))
}

pub async fn resync_whatsapp(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    println!(
        "🔄 Starting WhatsApp resync process for user {}",
        auth_user.user_id
    );

    // Get the bridge information first
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "whatsapp")
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
    let room_id = OwnedRoomId::try_from(bridge.room_id.unwrap_or_default()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Invalid room ID format"})),
        )
    })?;

    if let Some(room) = client.get_room(&room_id) {
        println!("📱 Setting up Matrix event handler");

        // Set up event handler for the Matrix client
        client.add_event_handler(|ev: SyncRoomMessageEvent| async move {
            match ev {
                SyncRoomMessageEvent::Original(_msg) => {
                    // Add more specific message handling logic here if needed
                }
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
        if let Err(e) = room
            .send(RoomMessageEventContent::text_plain(
                "!wa sync contacts --create-portals",
            ))
            .await
        {
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
        if let Err(e) = room
            .send(RoomMessageEventContent::text_plain(
                "!wa sync groups --create-portals",
            ))
            .await
        {
            tracing::error!("Failed to send groups sync command: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to send groups sync command"})),
            ));
        }
        tracing::debug!("✅ Sent groups sync command");

        tracing::debug!(
            "✅ WhatsApp resync process completed for user {}",
            auth_user.user_id
        );
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
    tracing::info!(
        "🔌 Starting WhatsApp disconnection process for user {}",
        auth_user.user_id
    );

    // Get the bridge information first
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "whatsapp")
        .map_err(|e| {
            tracing::error!("Failed to get WhatsApp bridge info: {}", e);
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

    let room_id_str = bridge.room_id.clone().unwrap_or_default();

    // Delete the bridge record IMMEDIATELY - user sees instant response
    state
        .user_repository
        .delete_bridge(auth_user.user_id, "whatsapp")
        .map_err(|e| {
            tracing::error!("Failed to delete WhatsApp bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to delete bridge record"})),
            )
        })?;

    tracing::info!(
        "✅ WhatsApp bridge record deleted for user {}",
        auth_user.user_id
    );

    // Spawn background task for cleanup - don't block the response
    let state_clone = state.clone();
    let user_id = auth_user.user_id;
    tokio::spawn(async move {
        tracing::info!(
            "🧹 Starting background cleanup for WhatsApp user {}",
            user_id
        );

        // Get Matrix client for cleanup
        let client = match matrix_auth::get_cached_client(user_id, &state_clone).await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Background cleanup: Failed to get Matrix client: {}", e);
                return;
            }
        };

        // Get the room and send cleanup commands
        if let Ok(room_id) = OwnedRoomId::try_from(room_id_str.as_str()) {
            if let Some(room) = client.get_room(&room_id) {
                // Send logout command
                if let Err(e) = room
                    .send(RoomMessageEventContent::text_plain("!wa logout"))
                    .await
                {
                    tracing::error!("Background cleanup: Failed to send logout command: {}", e);
                }
                sleep(Duration::from_secs(2)).await;

                // Send command to delete all portals
                if let Err(e) = room
                    .send(RoomMessageEventContent::text_plain(
                        "!wa delete-all-portals",
                    ))
                    .await
                {
                    tracing::error!(
                        "Background cleanup: Failed to send delete-portals command: {}",
                        e
                    );
                }
                sleep(Duration::from_secs(2)).await;

                // Send delete-session command
                if let Err(e) = room
                    .send(RoomMessageEventContent::text_plain("!wa delete-session"))
                    .await
                {
                    tracing::error!(
                        "Background cleanup: Failed to send delete-session command: {}",
                        e
                    );
                }
                sleep(Duration::from_secs(2)).await;
            }
        }

        // Check for remaining active bridges and cleanup if none left
        let has_active_bridges = state_clone
            .user_repository
            .has_active_bridges(user_id)
            .unwrap_or(false);

        if !has_active_bridges {
            if let Some(user_id_matrix) = client.user_id() {
                let username = user_id_matrix.localpart().to_string();
                if let Err(e) = clear_user_store(&username).await {
                    tracing::error!("Background cleanup: Failed to clear user store: {}", e);
                } else {
                    tracing::info!(
                        "Background cleanup: Cleared Matrix store for user {}",
                        user_id
                    );
                }
            }

            // Remove client and sync task
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

        tracing::info!(
            "🧹 Background cleanup completed for WhatsApp user {}",
            user_id
        );
    });

    Ok(AxumJson(json!({
        "message": "WhatsApp disconnected successfully"
    })))
}

/// Health check endpoint using !wa ping command
/// Returns the actual WhatsApp connection status from the bridge
pub async fn check_whatsapp_health(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!("🏥 Checking WhatsApp health for user {}", auth_user.user_id);

    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, "whatsapp")
        .map_err(|e| {
            tracing::error!("Failed to get WhatsApp bridge: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get WhatsApp bridge info"})),
            )
        })?;

    let Some(bridge) = bridge else {
        return Ok(AxumJson(json!({
            "healthy": false,
            "message": "WhatsApp is not connected"
        })));
    };

    let client = matrix_auth::get_cached_client(auth_user.user_id, &state)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get Matrix client: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to initialize Matrix client: {}", e)})),
            )
        })?;

    let room_id = OwnedRoomId::try_from(bridge.room_id.unwrap_or_default()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Invalid room ID format"})),
        )
    })?;

    let room = client.get_room(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            AxumJson(json!({"error": "WhatsApp bridge room not found"})),
        )
    })?;

    let bridge_bot = std::env::var("WHATSAPP_BRIDGE_BOT").expect("WHATSAPP_BRIDGE_BOT not set");
    let bot_user_id = OwnedUserId::try_from(bridge_bot.as_str()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Invalid bridge bot user ID"})),
        )
    })?;

    // Send `!wa list-logins` (read-only, no side effects). The bridge bot
    // responds with one line per login: "* `<id>` (<identifier>) - `CONNECTED`"
    // for healthy logins, other statuses (LOGGED_OUT, etc) for unhealthy.
    // We use the !wa prefix to be safe regardless of whether this is the
    // bridge's "management room" - the prefix always works.
    tracing::info!(
        "📤 Sending !wa list-logins for health check for user {}",
        auth_user.user_id
    );
    let responses = match crate::utils::bridge::probe_bridge_room(
        &client,
        &room,
        &bot_user_id,
        "!wa list-logins",
        Duration::from_secs(8),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("⚠️ probe_bridge_room failed: {}", e);
            return Ok(AxumJson(json!({
                "healthy": false,
                "ambiguous": true,
                "message": format!("probe failed: {}", e)
            })));
        }
    };

    if responses.is_empty() {
        // No fresh response from bridge bot. Could mean bot is down or sync is
        // lagging. DO NOT delete the bridge - just report ambiguous status so
        // the UI can prompt the user to retry.
        tracing::warn!(
            "⚠️ WhatsApp health check: no response from bridge bot for user {}",
            auth_user.user_id
        );
        return Ok(AxumJson(json!({
            "healthy": false,
            "ambiguous": true,
            "message": "Bridge bot did not respond. The bridge may be temporarily unreachable."
        })));
    }

    let combined = responses.join("\n");
    tracing::info!(
        "📨 WhatsApp list-logins response for user {}: {:?}",
        auth_user.user_id,
        combined
    );

    if crate::utils::bridge::list_logins_has_connected(&combined) {
        tracing::info!(
            "✅ WhatsApp health check passed for user {}",
            auth_user.user_id
        );
        if let Some(ident) = crate::utils::bridge::extract_first_connected_identifier(&combined) {
            if let Err(e) =
                state
                    .user_repository
                    .update_bridge_data(auth_user.user_id, "whatsapp", &ident)
            {
                tracing::warn!("Failed to save connected account: {}", e);
            }
        }
        Ok(AxumJson(json!({
            "healthy": true,
            "message": combined,
        })))
    } else {
        // Bridge confirms there are no CONNECTED logins. Report unhealthy but
        // do NOT delete the bridge record - that's the user's call. Frontend
        // can prompt them to reconnect.
        tracing::warn!(
            "❌ WhatsApp health check: no CONNECTED login for user {}: {:?}",
            auth_user.user_id,
            combined
        );
        Ok(AxumJson(json!({
            "healthy": false,
            "message": combined,
        })))
    }
}
