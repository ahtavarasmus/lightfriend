use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use reqwest::Client as HttpClient;
use crate::repositories::user_repository::UserRepository;
use serde_json::json;
use sha1::Sha1;
use uuid::Uuid;
use crate::AppState;
use matrix_sdk::{
    Client as MatrixClient,
    ruma::OwnedUserId,
};
use std::sync::Arc;
use reqwest;
use tokio::time::{sleep, Duration};
use url::Url;
use sha2::Digest;
use tokio::sync::Mutex;
use std::collections::HashMap;
use crate::utils::encryption::{encrypt, decrypt};


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
    let invited_rooms: Vec<_> = client.invited_rooms();

    if invited_rooms.is_empty() {
        tracing::info!("No room invitations found");
        return Ok(0);
    }
    
    tracing::info!("Found {} room invitations", invited_rooms.clone().len());
    let mut joined_count = 0;
    
    for room in invited_rooms.clone() {
        let room_id = room.room_id();
        tracing::info!("Attempting to join room");
        
        match client.join_room_by_id(room_id).await {
            Ok(_) => {
                tracing::info!("Successfully joined room");
                joined_count += 1;
            }
            Err(e) => {
                tracing::error!("Failed to join room {}", e);
                // Continue with other rooms even if one fails
            }
        }

        // Take a breath - wait 1 second between each room join
        sleep(Duration::from_secs(1)).await;
    }
    
    tracing::info!("Joined {} rooms out of {} invitations", joined_count, invited_rooms.len());
    Ok(joined_count)
}


pub async fn register_user(homeserver: &str, shared_secret: &str) -> Result<(String, String, String, String)> {
    tracing::info!("🔑 Starting Matrix user registration...");
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

    // Generate unique username and password
    let username = format!("appuser_{}", Uuid::new_v4().to_string().replace("-", ""));
    let password = Uuid::new_v4().to_string();
    tracing::info!("👤 Generated username and 🔑 password");

    // Calculate MAC
    let mac_content = format!("{}\0{}\0{}\0notadmin", nonce, username, password);
    let mut mac = Hmac::<Sha1>::new_from_slice(shared_secret.as_bytes())
        .map_err(|e| anyhow!("Failed to create HMAC: {}", e))?;
    mac.update(mac_content.as_bytes());
    let mac_result = hex::encode(mac.finalize().into_bytes());

    // Register user
    tracing::info!("📡 Sending registration request to Matrix server...");
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
    tracing::debug!("📡 Registration response status: {}", status);

    // Get response body
    let register_res = response .text()
        .await
        .map_err(|e| anyhow!("Failed to read response body: {}", e))?;
    tracing::debug!("📡 Registration response body: {}", register_res);

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
        tracing::debug!("✅ Matrix registration successful!");
        Ok((username, access_token, device_id, password))
    } else {
        let error = register_json["error"]
            .as_str()
            .unwrap_or("Unknown error");
        Err(anyhow!("Registration failed: {} (status: {})", error, status))
    }
}

pub async fn login_with_password(client: &MatrixClient, state: &Arc<AppState>, username: &str, password: &str, device_id: Option<&str>, user_id: i32) ->Result<()> {
    tracing::info!("🔑 Attempting to login with username and password and existing device");
    let res;
    if let Some(device_id) = device_id {
        tracing::debug!("using existing device_id");
        res = client.matrix_auth()
            .login_username(username, password)
            .device_id(device_id)
            .send()
            .await;
    } else {
        tracing::debug!("creating new device_id");
        res = client.matrix_auth()
            .login_username(username, password)
            .send()
            .await;
    }
    if let Ok(response) = res {
        tracing::info!("✅ Login successful");
        
        // Store the new device_id and access_token
        tracing::debug!("💾 Saving new device ID and access token to database");
        state.user_repository.set_matrix_device_id_and_access_token(user_id, &response.access_token, &response.device_id.as_str())?;
        tracing::debug!("✅ Successfully saved credentials");
        
    } else {
        tracing::error!("❌ Login failed: {:?}", res.err());
        return Err(anyhow!("Failed to login with username and password. User may need to be re-registered."));
    }
    tracing::info!("✅ Login with password completed successfully");
    Ok(())
}


