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

/// Sets up encryption backups for a Matrix client
/// 
/// # Arguments
/// * `client` - The Matrix client to set up backups for
/// * `encrypted_key` - Optional encrypted secret storage recovery key
/// 
/// # Returns
/// * `Ok(bool)` - Whether secret storage needs to be updated
/// * `Err` - Any error that occurred during setup
async fn setup_backups(
    client: &MatrixClient,
    encrypted_key: Option<&String>
) -> Result<bool> {
    println!("🔄 Checking encryption backups");
    let mut backups_enabled = client.encryption().backups().are_enabled().await;
    println!("🔐 Initial backups status: {}", backups_enabled);
    
    let mut needs_secret_storage_update = false;
    
    if !backups_enabled {
        println!("❌ Backups not enabled, attempting to restore");
        if let Some(key) = encrypted_key {
            println!("🔑 Found encrypted recovery key in database");
            if client.encryption().secret_storage().is_enabled().await? {
                println!("✅ Secret storage is enabled, importing secrets for backups");
                let passphrase = decrypt_token(key)?;
                println!("🔑 Decrypted recovery key (length: {})", passphrase.len());
                let secret_store = client.encryption().secret_storage().open_secret_store(&passphrase).await?;
                println!("🔓 Opened secret store, importing secrets");
                secret_store.import_secrets().await?;
                println!("✅ Successfully imported secrets");
                
                // Check if backups are now enabled after importing secrets
                backups_enabled = client.encryption().backups().are_enabled().await;
                println!("🔐 Backups status after import: {}", backups_enabled);
                
                if !backups_enabled {
                    println!("❌ Backups still not enabled after importing secrets");
                    needs_secret_storage_update = true;
                }
            } else {
                println!("❌ Secret storage is not enabled despite having a recovery key");
                needs_secret_storage_update = true;
            }
        } else {
            println!("❓ No recovery key found, will create new secret storage");
            needs_secret_storage_update = true;
        }
    }
    
    println!("✅ Backup setup complete. Needs storage update: {}", needs_secret_storage_update);
    Ok(needs_secret_storage_update)
}

/// Sets up cross-signing for a Matrix client
/// # Arguments
/// * `client` - The Matrix client to set up cross-signing for
/// * `username` - The username for authentication if needed
/// * `password` - The password for authentication if needed
/// * `encrypted_key` - Optional encrypted secret storage recovery key
/// 
/// # Returns
/// * `Ok(bool)` - Whether secret storage needs to be updated
/// * `Err` - Any error that occurred during setup
async fn setup_cross_signing(
    client: &MatrixClient,
    username: &str,
    password: &str,
    encrypted_key: Option<&String>
) -> Result<bool> {
    let mut needs_secret_storage_update = false;

    if let Some(cross_signing_status) = client.encryption().cross_signing_status().await {
        println!("🔐 Cross-signing status: {:?}", cross_signing_status);
        
        if !cross_signing_status.has_master || !cross_signing_status.has_self_signing || !cross_signing_status.has_user_signing {
            if let Some(encrypted_key) = encrypted_key {
                println!("🔑 Found encrypted secret storage recovery key in database");
                if client.encryption().secret_storage().is_enabled().await? {
                    println!("✅ Secret storage is enabled, importing secrets");
                    let passphrase = decrypt_token(encrypted_key)?;
                    println!("🔑 Decrypted recovery key (length: {})", passphrase.len());
                    let secret_store = client.encryption().secret_storage().open_secret_store(&passphrase).await?;
                    println!("🔓 Opened secret store, importing secrets");
                    secret_store.import_secrets().await?;
                    println!("✅ Successfully imported secrets");

                    // Check if we need to bootstrap after import
                    if let Some(status) = client.encryption().cross_signing_status().await {
                        if !status.has_master || !status.has_self_signing || !status.has_user_signing {
                            println!("🔐 Cross-signing status after import: {:?}", status);
                            needs_secret_storage_update = try_bootstrap_cross_signing(client, username, password).await?;
                        }
                    }
                } else {
                    println!("❌ Secret storage is not enabled despite having a recovery key");
                    needs_secret_storage_update = true;
                }
            } else {
                println!("❓ No secret storage recovery key found in database");
                needs_secret_storage_update = try_bootstrap_cross_signing(client, username, password).await?;
            }
        } else {
            println!("✅ Cross signing keys are found in local store. Moving on");
        }
    } else {
        println!("ℹ️ No cross-signing status available");
        needs_secret_storage_update = try_bootstrap_cross_signing(client, username, password).await?;
    }

    Ok(needs_secret_storage_update)
}

