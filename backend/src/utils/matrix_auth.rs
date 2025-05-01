use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use reqwest::Client as HttpClient;
use crate::repositories::user_repository::UserRepository;
use serde_json::json;
use sha1::Sha1;
use uuid::Uuid;
use magic_crypt::MagicCryptTrait;
use axum::{
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
use std::sync::Arc;
use reqwest;
use base64;
use tokio::time::{sleep, Duration, Instant};
use crate::{
    AppState,
    handlers::auth_middleware::AuthUser,
    models::user_models::{NewBridge, Bridge},
};
use url::Url;

use sha2::{Sha256, Digest};
use base64::{engine::general_purpose, Engine as _}; // or use hex crate

use tokio::sync::Mutex;
use std::collections::HashMap;

pub async fn initialize_matrix_user_clients(
    user_repository: Arc<UserRepository>,
) -> Arc<Mutex<HashMap<i32, matrix_sdk::Client>>> {
    let user_ids = user_repository.get_users_with_matrix_bridge_connections().unwrap_or_default();
    let mut clients = HashMap::new();

    for user_id in user_ids {
        match crate::utils::matrix_auth::get_client(user_id, &user_repository).await {
            Ok(client) => {
                clients.insert(user_id, client);
                tracing::info!("Initialized Matrix client for user {}", user_id);
            },
            Err(e) => {
                tracing::error!("Failed to initialize Matrix client for user {}: {}", user_id, e);
            }
        }
    }

    Arc::new(Mutex::new(clients))
}

/// Updates the matrix_user_clients map based on current bridge connections
pub async fn update_matrix_user_clients(
    state: &AppState,
) -> Result<(), anyhow::Error> {
    tracing::info!("Updating Matrix user clients based on bridge connections");
    
    // Get users who should have Matrix clients
    let users_with_bridges = state.user_repository.get_users_with_matrix_bridge_connections()?;
    let mut clients_map = state.matrix_user_clients.lock().await;
    
    // Track which users we've processed to identify those to remove
    let mut processed_users = std::collections::HashSet::new();
    
    // Add or update clients for users with bridges
    for user_id in &users_with_bridges {
        processed_users.insert(*user_id);
        
        // If user already has a client, skip
        if clients_map.contains_key(user_id) {
            continue;
        }
        
        // Create new client for user
        match get_client(*user_id, &state.user_repository).await {
            Ok(client) => {
                tracing::info!("Added new Matrix client for user {}", user_id);
                clients_map.insert(*user_id, client);
            },
            Err(e) => {
                tracing::error!("Failed to create Matrix client for user {}: {}", user_id, e);
            }
        }
    }
    
    // Remove clients for users who no longer have bridges
    let users_to_remove: Vec<i32> = clients_map.keys()
        .filter(|user_id| !processed_users.contains(user_id))
        .copied()
        .collect();
    
    for user_id in users_to_remove {
        clients_map.remove(&user_id);
        tracing::info!("Removed Matrix client for user {} who no longer has bridge connections", user_id);
    }
    
    tracing::info!("Matrix user clients updated. Total active clients: {}", clients_map.len());
    Ok(())
}

/// Initializes a Matrix client with given credentials.
async fn initialize_client(username: &str, access_token: &str, device_id: &str) -> Result<MatrixClient> {
    tracing::info!("Initializing Matrix client for user {}", username);
    
    // Get homeserver URL from environment
    let homeserver = std::env::var("MATRIX_HOMESERVER")
        .map_err(|_| anyhow!("MATRIX_HOMESERVER not set"))?;
    
    // Get domain from homeserver URL
    let url = Url::parse(&homeserver)
        .map_err(|e| anyhow!("Invalid homeserver URL: {}", e))?;
    let domain = url.host_str()
        .ok_or_else(|| anyhow!("No host in homeserver URL"))?;
    
    let full_user_id = format!("@{}:{}", username, domain);
    
    // Initialize Matrix client with persistent store
    let store_path = format!(
        "{}/{}",
        std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
            .map_err(|_| anyhow!("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"))?,
        username
    );
    
    std::fs::create_dir_all(&store_path)
        .map_err(|e| anyhow!("Failed to create store directory {}: {}", store_path, e))?;
    
    let client = MatrixClient::builder()
        .homeserver_url(&homeserver)
        .sqlite_store(store_path, None)
        .build()
        .await
        .map_err(|e| anyhow!("Failed to build Matrix client: {}", e))?;
    
    // Restore session with access token
    client.restore_session(matrix_sdk::AuthSession::Matrix(matrix_sdk::authentication::matrix::MatrixSession {
        meta: matrix_sdk::SessionMeta {
            user_id: OwnedUserId::try_from(full_user_id.clone())
                .map_err(|e| anyhow!("Invalid user_id format: {}", e))?,
            device_id: OwnedDeviceId::try_from(device_id)
                .map_err(|e| anyhow!("Invalid device_id format: {}", e))?,
        },
        tokens: matrix_sdk::authentication::matrix::MatrixSessionTokens {
            access_token: access_token.to_string(),
            refresh_token: None,
        },
    }))
    .await
    .map_err(|e| anyhow!("Failed to restore session: {}", e))?;
    
    // Perform initial sync to get room state
    client.sync_once(MatrixSyncSettings::default())
        .await
        .map_err(|e| anyhow!("Failed to perform initial sync: {}", e))?;
    
    tracing::info!("Matrix client initialized successfully for {}", username);
    Ok(client)
}

/// Checks for and joins any rooms the user has been invited to
/// 
/// # Arguments
/// * `client` - The Matrix client to use for checking invitations
/// 
/// # Returns
/// The number of rooms joined, or an error
pub async fn join_invited_rooms(client: &MatrixClient) -> Result<usize> {
    tracing::info!("Checking for room invitations for user {}", client.user_id().unwrap_or(&OwnedUserId::try_from("@unknown:unknown").unwrap()));
    
    // Get all rooms where the user has been invited
    let invited_rooms: Vec<_> = client
        .rooms()
        .into_iter()
        .filter(|room| room.state() == matrix_sdk::RoomState::Invited)
        .collect();
    
    if invited_rooms.is_empty() {
        tracing::info!("No room invitations found");
        return Ok(0);
    }
    
    tracing::info!("Found {} room invitations", invited_rooms.clone().len());
    let mut joined_count = 0;
    
    for room in invited_rooms.clone() {
        let room_id = room.room_id();
        tracing::info!("Attempting to join room: {}", room_id);
        
        match client.join_room_by_id(room_id).await {
            Ok(_) => {
                tracing::info!("Successfully joined room: {}", room_id);
                joined_count += 1;
            }
            Err(e) => {
                tracing::error!("Failed to join room {}: {}", room_id, e);
                // Continue with other rooms even if one fails
            }
        }
    }
    
    tracing::info!("Joined {} rooms out of {} invitations", joined_count, invited_rooms.len());
    Ok(joined_count)
}



pub async fn register_user(homeserver: &str, shared_secret: &str) -> Result<(String, String, String, String)> {
    println!("🔑 Starting Matrix user registration...");
    // Create HTTP client
    let http_client = HttpClient::new();
    
    // Get registration nonce
    let nonce_res = http_client
        .get(format!("{}/_synapse/admin/v1/register", homeserver))
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch nonce: {}", e))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| anyhow!("Failed to parse nonce response: {}", e))?;
    let nonce = nonce_res["nonce"]
        .as_str()
        .ok_or_else(|| anyhow!("No nonce in response"))?;
    println!("📝 Got registration nonce: {}", nonce);

    // Generate unique username and password
    let username = format!("appuser_{}", Uuid::new_v4().to_string().replace("-", ""));
    let password = Uuid::new_v4().to_string();
    println!("👤 Generated username: {}", username);
    println!("🔑 Generated password: {}", password);

    // Calculate MAC
    let mac_content = format!("{}\0{}\0{}\0notadmin", nonce, username, password);
    println!("🔒 MAC content: {}", mac_content);
    let mut mac = Hmac::<Sha1>::new_from_slice(shared_secret.as_bytes())
        .map_err(|e| anyhow!("Failed to create HMAC: {}", e))?;
    mac.update(mac_content.as_bytes());
    let mac_result = hex::encode(mac.finalize().into_bytes());
    println!("🔐 Generated MAC: {}", mac_result);

    // Register user
    println!("📡 Sending registration request to Matrix server...");
    let response = http_client
        .post(format!("{}/_synapse/admin/v1/register", homeserver))
        .json(&json!({
            "nonce": nonce,
            "username": username,
            "password": password,
            "admin": false,
            "mac": mac_result
        }))
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send registration request: {}", e))?;

    // Log status
    let status = response.status();
    println!("📡 Registration response status: {}", status);

    // Get response body
    let register_res = response .text()
        .await
        .map_err(|e| anyhow!("Failed to read response body: {}", e))?;
    println!("📡 Registration response body: {}", register_res);

    let register_json: serde_json::Value = serde_json::from_str(&register_res)
        .map_err(|e| anyhow!("Failed to parse registration response: {}", e))?;

    if status.is_success() {
        let access_token = register_json["access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("No access_token in response: {}", register_res))?
            .to_string();
        let device_id = register_json["device_id"]
            .as_str()
            .ok_or_else(|| anyhow!("No device_id in response: {}", register_res))?
            .to_string();
        println!("✅ Matrix registration successful!");
        println!("📱 Device ID: {}", device_id);
        println!("🎫 Access token received (length: {})", access_token.len());
        Ok((username, access_token, device_id, password))
    } else {
        let error = register_json["error"]
            .as_str()
            .unwrap_or("Unknown error");
        Err(anyhow!("Registration failed: {} (status: {})", error, status))
    }
}


/// Generates a deterministic password for a Matrix user based on their user ID.
/// This allows us to recreate the same password when needed for authentication.
fn generate_matrix_password(user_id: &str) -> String {
    // Create a secret key from environment
    let secret_key = std::env::var("MATRIX_PASSWORD_SECRET")
        .unwrap_or_else(|_| {
            tracing::warn!("MATRIX_PASSWORD_SECRET not set, using default value");
            "default_matrix_password_secret".to_string()
        });
    
    // Create a deterministic password by hashing the user ID with the secret
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}", user_id, secret_key).as_bytes());
    let result = hasher.finalize();
    
    // Convert to base64 for a usable password
    general_purpose::STANDARD.encode(result)
}



/// Attempts to login a Matrix user with the provided credentials
/// 
/// # Arguments
/// * `homeserver` - The URL of the Matrix homeserver
/// * `username` - The username to login with
/// * `password` - The password for authentication
/// 
/// # Returns
/// A tuple containing the access token and device ID on success
// TODO delete i think
pub async fn login_user(homeserver: &str, username: &str, password: &str) -> Result<(String, String)> {
    println!("🔑 Attempting Matrix user login for {}", username);
    // Create HTTP client
    let http_client = HttpClient::new();
    
    let response = http_client
        .post(format!("{}/_matrix/client/v3/login", homeserver))
        .json(&json!({
            "type": "m.login.password",
            "identifier": {
                "type": "m.id.user",
                "user": username
            },
            "password": password,
            "initial_device_display_name": "Bridge Device"
        }))
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send login request: {}", e))?;

    let status = response.status();
    let login_res = response
        .text()
        .await
        .map_err(|e| anyhow!("Failed to read login response: {}", e))?;
    let login_json: serde_json::Value = serde_json::from_str(&login_res)
        .map_err(|e| anyhow!("Failed to parse login response: {}", e))?;

    if status.is_success() {
        let access_token = login_json["access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("No access_token in response: {}", login_res))?
            .to_string();
        let device_id = login_json["device_id"]
            .as_str()
            .ok_or_else(|| anyhow!("No device_id in response: {}", login_res))?
            .to_string();
        println!("✅ Matrix login successful for {}", username);
        Ok((access_token, device_id))
    } else {
        let error = login_json["error"]
            .as_str()
            .unwrap_or("Unknown error");
        Err(anyhow!("Login failed: {} (status: {})", error, status))
    }
}

/// Deletes a Matrix device using the admin API
/// 
/// # Arguments
/// * `homeserver` - The URL of the Matrix homeserver
/// * `shared_secret` - The shared secret for admin API authentication
/// * `user_id` - The Matrix user ID whose device should be deleted
/// * `device_id` - The device ID to delete
/// 
/// # Returns
/// Success or error result
// TODO remove i think
pub async fn delete_device(homeserver: &str, shared_secret: &str, user_id: &str, device_id: &str) -> Result<()> {
    println!("🗑️ Deleting device {} for user {}", device_id, user_id);
    // Create HTTP client
    let http_client = HttpClient::new();
    
    let response = http_client
        .delete(format!(
            "{}/_synapse/admin/v2/users/{}/devices/{}",
            homeserver, user_id, device_id
        ))
        .header("Authorization", format!("Bearer {}", shared_secret))
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send device deletion request: {}", e))?;

    let status = response.status();
    if status.is_success() {
        println!("✅ Device {} deleted successfully", device_id);
        Ok(())
    } else {
        let error_res = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read deletion response: {}", e))?;
        Err(anyhow!("Device deletion failed: {} (status: {})", error_res, status))
    }
}

/// Encrypts a Matrix access token for secure storage
/// 
/// # Arguments
/// * `token` - The token to encrypt
/// 
/// # Returns
/// The encrypted token as a base64 string
pub fn encrypt_token(token: &str) -> Result<String> {
    println!("🔒 Encrypting access token...");
    let encryption_key = std::env::var("ENCRYPTION_KEY")
        .map_err(|_| anyhow!("ENCRYPTION_KEY not set"))?;
    
    let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);
    Ok(cipher.encrypt_str_to_base64(token))
}

/// Decrypts a previously encrypted Matrix access token
/// 
/// # Arguments
/// * `encrypted_token` - The encrypted token in base64 format
/// 
/// # Returns
/// The decrypted token as a string
pub fn decrypt_token(encrypted_token: &str) -> Result<String> {
    println!("🔓 Decrypting access token...{}", encrypted_token);
    let encryption_key = std::env::var("ENCRYPTION_KEY")
        .map_err(|_| anyhow!("ENCRYPTION_KEY not set"))?;
    
    let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);
    cipher.decrypt_base64_to_string(encrypted_token)
        .map_err(|e| anyhow!("Failed to decrypt token: {}", e))
}

