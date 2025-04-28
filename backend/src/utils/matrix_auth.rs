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
        match crate::utils::matrix_auth::get_or_create_matrix_client(user_id, &user_repository).await {
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
        match get_or_create_matrix_client(*user_id, &state.user_repository).await {
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
    println!("🔑 Generated password: [hidden]");

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
    let register_res = response
        .text()
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

    


/// Sets up cross-signing for a new user.
/// 
/// Cross-signing allows a user to verify their devices and establish trust between them.
/// This is important for secure end-to-end encrypted communication.

pub async fn setup_cross_signing(client: &MatrixClient, user_id: i32, user_repository: &UserRepository) -> Result<String> {
    tracing::info!("Checking cross-signing status for user {}", client.user_id().unwrap());
    
    let cross_signing_status = client.encryption().cross_signing_status().await
        .ok_or_else(|| anyhow!("Failed to get cross-signing status"))?;
    
    if cross_signing_status.is_complete() {
        tracing::info!("Cross-signing already set up for user {}", client.user_id().unwrap());
        // Check if secret storage is enabled
        let secret_storage_enabled = client.encryption().secret_storage().is_enabled().await?;
        if secret_storage_enabled {
            // Fetch existing recovery key (secret storage key) from DB
            if let Some(recovery_key) = user_repository.get_matrix_secret_storage_recovery_key(user_id)? {
                return Ok(recovery_key);
            } else {
                // Secret storage is enabled but no recovery key found in DB
                tracing::warn!("Secret storage enabled but no recovery key found in DB for user {}. Creating new secret store.", user_id);
                
                // Create a new secret store
                let secret_store = client.encryption().secret_storage().create_secret_store().await?;
                let new_recovery_key = secret_store.secret_storage_key();
                
                // Store the new recovery key in the database
                user_repository.set_matrix_secret_storage_recovery_key(user_id, &new_recovery_key)?;
                
                tracing::info!("Created and stored new recovery key for user {}", user_id);
                return Ok(new_recovery_key);
            }
        } else {
            // Secret storage not enabled, proceed to set it up
            tracing::info!("Secret storage not enabled, setting up...");
        }
    }
    
    tracing::info!("Setting up cross-signing for user {}", client.user_id().unwrap());
    
    let bootstrap_result = client.encryption().bootstrap_cross_signing(None).await;
    
    match bootstrap_result {
        Ok(_) => {
            tracing::info!("Successfully set up cross-signing for user {}", client.user_id().unwrap());
            // Create a new secret store (SSSS)
            let secret_store = client.encryption().secret_storage().create_secret_store().await?;
            // Get the secret storage key (our "recovery key")
            let recovery_key = secret_store.secret_storage_key();
            // store the recovery key in your database
            user_repository.set_matrix_secret_storage_recovery_key(user_id, &recovery_key)?;
            tracing::info!("Secret storage set up with key: {}", recovery_key);
            // Enable key backup for room keys
            client.encryption().backups().create().await
                .map_err(|e| anyhow!("Failed to enable key backup: {}", e))?;
            tracing::info!("Enabled key backup for room keys for user ID: {}", user_id);
            Ok(recovery_key)
        },
        Err(err) => {
            if let Some(uiaa_info) = err.as_uiaa_response() {
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
                    .await?;
                
                // Create a new secret store after successful authentication
                let secret_store = client.encryption().secret_storage().create_secret_store().await?;
                let recovery_key = secret_store.secret_storage_key();
                user_repository.set_matrix_secret_storage_recovery_key(user_id, &recovery_key)?;
                tracing::info!("Secret storage set up with key: {}", recovery_key);
                // Enable key backup for room keys
                client.encryption().backups().create().await
                    .map_err(|e| anyhow!("Failed to enable key backup: {}", e))?;
                tracing::info!("Enabled key backup for room keys for user ID: {}", user_id);
                Ok(recovery_key)
            } else {
                Err(anyhow!("Failed to set up cross-signing: {:?}", err))
            }
        }
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
    println!("🔓 Decrypting access token...");
    let encryption_key = std::env::var("ENCRYPTION_KEY")
        .map_err(|_| anyhow!("ENCRYPTION_KEY not set"))?;
    
    let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);
    cipher.decrypt_base64_to_string(encrypted_token)
        .map_err(|e| anyhow!("Failed to decrypt token: {}", e))
}

/// Gets or creates a Matrix client for a user
///
/// This function checks if a Matrix user already exists for the given user ID.
/// If it does, it logs in with the existing credentials to get a fresh token.
/// If not, it creates a new user.
///
/// # Arguments
/// * `user_id` - The user ID to get or create a Matrix client for
/// * `user_repository` - store containing user repository methods of manipulation db
///
/// # Returns
/// A logged-in Matrix client ready for use with fresh credentials
pub async fn get_or_create_matrix_client(user_id: i32, user_repository: &UserRepository) -> Result<MatrixClient> {
    tracing::info!("Getting or creating Matrix client for user ID: {}", user_id);
    
    // Get homeserver URL and shared secret from environment
    let homeserver_url = std::env::var("MATRIX_HOMESERVER")
        .map_err(|_| anyhow!("MATRIX_HOMESERVER not set"))?;
    let shared_secret = std::env::var("MATRIX_SHARED_SECRET")
        .map_err(|_| anyhow!("MATRIX_SHARED_SECRET not set"))?;
    
    // Check if we already have credentials for this user
    let existing_credentials = user_repository.get_matrix_credentials(user_id)
        .map_err(|e| anyhow!("Database error when checking for existing credentials: {}", e))?;
    
    let (username, access_token, device_id, password) = if let Some((username, _, _, password)) = existing_credentials.clone() {
        tracing::info!("Found existing Matrix credentials for user ID: {}, logging in to refresh token", user_id);
        
        // We have existing credentials, but we'll log in again to get a fresh token
        // First, try to delete the old device if possible
        if let Some((_, old_token, old_device_id, _)) = &existing_credentials {
            // Get domain from homeserver URL
            let url = Url::parse(&homeserver_url)?;
            let domain = url.host_str().ok_or_else(|| anyhow!("No host in homeserver URL"))?;
            let full_user_id = format!("@{}:{}", username, domain);
            
            // Try to delete the old device, but don't fail if this doesn't work
            if let Err(e) = delete_device(&homeserver_url, &shared_secret, &full_user_id, old_device_id).await {
                tracing::warn!("Failed to delete old Matrix device: {}", e);
            }
        }
        
        // Log in with username and password to get fresh credentials
        let (new_access_token, new_device_id) = login_user(&homeserver_url, &username, &password).await?;
        
        // Update the credentials in the database
        user_repository.set_matrix_credentials(user_id, &username, &new_access_token, &new_device_id, &password)
            .map_err(|e| anyhow!("Failed to update Matrix credentials: {}", e))?;
        
        tracing::info!("Updated Matrix credentials with fresh token for user ID: {}", user_id);
        
        (username, new_access_token, new_device_id, password)
    } else {
        tracing::info!("No existing Matrix credentials found, registering new user for user ID: {}", user_id);
        
        // Register a new user
        let (username, access_token, device_id, password) = register_user(&homeserver_url, &shared_secret).await?;
        
        // Store the new credentials in the database
        user_repository.set_matrix_credentials(user_id, &username, &access_token, &device_id, &password)
            .map_err(|e| anyhow!("Failed to store Matrix credentials: {}", e))?;
        
        tracing::info!("Stored new Matrix credentials for user ID: {}", user_id);
        
        (username, access_token, device_id, password)
    };
    
    // Initialize the Matrix client
    let store_path = format!(
        "{}/{}",
        std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
            .map_err(|_| anyhow!("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"))?,
        username
    );
    // Purge the existing store if it exists
    if std::path::Path::new(&store_path).exists() {
        std::fs::remove_dir_all(&store_path)
            .map_err(|e| anyhow!("Failed to remove existing store directory {}: {}", store_path, e))?;
        tracing::info!("Purged existing store directory at {}", store_path);
    }
    
    std::fs::create_dir_all(&store_path)
        .map_err(|e| anyhow!("Failed to create store directory {}: {}", store_path, e))?;
    
    // Get domain from homeserver URL
    let url = Url::parse(&homeserver_url)
        .map_err(|e| anyhow!("Invalid homeserver URL: {}", e))?;
    let domain = url.host_str()
        .ok_or_else(|| anyhow!("No host in homeserver URL"))?;
    
    let full_user_id = format!("@{}:{}", username, domain);
    
    let client = MatrixClient::builder()
        .homeserver_url(&homeserver_url)
        .sqlite_store(store_path, None)
        .build()
        .await
        .map_err(|e| anyhow!("Failed to build Matrix client: {}", e))?;
    
    // Restore session with the fresh access token
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

    // Check if secret storage is enabled
    let secret_storage_enabled = client.encryption().secret_storage().is_enabled().await?;
    if secret_storage_enabled {
        // Retrieve the recovery key (secret storage key)
        let recovery_key = user_repository.get_matrix_secret_storage_recovery_key(user_id)?
            .ok_or_else(|| anyhow!("No recovery key found for user ID: {}", user_id))?;
        
        // Open the secret store to access backed-up keys
        client.encryption().secret_storage().open_secret_store(&recovery_key).await?;
        tracing::info!("Opened secret store for user ID: {}", user_id);
    }
    
    // Perform initial sync to get room state
    client.sync_once(MatrixSyncSettings::default())
        .await
        .map_err(|e| anyhow!("Failed to perform initial sync: {}", e))?;
    
    // Set up cross-signing if needed
    if let Err(e) = setup_cross_signing(&client, user_id, user_repository).await {
        tracing::warn!("Failed to set up cross-signing: {}", e);
    }
    
    tracing::info!("Matrix client initialized successfully for user ID: {}", user_id);
    Ok(client)
}


// create client with the client sqlite store(that stores both state and encryption keys)

pub async fn login_with_password(client: &MatrixClient, user_repository: &UserRepository, username: &str, password: &str, user_id: i32) ->Result<()> {
    let res = client.matrix_auth()
            .login_username(username, password)
            .send()
            .await;
    if let Ok(response) = res {
        // store the new device_id and access_token
        user_repository.set_matrix_device_id_and_access_token(user_id, &response.access_token, &response.device_id.as_str())?;
        // TODO delete old devices from client store and possibly from server also if we can
    } else {
        // TODO we should fail here and inform the user that new registration has to be created
    }
    Ok(())
}


pub async fn get_client(user_id: i32, user_repository: &UserRepository) -> Result<MatrixClient> {

    // Get user profile from database
    let user = user_repository.find_by_id(user_id).unwrap().unwrap();

    // Initialize the Matrix client
    let homeserver_url = std::env::var("MATRIX_HOMESERVER")
        .map_err(|_| anyhow!("MATRIX_HOMESERVER not set"))?;
    let shared_secret = std::env::var("MATRIX_SHARED_SECRET")
        .map_err(|_| anyhow!("MATRIX_SHARED_SECRET not set"))?;
    
    let store_path = format!(
        "{}/{}",
        std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
            .map_err(|_| anyhow!("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"))?,
        user.matrix_username.unwrap()
    );
    
    std::fs::create_dir_all(&store_path)
        .map_err(|e| anyhow!("Failed to create store directory {}: {}", store_path, e))?;

    // if matrix user doesn't exist, register one
    if let Some(username) = user.matrix_username {
        let (username, access_token, device_id, password) = register_user(&homeserver_url, &shared_secret).await?;
        user_repository.set_matrix_credentials(user.id, &username, &access_token, &device_id, &password)?;
    }
    
    let client = MatrixClient::builder()
        .homeserver_url(&homeserver_url)
        .sqlite_store(store_path, None)
        .build()
        .await
        .unwrap();
    
    // logged_in checks if client store has device_id and access token stored
    if !client.logged_in() {
        if let Some(device_id) = user.matrix_device_id {
            let access_token = decrypt_token(user.encrypted_matrix_access_token.unwrap().as_str()).unwrap();
            let res = client.matrix_auth()
                        .login_token(&access_token.as_str())
                        .device_id(device_id.as_str())
                        .send()
                        .await;
            if let Ok(response) = res {
                // if client store tokens and device were different they will now at least get overriden
            } else {
                // if fails, we try logging in with username and password
                let password = decrypt_token(user.encrypted_matrix_password.unwrap().as_str()).unwrap();
                login_with_password(&client, user_repository, user.matrix_username.unwrap(), password, user.id);
            }
            
            
        } else {
            // if not device_id was stored, let's try logging in with username and password
            let password = decrypt_token(user.encrypted_matrix_password.unwrap()).unwrap();
            login_with_password(&client, user_repository, user.matrix_username, password, user.id);
        }
    } else { // client store has some device id and access token already
	    // call whoami to check if client store device id and access token are valid at the server also
	    let res = client.whoami();
        match res.device_id {
            Some(owned_device_id) => {
                if owned_device_id.as_str() != user.matrix_device_id.unwrap() {
                    let password = decrypt_token(user.encrypted_matrix_password.unwrap()).unwrap();
                    // we are lazy and just create new device if they don't match
                    login_with_password(&client, user_repository, user.matrix_username, password, user.id);
                } 
            },
            None => {
                // create new device if none was found on the server
                let password = decrypt_token(user.encrypted_matrix_password.unwrap()).unwrap();
                login_with_password(&client, user_repository, user.matrix_username, password, user.id);
            }
        }
    }
    // here we should have client store, our db and server synced with the same device id and access token
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
if !client.cross_signing_keys_exist() { 
	if !user.secret_storage_key { 
		client.secret_storage.create_secret_storage() // Save new recovery key to database 
		store_to_db(secret_storage_key = bootstrap_result.recovery_key) 
	} else {
		// do we need to open the store here or?
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
		// figure out if the keys will go automatically to the secret store
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