/// Attempts to bootstrap cross-signing, handling authentication if needed
async fn try_bootstrap_cross_signing(
    client: &MatrixClient,
    username: &str,
    password: &str,
) -> Result<bool> {
    println!("🔄 Bootstrapping cross-signing keys");
    match client.encryption().bootstrap_cross_signing(None).await {
        Ok(_) => {
            println!("✅ Successfully bootstrapped cross-signing");
            Ok(true)
        },
        Err(e) => {
            if let Some(response) = e.as_uiaa_response() {
                println!("⚠️ Bootstrap requires authentication");
                println!("👤 Username: {}", username);
                println!("🔑 Password: {}", password);
                
                let user_identifier = matrix_sdk::ruma::api::client::uiaa::UserIdentifier::UserIdOrLocalpart(username.to_string());
                let mut password_auth = matrix_sdk::ruma::api::client::uiaa::Password::new(user_identifier, password.to_string());
                password_auth.session = response.session.clone();
                
                println!("🔄 Retrying bootstrap with authentication");
                match client.encryption()
                    .bootstrap_cross_signing(Some(matrix_sdk::ruma::api::client::uiaa::AuthData::Password(password_auth)))
                    .await 
                {
                    Ok(_) => {
                        println!("✅ Successfully bootstrapped cross-signing with authentication");
                        Ok(true)
                    },
                    Err(e) => {
                        println!("❌ Error during cross signing bootstrap with auth: {}", e);
                        if e.to_string().contains("M_INVALID_SIGNATURE") || e.to_string().contains("already exists") {
                            println!("🔄 Detected {} error, attempting recovery...", 
                                if e.to_string().contains("M_INVALID_SIGNATURE") { "invalid signature" } else { "duplicate key" });
                            
                            // Clear the client's store path
                            let store_path = format!(
                                "{}/{}",
                                std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
                                    .map_err(|_| anyhow!("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"))?,
                                username
                            );
                            
                            println!("🗑️ Clearing store directory: {}", store_path);
                            if std::path::Path::new(&store_path).exists() {
                                std::fs::remove_dir_all(&store_path)?;
                                std::fs::create_dir_all(&store_path)?;
                            }
                            
                            Err(anyhow!("Matrix client needs to be reinitialized. Please try again."))
                        } else {
                            Err(anyhow!("Failed to bootstrap cross-signing: {}", e))
                        }
                    }
                }
            } else {
                println!("❌ Error during cross signing bootstrap: {:#?}", e);
                Err(anyhow!("Failed to bootstrap cross-signing: {}", e))
            }
        }
    }
}

pub async fn initialize_matrix_user_clients(
    user_repository: Arc<UserRepository>,
) -> Arc<Mutex<HashMap<i32, matrix_sdk::Client>>> {
    let user_ids = user_repository.get_users_with_matrix_bridge_connections().unwrap_or_default();
    let mut clients = HashMap::new();

    for user_id in user_ids {
    match crate::utils::matrix_auth::get_client(user_id, &user_repository, true).await {
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

    // Generate unique username and password
    let username = format!("appuser_{}", Uuid::new_v4().to_string().replace("-", ""));
    let password = Uuid::new_v4().to_string();
    println!("👤 Generated username and 🔑 password");

    // Calculate MAC
    let mac_content = format!("{}\0{}\0{}\0notadmin", nonce, username, password);
    let mut mac = Hmac::<Sha1>::new_from_slice(shared_secret.as_bytes())
        .map_err(|e| anyhow!("Failed to create HMAC: {}", e))?;
    mac.update(mac_content.as_bytes());
    let mac_result = hex::encode(mac.finalize().into_bytes());

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
        println!("🎫 Access token and device id received");
        Ok((username, access_token, device_id, password))
    } else {
        let error = register_json["error"]
            .as_str()
            .unwrap_or("Unknown error");
        Err(anyhow!("Registration failed: {} (status: {})", error, status))
    }
}


/// Encrypts a token for secure storage
/// 
/// # Arguments
/// * `token` - The token to encrypt
/// 
/// # Returns
/// The encrypted token as a base64 string
pub fn encrypt_token(token: &str) -> Result<String> {
    println!("🔒 Encrypting token...");
    let encryption_key = std::env::var("ENCRYPTION_KEY")
        .map_err(|_| anyhow!("ENCRYPTION_KEY not set"))?;
    
    let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);
    Ok(cipher.encrypt_str_to_base64(token))
}

/// Decrypts a previously encrypted token
/// 
/// # Arguments
/// * `encrypted_token` - The encrypted token in base64 format
/// 
/// # Returns
/// The decrypted token as a string
pub fn decrypt_token(encrypted_token: &str) -> Result<String> {
    println!("🔓 Decrypting token...");
    let encryption_key = std::env::var("ENCRYPTION_KEY")
        .map_err(|_| anyhow!("ENCRYPTION_KEY not set"))?;
    
    let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);
    cipher.decrypt_base64_to_string(encrypted_token)
        .map_err(|e| anyhow!("Failed to decrypt token: {}", e))
}


// create client with the client sqlite store(that stores both state and encryption keys)

pub async fn login_with_password(client: &MatrixClient, user_repository: &UserRepository, username: &str, password: &str, device_id: Option<&str>, user_id: i32) ->Result<()> {
    println!("🔑 Attempting to login with username and password and existing device");
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
        println!("🎫 New access token received");
        
        // Store the new device_id and access_token
        println!("💾 Saving new device ID and access token to database");
        user_repository.set_matrix_device_id_and_access_token(user_id, &response.access_token, &response.device_id.as_str())?;
        println!("✅ Successfully saved credentials");
        
    } else {
        println!("❌ Login failed: {:?}", res.err());
        return Err(anyhow!("Failed to login with username and password. User may need to be re-registered."));
    }
    println!("✅ Login with password completed successfully");
    Ok(())
}