/// Deletes all Matrix devices except the current one
/// 
/// This is important for security as it ensures only one device has access
/// to the user's Matrix account at a time.
/// 
/// # Arguments
/// * `client` - The Matrix client
/// * `current_device_id` - The device ID to keep
/// 
/// # Returns
/// The number of devices deleted, or an error
pub async fn delete_other_devices(client: &MatrixClient, current_device_id: &str) -> Result<usize> {
    tracing::info!("Checking for other devices to delete for user {}", client.user_id().unwrap_or(&OwnedUserId::try_from("@unknown:unknown").unwrap()));
    
    // Get all devices
    let response = client.devices().await
        .map_err(|e| anyhow!("Failed to get devices: {}", e))?;
    
    // Filter out the current device
    let devices_to_delete: Vec<OwnedDeviceId> = response.devices
        .into_iter()
        .filter(|device| device.device_id.as_str() != current_device_id)
        .map(|device| device.device_id)
        .collect();
    
    if devices_to_delete.is_empty() {
        tracing::info!("No other devices to delete");
        return Ok(0);
    }
    
    tracing::info!("Found {} other devices to delete", devices_to_delete.len());
    
    // Try to delete devices
    match client.delete_devices(&devices_to_delete, None).await {
        Ok(_) => {
            tracing::info!("Successfully deleted {} devices", devices_to_delete.len());
            Ok(devices_to_delete.len())
        },
        Err(e) => {
            // Handle UIAA (User Interactive Authentication API) response
            if let Some(uiaa_info) = e.as_uiaa_response() {
                // Get user ID and generate password
                let user_id_str = client.user_id().unwrap().to_string();
                let username = user_id_str.strip_prefix('@')
                    .and_then(|s| s.split(':').next())
                    .ok_or_else(|| anyhow!("Invalid user ID format"))?;
                let password = generate_matrix_password(&user_id_str);
                
                // Create authentication data
                let user_identifier = matrix_sdk::ruma::api::client::uiaa::UserIdentifier::UserIdOrLocalpart(username.to_string());
                let mut password_auth = matrix_sdk::ruma::api::client::uiaa::Password::new(user_identifier, password);
                password_auth.session = uiaa_info.session.clone();
                
                // Try again with authentication
                match client.delete_devices(&devices_to_delete, Some(matrix_sdk::ruma::api::client::uiaa::AuthData::Password(password_auth))).await {
                    Ok(_) => {
                        tracing::info!("Successfully deleted {} devices after authentication", devices_to_delete.len());
                        Ok(devices_to_delete.len())
                    },
                    Err(e) => {
                        tracing::error!("Failed to delete devices after authentication: {}", e);
                        Err(anyhow!("Failed to delete devices after authentication: {}", e))
                    }
                }
            } else {
                tracing::error!("Failed to delete devices: {}", e);
                Err(anyhow!("Failed to delete devices: {}", e))
            }
        }
    }
}