pub async fn get_client(user_id: i32, state: &Arc<AppState>) -> Result<MatrixClient> {
    tracing::info!("🔄 Starting get_client for user_id: {}", user_id);

    // Get user profile from database
    let user = state.user_core.find_by_id(user_id).unwrap().unwrap();
    tracing::debug!("👤 Found user: id={}", user.id);

    // Initialize the Matrix client
    let homeserver_url = std::env::var("MATRIX_HOMESERVER")
        .map_err(|_| anyhow!("MATRIX_HOMESERVER not set"))?;
    let shared_secret = std::env::var("MATRIX_SHARED_SECRET")
        .map_err(|_| anyhow!("MATRIX_SHARED_SECRET not set"))?;

    // Get or register Matrix credentials
    let (username, password, device_id, access_token) = if user.matrix_username.is_none() {
        tracing::info!("🆕 Registering new Matrix user");
        let (username, access_token, device_id, password) = register_user(&homeserver_url, &shared_secret).await?;
        state.user_repository.set_matrix_credentials(user.id, &username, &access_token, &device_id, &password)?;
        (username, password, Some(device_id), Some(access_token))
    } else {
        tracing::debug!("✓ Existing Matrix credentials found");
        let access_token = user.encrypted_matrix_access_token.as_ref().map(|t| decrypt(t)).transpose()?;
        (user.matrix_username.unwrap(), decrypt(user.encrypted_matrix_password.as_ref().unwrap())?, user.matrix_device_id, access_token)
    };
 
    let store_path = format!(
        "{}/{}",
        std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
            .map_err(|_| anyhow!("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"))?,
            username
    );
    
    std::fs::create_dir_all(&store_path)
        .map_err(|e| anyhow!("Failed to create store directory {}", e))?;

    // Get domain from homeserver URL
    let url = Url::parse(&homeserver_url)
        .map_err(|e| anyhow!("Invalid homeserver URL: {}", e))?;
    let domain = url.host_str()
        .ok_or_else(|| anyhow!("No host in homeserver URL"))?;
    
    let full_user_id = format!("@{}:{}", username, domain);

    tracing::debug!("🔨 Building Matrix client");
    let client = MatrixClient::builder()
        .homeserver_url(&homeserver_url)
        .sqlite_store(store_path, None)
        .build()
        .await
        .unwrap();
    tracing::debug!("✅ Matrix client built successfully");
    
    // logged_in checks if client store has device_id and access token stored
    tracing::debug!("🔑 Checking if client is logged in: {}", client.logged_in());

    // Attempt to restore session
    let mut session_restored = false;
    if let Some(stored_session) = client.matrix_auth().session() {
        tracing::debug!("🔄 Found session in store, attempting to restore");
        if let Err(e) = client.matrix_auth().restore_session(stored_session.clone()).await {
            tracing::debug!("⚠️ Failed to restore session from store: {}", e);
        } else {
            tracing::debug!("✅ Session restored from store");
            session_restored = true;
            // Verify session validity
            if let Ok(response) = client.whoami().await {
                tracing::debug!("🔍 Server reports user_id: {}", response.user_id);
                // Update database if credentials changed
                state.user_repository.set_matrix_credentials(
                    user.id,
                    &username,
                    &stored_session.tokens.access_token,
                    &response.device_id.expect("default").as_str(),
                    &password,
                )?;
            } else {
                tracing::debug!("❌ Restored session is invalid, will attempt re-authentication");
                session_restored = false;
            }
        }
    }

    // If no valid session was restored, try token-based login or password login
    if !session_restored {
        tracing::debug!("🔑 No valid session restored, attempting authentication");
        if let Some(access_token) = access_token {
            tracing::debug!("🔄 Attempting token-based login");

            let session = matrix_sdk::authentication::matrix::MatrixSession {
                meta: matrix_sdk::SessionMeta {
                    user_id: OwnedUserId::try_from(full_user_id.clone()).unwrap(),
                    device_id: matrix_sdk::ruma::OwnedDeviceId::try_from(device_id.clone().unwrap()).unwrap(),
                },
                tokens: matrix_sdk::authentication::matrix::MatrixSessionTokens {
                    access_token: access_token.clone(),
                    refresh_token: None,
                },
            };
            if let Ok(_) = client.matrix_auth().restore_session(session.clone()).await {
                tracing::debug!("✅ Token-based session restored");
                // Verify session
                if let Ok(response) = client.whoami().await {
                    state.user_repository.set_matrix_credentials(
                        user.id,
                        &username,
                        &access_token.as_str(),
                        &response.device_id.expect("default").as_str(),
                        &password,
                    )?;
                    session_restored = true;
                }
            }
        }

        // Fallback to password login if token-based login fails
        if !session_restored {
            tracing::debug!("🔄 Attempting password-based login");
            login_with_password(&client, &state, &username, &password, device_id.as_deref(), user.id).await?;
        }
    }
    tracing::info!("✅ Authentication complete - client is logged already in");
    // here we should have client store, our db and server synced with the same device id and access token
    
    tracing::info!("✅ Matrix client fully initialized for user {}", user_id);
    Ok(client)
}

/// Get a cached Matrix client from AppState, with fallback to creating a new client
/// Note: The fallback client is not stored in the cache - that's managed by the scheduler
pub async fn get_cached_client(
    user_id: i32,
    state: &Arc<AppState>,
) -> Result<Arc<MatrixClient>> {
    // Get the matrix clients map from AppState
    let matrix_clients = state.matrix_clients.lock().await;
    
    // Try to get the client for this user
    if let Some(client) = matrix_clients.get(&user_id) {
        tracing::debug!("Found cached Matrix client for user {}", user_id);
        Ok(client.clone())
    } else {
        tracing::debug!("No cached Matrix client found for user {}, creating temporary client", user_id);
        // Drop the lock before the potentially long-running get_client operation
        drop(matrix_clients);
        
        // Create a new client as fallback
        match get_client(user_id, state).await {
            Ok(client) => {
                tracing::debug!("Successfully created temporary Matrix client for user {}", user_id);
                Ok(Arc::new(client))
            },
            Err(e) => {
                tracing::error!("Failed to create temporary Matrix client for user {}: {}", user_id, e);
                Err(anyhow!("Failed to create Matrix client: {}", e))
            }
        }
    }
}