pub async fn get_client(user_id: i32, user_repository: &UserRepository, setup_encryption: bool) -> Result<MatrixClient> {
    println!("🔄 Starting get_client for user_id: {}", user_id);

    // Get user profile from database
    let user = user_repository.find_by_id(user_id).unwrap().unwrap();
    println!("👤 Found user: id={}", user.id);

    // Initialize the Matrix client
    let homeserver_url = std::env::var("MATRIX_HOMESERVER")
        .map_err(|_| anyhow!("MATRIX_HOMESERVER not set"))?;
    let shared_secret = std::env::var("MATRIX_SHARED_SECRET")
        .map_err(|_| anyhow!("MATRIX_SHARED_SECRET not set"))?;

    // Get or register Matrix credentials
    let (username, password, device_id, access_token) = if user.matrix_username.is_none() {
        println!("🆕 Registering new Matrix user");
        let (username, access_token, device_id, password) = register_user(&homeserver_url, &shared_secret).await?;
        user_repository.set_matrix_credentials(user.id, &username, &access_token, &device_id, &password)?;
        (username, password, Some(device_id), Some(access_token))
    } else {
        println!("✓ Existing Matrix credentials found");
        let access_token = user.encrypted_matrix_access_token.as_ref().map(|t| decrypt_token(t)).transpose()?;
        (user.matrix_username.unwrap(), decrypt_token(user.encrypted_matrix_password.as_ref().unwrap())?, user.matrix_device_id, access_token)
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

    // Attempt to restore session
    let mut session_restored = false;
    if let Some(stored_session) = client.matrix_auth().session() {
        println!("🔄 Found session in store, attempting to restore");
        if let Err(e) = client.matrix_auth().restore_session(stored_session.clone()).await {
            println!("⚠️ Failed to restore session from store: {}", e);
        } else {
            println!("✅ Session restored from store");
            session_restored = true;
            // Verify session validity
            if let Ok(response) = client.whoami().await {
                println!("🔍 Server reports user_id: {}", response.user_id);
                // Update database if credentials changed
                user_repository.set_matrix_credentials(
                    user.id,
                    &username,
                    &stored_session.tokens.access_token,
                    &response.device_id.expect("default").as_str(),
                    &password,
                )?;
            } else {
                println!("❌ Restored session is invalid, will attempt re-authentication");
                session_restored = false;
            }
        }
    }

    // If no valid session was restored, try token-based login or password login
    if !session_restored {
        println!("🔑 No valid session restored, attempting authentication");
        if let Some(access_token) = access_token {
            println!("🔄 Attempting token-based login");

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
                println!("✅ Token-based session restored");
                // Verify session
                if let Ok(response) = client.whoami().await {
                    user_repository.set_matrix_credentials(
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
            println!("🔄 Attempting password-based login");
            login_with_password(&client, user_repository, &username, &password, device_id.as_deref(), user.id).await?;
        }
    }
    println!("✅ Authentication complete - client is logged already in");
    // here we should have client store, our db and server synced with the same device id and access token

    // handle keys now
    if setup_encryption {
        println!("🔐 Setting up encryption keys and secret storage");
        let mut redo_secret_storage = false;

        // Get bridge bot username from environment variable or use default pattern
        let bridge_bot_username = std::env::var("WHATSAPP_BRIDGE_BOT")
            .unwrap_or_else(|_| "@whatsappbot:".to_string());
            
            // Handle cross-signing setup
            match setup_cross_signing(&client, &username, &password, user.encrypted_matrix_secret_storage_recovery_key.as_ref()).await {
                Ok(should_update_storage) => {
                    if should_update_storage {
                        redo_secret_storage = true;
                        println!("🚩 Marked for secret storage update");
                    }
                },
                Err(e) => return Err(e),
            }

        println!("🔄 Setting up encryption backups");
        match setup_backups(&client, user.encrypted_matrix_secret_storage_recovery_key.as_ref()).await {
            Ok(should_update_storage) => {
                if should_update_storage {
                    redo_secret_storage = true;
                    println!("🚩 Marked for secret storage update from backups setup");
                }
            },
            Err(e) => return Err(e),
        }

        if redo_secret_storage {
            println!("🔄 Updating secret storage");
            // run secret storage create and cross signing keys + backup key will go there automatically.
            if let Some(encrypted_key) = user.encrypted_matrix_secret_storage_recovery_key.clone() {
                println!("🔑 Using existing recovery key");
                let passphrase = decrypt_token(&encrypted_key.as_str()).unwrap();
                println!("🔑 Decrypted recovery key");
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
        sleep(Duration::from_secs(2)).await;
        println!("✅ Initial sync complete");

    // Return the fully initialized client
    } else {
        println!("⏩ Skipping encryption setup as it's not needed for this operation");
    }
    
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