// create client with the client sqlite store(that stores both state and encryption keys)

pub async fn login_with_password(client: &MatrixClient, user_repository: &UserRepository, username: &str, password: &str, device_id: Option<&str>, user_id: i32) ->Result<()> {
    println!("🔑 Attempting to login with username: {}, password: {}, device_id: {:#?}", username, password, device_id);
    let res;
    if let Some(device_id) = device_id {
        println!("using existing device_id");
        res = client.matrix_auth()
            .login_username(username, password)
            .device_id(device_id)
            .send()
            .await;
    } else {
        println!("creating new device_id");
        res = client.matrix_auth()
            .login_username(username, password)
            .send()
            .await;
    }
    if let Ok(response) = res {
        println!("✅ Login successful");
        println!("📱 device ID: {}", response.device_id.as_str());
        println!("🎫 New access token received (length: {})", response.access_token.len());
        
        // Store the new device_id and access_token
        println!("💾 Saving new device ID and access token to database");
        user_repository.set_matrix_device_id_and_access_token(user_id, &response.access_token, &response.device_id.as_str())?;
        println!("✅ Successfully saved credentials");
        
        /*
        // Delete other devices for security
        println!("🔄 Checking for other devices to delete");
        match delete_other_devices(client, &response.device_id.as_str()).await {
            Ok(count) => {
                if count > 0 {
                    println!("🗑️ Deleted {} other devices for user {}", count, user_id);
                    tracing::info!("Deleted {} other devices for user {}", count, user_id);
                } else {
                    println!("✅ No other devices to delete");
                }
            },
            Err(e) => {
                println!("⚠️ Failed to delete other devices: {}", e);
                tracing::warn!("Failed to delete other devices for user {}: {}", user_id, e);
                // Continue anyway as this is not critical
            }
        }
        */
    } else {
        println!("❌ Login failed: {:?}", res.err());
        return Err(anyhow!("Failed to login with username and password. User may need to be re-registered."));
    }
    println!("✅ Login with password completed successfully");
    Ok(())
}


pub async fn get_client(user_id: i32, user_repository: &UserRepository) -> Result<MatrixClient> {
    println!("🔄 Starting get_client for user_id: {}", user_id);

    // Get user profile from database
    let user = user_repository.find_by_id(user_id).unwrap().unwrap();
    println!("👤 Found user: id={}, username={}", user.id, user.email);

    // Initialize the Matrix client
    let homeserver_url = std::env::var("MATRIX_HOMESERVER")
        .map_err(|_| anyhow!("MATRIX_HOMESERVER not set"))?;
    let shared_secret = std::env::var("MATRIX_SHARED_SECRET")
        .map_err(|_| anyhow!("MATRIX_SHARED_SECRET not set"))?;
    println!("🏠 Using Matrix homeserver: {}", homeserver_url);

    // if matrix user doesn't exist, register one
    let mut username: String = "default".to_string();
    let mut password: String = "default".to_string();
    let mut device_id: Option<String> = Some("default".to_string());
    if user.matrix_username.is_none() {
        println!("🆕 No Matrix username found, registering new Matrix user");
        let (new_username, new_access_token, new_device_id, new_password) = register_user(&homeserver_url, &shared_secret).await?;
        println!("✅ Successfully registered new Matrix user: {}, with password: {}", new_username, new_password);
        user_repository.set_matrix_credentials(user.id, &new_username, &new_access_token, &new_device_id, &new_password)?;
        username = new_username;
        password = new_password;
        device_id = Some(new_device_id);
        println!("💾 Saved Matrix credentials to database and here is the username from db {:#?}, password: {:#?}, device_id: {:#?}", username, password, device_id);
    } else {
        println!("✓ Matrix username exists: {}", user.matrix_username.clone().unwrap());
        println!("✓ Matrix password exists: {}", user.encrypted_matrix_password.clone().unwrap());
        username = user.matrix_username.clone().unwrap();
        let pass = decrypt_token(user.encrypted_matrix_password.as_ref().unwrap().as_str()).unwrap();
        password = pass;
        device_id = user.matrix_device_id;
    }
 
    let store_path = format!(
        "{}/{}",
        std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
            .map_err(|_| anyhow!("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"))?,
            username
    );
    
    println!("💾 Using store path: {}", store_path);
    std::fs::create_dir_all(&store_path)
        .map_err(|e| anyhow!("Failed to create store directory {}: {}", store_path, e))?;

    println!("🔨 Building Matrix client");
    let client = MatrixClient::builder()
        .homeserver_url(&homeserver_url)
        .sqlite_store(store_path, None)
        .build()
        .await
        .unwrap();
    println!("✅ Matrix client built successfully");
    
    // logged_in checks if client store has device_id and access token stored
    println!("🔑 Checking if client is logged in: {}", client.logged_in());
    if !client.logged_in() {
        println!("❌ Client not logged in, attempting authentication");

        println!("🔄 Attempting to login with username and password: {}, {}", username, password);
        login_with_password(&client, user_repository, username.as_str(), password.as_str(), device_id.as_deref(), user.id).await?;
        /*
        if let Some(device_id) = user.matrix_device_id.clone() {
            println!("🔑 Found device_id in database: {}", device_id);
            let access_token = decrypt_token(user.encrypted_matrix_access_token.as_ref().unwrap().as_str()).unwrap();
            println!("🔑 Decrypted access token (length: {}): {}", access_token.clone().len(), access_token.clone());
            println!("🔄 Attempting to login with token and device_id");
            let res = client.matrix_auth()
                        .login_token(&access_token.as_str())
                        .device_id(device_id.as_str())
                        .send()
                        .await;
            if let Ok(response) = res {
                println!("✅ Successfully logged in with token and device_id");
                // if client store tokens and device were different they will now at least get overriden
            } else {
                println!("❌ Token login failed, falling back to username/password");
                // if fails, we try logging in with username and password
                let password = decrypt_token(user.encrypted_matrix_password.as_ref().unwrap().as_str()).unwrap();
                println!("🔑 Decrypted password (length: {})", password.len());
                println!("🔄 Attempting to login with username and password");
                login_with_password(&client, user_repository, user.matrix_username.as_ref().unwrap().as_str(), password.as_str(), user.id).await?;
            }
            
            
        } else {
            println!("❓ No device_id found in database");
            // if not device_id was stored, let's try logging in with username and password
            let encrypted_stored_password = user.encrypted_matrix_password.clone();


            let password = decrypt_token(user.encrypted_matrix_password.as_ref().unwrap().as_str()).unwrap();
            println!("🔑 Decrypted password (length: {})", password.len());
            println!("🔄 Attempting to login with username and password");
            login_with_password(&client, user_repository, user.matrix_username.as_ref().unwrap().as_str(), password.as_str(), user.id).await?;
        }
        */
    } else { // client store has some device id and access token already
        /*
        println!("✅ Client already logged in from store");
	    // call whoami to check if client store device id and access token are valid at the server also
	    println!("🔍 Verifying client credentials with whoami");
	    let res = client.whoami().await.unwrap();
        match res.device_id {
            Some(owned_device_id) => {
                println!("🔍 Server reports device_id: {}", owned_device_id.as_str());
                if user.matrix_device_id.is_none() || owned_device_id.as_str() != user.matrix_device_id.as_ref().unwrap() {
                    println!("❌ Device ID mismatch between server and database");
                    let password = decrypt_token(user.encrypted_matrix_password.as_ref().unwrap().as_str()).unwrap();
                    println!("🔄 Creating new device with username/password login");
                    // we are lazy and just create new device if they don't match
                    login_with_password(&client, user_repository, user.matrix_username.as_ref().unwrap().as_str(), password.as_str(), user.id).await?;
                } else {
                    println!("✅ Device ID matches between server and database");
                }
            },
            None => {
                println!("❌ No device ID found on server");
                // create new device if none was found on the server
                let password = decrypt_token(user.encrypted_matrix_password.as_ref().unwrap().as_str()).unwrap();
                println!("🔄 Creating new device with username/password login");
                login_with_password(&client, user_repository, user.matrix_username.as_ref().unwrap().as_str(), password.as_str(), user.id).await?;
            }
        }
        */
    }
    println!("✅ Authentication complete - client is logged already in");
    // here we should have client store, our db and server synced with the same device id and access token

    // handle keys now
    println!("🔐 Setting up encryption keys and secret storage");
    let mut redo_secret_storage = false;
    if let Some(status) = client.encryption().cross_signing_status().await {
        println!("🔐 Cross-signing status: {:?}", status);
        if let Some(encrypted_key) = user.encrypted_matrix_secret_storage_recovery_key.clone() {
            println!("🔑 Found encrypted secret storage recovery key in database");
            if client.encryption().secret_storage().is_enabled().await? {
                println!("✅ Secret storage is enabled, importing secrets");
                let passphrase = decrypt_token(&encrypted_key.as_str()).unwrap();
                println!("🔑 Decrypted recovery key (length: {})", passphrase.len());
                let secret_store = client.encryption().secret_storage().open_secret_store(&passphrase.as_str()).await?;
                println!("🔓 Opened secret store, importing secrets");
                secret_store.import_secrets().await?;
                println!("✅ Successfully imported secrets");
            } else {
                println!("❌ Secret storage is not enabled despite having a recovery key");
            }
        } else {
            println!("❓ No secret storage recovery key found in database");
        }
        
        // check again
        if let Some(status) = client.encryption().cross_signing_status().await {
            println!("🔐 Cross-signing status after import: {:?}", status);
            // bootstrap cross signing keys here
            println!("🔄 Bootstrapping cross-signing keys");
            if let Err(e) = client.encryption().bootstrap_cross_signing(None).await {
                println!("⚠️ Bootstrap requires authentication");
                if let Some(response) = e.as_uiaa_response() {
                    println!("👤 Username: {}", username);
                    println!("🔑 Password: {}", password);
                    let user_identifier = matrix_sdk::ruma::api::client::uiaa::UserIdentifier::UserIdOrLocalpart(username.to_string());
                    let mut password_auth = matrix_sdk::ruma::api::client::uiaa::Password::new(user_identifier, password);
                    password_auth.session = response.session.clone();
                    
                    println!("🔄 Retrying bootstrap with authentication");
                    client.encryption()
                        .bootstrap_cross_signing(Some(matrix_sdk::ruma::api::client::uiaa::AuthData::Password(password_auth)))
                        .await
                        .expect("couldn't bootstrap cross signing");

                    println!("✅ Successfully bootstrapped cross-signing with authentication");
                } else {
                    println!("❌ Error during cross signing bootstrap: {:#?}", e);
                    panic!("Error during cross signing bootstrap {:#?}", e);
                }
            } else {
                println!("✅ Successfully bootstrapped cross-signing");
            }
            redo_secret_storage = true;
            println!("🚩 Marked for secret storage update");
        }
    } else {
        println!("ℹ️ No cross-signing status available");
    }

    println!("🔄 Checking encryption backups");
    let backups_enabled = client.encryption().backups().are_enabled().await;
    println!("🔐 Backups enabled: {}", backups_enabled);
    
    if !backups_enabled {
        println!("❌ Backups not enabled, attempting to set up");
        if let Some(key) = user.encrypted_matrix_secret_storage_recovery_key.clone() {
            println!("🔑 Found encrypted recovery key in database");
            if client.encryption().secret_storage().is_enabled().await.unwrap() {
                println!("✅ Secret storage is enabled, importing secrets for backups");
                let passphrase = decrypt_token(&key.as_str()).unwrap();
                println!("🔑 Decrypted recovery key (length: {})", passphrase.len());
                let secret_store = client.encryption().secret_storage().open_secret_store(&passphrase.as_str()).await?;
                println!("🔓 Opened secret store, importing secrets");
                secret_store.import_secrets().await?;
                println!("✅ Successfully imported secrets for backups");
            } else {
                println!("❌ Secret storage is not enabled despite having a recovery key");
                redo_secret_storage = true;
                println!("🚩 Marked for secret storage update");
            }        
        } else {
            println!("❓ No recovery key found, will create new secret storage");
            // we will just set the flag and backups get created anyways later
            redo_secret_storage = true;
            println!("🚩 Marked for secret storage update");
        }
    }

    if redo_secret_storage {
        println!("🔄 Updating secret storage");
        // run secret storage create and cross signing keys + backup key will go there automatically.
        if let Some(encrypted_key) = user.encrypted_matrix_secret_storage_recovery_key.clone() {
            println!("🔑 Using existing recovery key");
            let passphrase = decrypt_token(&encrypted_key.as_str()).unwrap();
            println!("🔑 Decrypted recovery key (length: {})", passphrase.len());
            // recovery enable will create a new secret store and make sure backups enabled
            println!("🔄 Enabling recovery with existing passphrase");
            client.encryption().recovery().enable().with_passphrase(&passphrase.as_str()).await?;
            println!("✅ Successfully enabled recovery with existing passphrase");
        } else {
            println!("🔑 Creating new recovery key");
            // recovery enable will create a new secret store and make sure backups enabled
            let recovery = client.encryption().recovery();
            let enable = recovery
                .enable()
                .wait_for_backups_to_upload();

            println!("🔄 Enabling recovery and waiting for backups");
            let recovery_key = enable.await?;
            println!("✅ Successfully created new recovery key");
            println!("💾 Saving recovery key to database");
            user_repository.set_matrix_secret_storage_recovery_key(user.id, &recovery_key)?;
            println!("✅ Successfully saved recovery key");
        }
    }

    // sync for rooms keys
    println!("🔄 Performing initial sync to get room keys");
    client.sync_once(matrix_sdk::config::SyncSettings::new()).await?;
    println!("✅ Initial sync complete");
    
    // Return the fully initialized client
    println!("✅ Matrix client fully initialized for user {}", user_id);
    Ok(client)
}
/*

if !user.matrix_password {
	//register a new user and save tokens
	res = register_user(user)
	store_to_db(res.username, res.password, res.access_token, res.device_id)
}
if !user.logged_in() { // (client store does not have device id and access token)
	// if we have still a stored device id in our db
	if user.device_id { 
		let res = client.login_token(user.access_token).device_id(user.device_id)
		if res.success() {
			// our db tokens were valid at the server and if client store ones differed 
			// they will now be overriden to match our db.
		} else { // (our db credentials were invalid)
			// try logging in with username and password stored in our db
			let res = client.login_username() // this creates a new device for the user
			// store the new res.access_token and res.device_id to our db 
			store_to_db(res.access_token, res.device_id)
			// and they have also been overriden to client store
			// Enforce single-device policy: delete other devices
			client.delete_devices_except(res.device_id)
		}
	} else {
		// try logging in with username and password stored in our db
		let res = client.login_username() // this creates new device for user
		// store the new res.access_token and res.device_id to our db 
		store_to_db(res.access_token, res.device_id)
		// and they have also been overriden to client store
		// Enforce single-device policy: delete other devices
		client.delete_devices_except(res.device_id)
	}
} else { // (client store has device id and access token) 
	// call whoami to check if client store device id and access token are valid at the server also
	let res = client.whoami();
	if !res.device_id_exists or res.device_id != user.device_id {
		// try logging in with username and password stored in our db
		let res = client.login_username()
		// store the new res.access_token and res.device_id to our db
		store_to_db(res.access_token, res.device_id)
		// and they have also been overriden to client store
		// Enforce single-device policy: delete other devices
		client.delete_devices_except(res.device_id)
	}
}
// here we should have all synced up and logged in across(our db, client store and server)

// Handle cross-signing keys 


// check cross signing status locally
client.encryption().cross_signing_status()
// check cross signing status of user on the server. not found means it is not setup
client.encryption().request_user_identify(user_id)
// Create and upload a new cross signing identity to the client and public keys to the server
client.encryption().bootstrap_cross_signing()

// Enable secret storage and backups. This method will create a new secret storage key and a new backup if one doesn’t already exist. 
// It will then upload all the locally cached secrets, including the backup recovery key, to the new secret store.
client.encryption().recovery().enable()
// Reset the recovery key. This will rotate the secret storage key and re-upload all the secrets to the SecretStore.
client.encryption().recovery().reset_key()
// Recover all the secrets from the homeserver.
client.encryption().recovery().recover(recovery_key)

// check if backups exists at the server
client.encryption().backups().fetch_exists_on_server()
// check if we have active backup key on client and ready to upload room keys to server
client.encryption().backups().are_enabled()
// create new backup to server and a new recovery key locally as m.secret.send
client.encryption().backups().create()

// open secret store on the server so we can fetch or modify the keys there
client.encryption().secret_storage().open_secret_store(secret_storage_key)
// create new secret store. the passphrase can be fetched with client.encryption().secret_store().secret_storage_key()
// if we have secrets on the client, they will be automatically sent to the secret store with this(cross signing keys, backups key)
client.encryption().secret_storage().create_secret_store()
// check is secret storage has been setup on the server
client.encryption().secret_storage().is_enabled()

// Retrieve and store well-known secrets locally (cross signing keys, backup key)
client.encryption().secret_store().import_secrets()
// store a key value pair in open secret storage
client.encryption().secret_store().put_secret()
// fetch a key value pair from open secret storage
client.encryption().secret_store().get_secret()

so...
quick checks:
client.encryption().cross_signing_status() // check cross signing status locally
if client.encryption().backups().are_enabled() { // check if we have active backup key on client and ready to upload room keys to server
    if client.encryption().backups().fetch_exists_on_server() { // check if backups exists at the server
        
    } else {
        // creates new backup to server and saves new recovery key locally as m.secret.send
        client.encryption().backups().create()
    }
}

// TODO do moodle kandi things today
    let redo_secret_storage = false;
    if let Some(status) = client.encryption().cross_signing_status().await {
        if user.secret_store_key and client.encryption().secret_storage().is_enabled() {
            client.encryption().secret_storage().open_secret_store().await?;
            client.encryption().secret_storage().import_secrets().await?;
        }
        // check again
        if let Some(status) = client.encryption().cross_signing_status().await {
            // bootstrap cross signing keys here
            if let Err(e) = client.encryption().bootstrap_cross_signing(None).await {
                if let Some(response) = e.as_uiaa_response() {
                    let user_id_str = client.user_id().unwrap().to_string();
                    let username = user_id_str.strip_prefix('@')
                        .and_then(|s| s.split(':').next())
                        .ok_or_else(|| anyhow!("Invalid user ID format"))?;
                    let password = generate_matrix_password(&user_id_str);
                    let user_identifier = matrix_sdk::ruma::api::client::uiaa::UserIdentifier::UserIdOrLocalpart(username.to_string());
                    let mut password_auth = matrix_sdk::ruma::api::client::uiaa::Password::new(user_identifier, password);
                    password_auth.session = uiaa_info.session.clone();
                    
                    client.encryption()
                        .bootstrap_cross_signing(Some(matrix_sdk::ruma::api::client::uiaa::AuthData::Password(password_auth)))
                        .await
                        .expect("couldn't bootstrap cross signing");
                } else {
                    panic!("Error during cross signing bootstrap {:#?}", e);
                }
            }
            redo_secret_storage = true;
        }
    }
    if !backups okay {
        if user.secret_store_key and client.encryption().secret_storage().is_enabled() {
            client.encryption().secret_storage().open_secret_store().await?;
            client.encryption().secret_storage().import_secrets().await?;
        } else {
            // we will just set the flag and backups get created anyways later
            redo_secret_storage = true;
        }
    }

    if redo_secret_storage {
        // run secret storage create and cross signing keys + backup key will go there automatically.
        if user.secret_store_key {
            let passphrase = decrypt_token(user.encrypted_matrix_secret_storage_recovery_key.unwrap()).unwrap();
            // recovery enable will create a new secret store and make sure backups enabled
            client.encryption().recovery().enable().with_passphrase(passphrase).await;
        } else {
            // recovery enable will create a new secret store and make sure backups enabled
            client.encryption().recovery().enable().await;
            let new_key = client.encryption().secret_store().secret_storage_key();
            user_repository.set_matrix_secret_storage_recovery_key(user.id, new_key)?;
        }
    }
}




let create_new_secret_store = true;
if !client.cross_signing_keys_exist() || !client.encryption().backups().are_enabled() { 
	if !user.secret_storage_key { 
        // bootstrap new cross signing keys to client
		client.secret_storage.create_secret_storage() // Save new recovery key to database
		store_to_db(secret_storage_key = bootstrap_result.recovery_key) 
	} else {
		client.secret_storage.import_secrets(user.secret_storage_key) 
	}
	if secret_storage.has_cross_signing_keys() 
		try { 
			// Restore cross-signing keys from Secret Storage
			client.secret_storage.import_secrets(user.secret_storage_key) 
		} catch (error) { 
			// Restoration failed (e.g., key mismatch); bootstrap new keys 
			bootstrap_result = client.encryption.bootstrap_cross_signing(None)
			client.secret_storage.bootstrap_secret_storage() // Save new recovery key to database 
			store_to_db(secret_storage_key = bootstrap_result.recovery_key) 
		} 
	} else { // No keys in Secret Storage or no SSK; bootstrap new keys 
		bootstrap_result = client.encryption.bootstrap_cross_signing(true) 
	/ 	// figure out if the keys will go automatically to the secret store
	} 
}

// Handle room key backups 
if !client.encryption.backup_enabled() { 
	if user.secret_storage_key { 
		client.secret_storage.create_secret_storage() // Save new recovery key to database 
		store_to_db(secret_storage_key = bootstrap_result.recovery_key)
	} else {
		// do we need to open the store here or?
	}
	if secret_storage.has_backup_key() {
		backup_key = client.secret_storage.get_secret("m.room_keys.backup") 
		client.encryption.backup.restore_backup(backup_key) 
	} else { // Enable backup and store key in Secret Storage 
		backup_key = client.encryption.backup.enable() 
		client.secret_storage.store_secret("m.room_keys.backup", backup_key) 
	} 
}
// Sync client to initialize room state and E2EE metadata 
client.sync_once()
    */
