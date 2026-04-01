use crate::utils::encryption::decrypt;
use crate::AppState;
use crate::UserCoreOps;
use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use matrix_sdk::store::RoomLoadSettings;
use matrix_sdk::{ruma::OwnedUserId, Client as MatrixClient};
use reqwest;
use reqwest::Client as HttpClient;
use serde_json::json;
use sha1::Sha1;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use url::Url;
use uuid::Uuid;

/// Register a new Matrix user via Synapse admin API (HMAC-SHA1 nonce flow).
/// Used on the VPS where Synapse is the homeserver.
pub async fn register_user_synapse(
    homeserver: &str,
    shared_secret: &str,
) -> Result<(String, String, String, String)> {
    tracing::info!("Starting Matrix user registration (Synapse admin API)...");
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

    // Calculate MAC
    let mac_content = format!("{}\0{}\0{}\0notadmin", nonce, username, password);
    let mut mac = Hmac::<Sha1>::new_from_slice(shared_secret.as_bytes())
        .map_err(|e| anyhow!("Failed to create HMAC: {}", e))?;
    mac.update(mac_content.as_bytes());
    let mac_result = hex::encode(mac.finalize().into_bytes());

    // Register user
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

    let status = response.status();
    let register_res = response
        .text()
        .await
        .map_err(|e| anyhow!("Failed to read response body: {}", e))?;

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
        tracing::info!("Matrix registration successful (Synapse)");
        Ok((username, access_token, device_id, password))
    } else {
        let error = register_json["error"].as_str().unwrap_or("Unknown error");
        Err(anyhow!(
            "Registration failed: {} (status: {})",
            error,
            status
        ))
    }
}

/// Register a new Matrix user using the standard UIAA registration_token flow.
/// Works with Tuwunel and any spec-compliant homeserver.
pub async fn register_user_token(
    homeserver: &str,
    registration_token: &str,
) -> Result<(String, String, String, String)> {
    tracing::info!("Starting Matrix user registration (UIAA token flow)...");
    let http_client = HttpClient::new();

    let username = format!("appuser_{}", Uuid::new_v4().to_string().replace("-", ""));
    let password = Uuid::new_v4().to_string();

    let register_url = format!("{}/_matrix/client/v3/register", homeserver);

    // Step 1: Initial request to get UIAA session
    let initial_response = http_client
        .post(&register_url)
        .json(&json!({
            "username": username,
            "password": password
        }))
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send initial registration request: {}", e))?;

    let initial_status = initial_response.status();
    let initial_body: serde_json::Value = initial_response
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse initial registration response: {}", e))?;

    // If 200 - registration succeeded without UIAA (unlikely but handle it)
    if initial_status.is_success() {
        let access_token = initial_body["access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("No access_token in response"))?
            .to_string();
        let device_id = initial_body["device_id"]
            .as_str()
            .ok_or_else(|| anyhow!("No device_id in response"))?
            .to_string();
        return Ok((username, access_token, device_id, password));
    }

    // Expect 401 with UIAA session
    if initial_status.as_u16() != 401 {
        let error = initial_body["error"].as_str().unwrap_or("Unknown error");
        return Err(anyhow!(
            "Registration failed at step 1: {} (status: {})",
            error,
            initial_status
        ));
    }

    let session = initial_body["session"]
        .as_str()
        .ok_or_else(|| anyhow!("No session in UIAA 401 response"))?;

    // Step 2: Complete registration with token
    let response = http_client
        .post(&register_url)
        .json(&json!({
            "auth": {
                "type": "m.login.registration_token",
                "token": registration_token,
                "session": session
            },
            "username": username,
            "password": password
        }))
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send registration request: {}", e))?;

    let status = response.status();
    let register_body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse registration response: {}", e))?;

    if status.is_success() {
        let access_token = register_body["access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("No access_token in response"))?
            .to_string();
        let device_id = register_body["device_id"]
            .as_str()
            .ok_or_else(|| anyhow!("No device_id in response"))?
            .to_string();
        tracing::info!("Matrix registration successful (UIAA token)");
        Ok((username, access_token, device_id, password))
    } else {
        let error = register_body["error"].as_str().unwrap_or("Unknown error");
        Err(anyhow!(
            "Registration failed: {} (status: {})",
            error,
            status
        ))
    }
}

/// Register a new Matrix user, auto-detecting which method to use:
/// - If MATRIX_REGISTRATION_TOKEN is set: uses standard UIAA token flow (Tuwunel/Docker)
/// - Otherwise: uses Synapse admin API with MATRIX_SHARED_SECRET (VPS)
pub async fn register_user(homeserver: &str) -> Result<(String, String, String, String)> {
    if let Ok(token) = std::env::var("MATRIX_REGISTRATION_TOKEN") {
        if !token.is_empty() {
            return register_user_token(homeserver, &token).await;
        }
    }

    let shared_secret = std::env::var("MATRIX_SHARED_SECRET").map_err(|_| {
        anyhow!("Neither MATRIX_REGISTRATION_TOKEN nor MATRIX_SHARED_SECRET is set")
    })?;
    register_user_synapse(homeserver, &shared_secret).await
}

pub async fn login_with_password(
    client: &MatrixClient,
    state: &Arc<AppState>,
    username: &str,
    password: &str,
    device_id: Option<&str>,
    user_id: i32,
) -> Result<()> {
    tracing::info!("🔑 Attempting to login with username and password and existing device");
    let res;
    if let Some(device_id) = device_id {
        tracing::debug!("using existing device_id");
        res = client
            .matrix_auth()
            .login_username(username, password)
            .device_id(device_id)
            .send()
            .await;
    } else {
        tracing::debug!("creating new device_id");
        res = client
            .matrix_auth()
            .login_username(username, password)
            .send()
            .await;
    }
    if let Ok(response) = res {
        tracing::info!("✅ Login successful");

        // Store the new device_id and access_token
        tracing::debug!("💾 Saving new device ID and access token to database");
        state
            .user_repository
            .set_matrix_device_id_and_access_token(
                user_id,
                &response.access_token,
                response.device_id.as_str(),
            )?;
        tracing::debug!("✅ Successfully saved credentials");
    } else {
        tracing::error!("❌ Login failed: {:?}", res.err());
        return Err(anyhow!(
            "Failed to login with username and password. User may need to be re-registered."
        ));
    }
    tracing::info!("✅ Login with password completed successfully");
    Ok(())
}

/// Sets up encryption backups for a Matrix client
async fn setup_backups(client: &MatrixClient, encrypted_key: Option<&String>) -> Result<bool> {
    tracing::info!("Checking encryption backups");
    let mut backups_enabled = client.encryption().backups().are_enabled().await;
    let mut needs_secret_storage_update = false;

    if !backups_enabled {
        tracing::debug!("Backups not enabled, attempting to restore");
        if let Some(key) = encrypted_key {
            if client.encryption().secret_storage().is_enabled().await? {
                let passphrase = decrypt(key)?;
                let secret_store = client
                    .encryption()
                    .secret_storage()
                    .open_secret_store(&passphrase)
                    .await?;
                secret_store.import_secrets().await?;

                backups_enabled = client.encryption().backups().are_enabled().await;
                if !backups_enabled {
                    needs_secret_storage_update = true;
                }
            } else {
                needs_secret_storage_update = true;
            }
        } else {
            needs_secret_storage_update = true;
        }
    }

    Ok(needs_secret_storage_update)
}

/// Sets up cross-signing for a Matrix client
async fn setup_cross_signing(
    client: &MatrixClient,
    username: &str,
    password: &str,
    encrypted_key: Option<&String>,
) -> Result<bool> {
    let mut needs_secret_storage_update = false;

    if let Some(cross_signing_status) = client.encryption().cross_signing_status().await {
        if !cross_signing_status.has_master
            || !cross_signing_status.has_self_signing
            || !cross_signing_status.has_user_signing
        {
            if let Some(encrypted_key) = encrypted_key {
                if client.encryption().secret_storage().is_enabled().await? {
                    let passphrase = decrypt(encrypted_key)?;
                    let secret_store = client
                        .encryption()
                        .secret_storage()
                        .open_secret_store(&passphrase)
                        .await?;
                    secret_store.import_secrets().await?;

                    if let Some(status) = client.encryption().cross_signing_status().await {
                        if !status.has_master
                            || !status.has_self_signing
                            || !status.has_user_signing
                        {
                            needs_secret_storage_update =
                                try_bootstrap_cross_signing(client, username, password).await?;
                        }
                    }
                } else {
                    needs_secret_storage_update = true;
                }
            } else {
                needs_secret_storage_update =
                    try_bootstrap_cross_signing(client, username, password).await?;
            }
        }
    } else {
        needs_secret_storage_update =
            try_bootstrap_cross_signing(client, username, password).await?;
    }

    Ok(needs_secret_storage_update)
}

/// Attempts to bootstrap cross-signing, handling authentication if needed
async fn try_bootstrap_cross_signing(
    client: &MatrixClient,
    username: &str,
    password: &str,
) -> Result<bool> {
    tracing::info!("Bootstrapping cross-signing keys");

    async fn clear_store(username: &str) -> Result<()> {
        let store_path = format!(
            "{}/{}",
            std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
                .map_err(|_| anyhow!("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"))?,
            username
        );
        if std::path::Path::new(&store_path).exists() {
            tokio::fs::remove_dir_all(&store_path).await?;
            tokio::fs::create_dir_all(&store_path).await?;
        }
        Ok(())
    }

    fn is_key_conflict(e: &matrix_sdk::Error) -> bool {
        let error_str = e.to_string().to_lowercase();
        error_str.contains("one time key") && error_str.contains("already exists")
            || error_str.contains("m_invalid_signature")
            || error_str.contains("already exists")
    }

    let mut retry_count = 0;
    let max_retries = 3;

    loop {
        match client.encryption().bootstrap_cross_signing(None).await {
            Ok(_) => {
                tracing::info!("Successfully bootstrapped cross-signing");
                return Ok(true);
            }
            Err(e) => {
                if let Some(response) = e.as_uiaa_response() {
                    let user_identifier =
                        matrix_sdk::ruma::api::client::uiaa::UserIdentifier::UserIdOrLocalpart(
                            username.to_string(),
                        );
                    let mut password_auth = matrix_sdk::ruma::api::client::uiaa::Password::new(
                        user_identifier,
                        password.to_string(),
                    );
                    password_auth.session = response.session.clone();

                    match client
                        .encryption()
                        .bootstrap_cross_signing(Some(
                            matrix_sdk::ruma::api::client::uiaa::AuthData::Password(password_auth),
                        ))
                        .await
                    {
                        Ok(_) => return Ok(true),
                        Err(e) => {
                            if is_key_conflict(&e) {
                                if retry_count >= max_retries {
                                    return Err(anyhow!(
                                        "Failed to bootstrap after {} retries",
                                        max_retries
                                    ));
                                }
                                let _ = clear_store(username).await;
                                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                                retry_count += 1;
                                continue;
                            }
                            return Err(anyhow!("Failed to bootstrap cross-signing: {}", e));
                        }
                    }
                } else if is_key_conflict(&e) {
                    if retry_count >= max_retries {
                        return Err(anyhow!("Failed to bootstrap after {} retries", max_retries));
                    }
                    let _ = clear_store(username).await;
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    retry_count += 1;
                    continue;
                } else {
                    return Err(anyhow!("Failed to bootstrap cross-signing: {}", e));
                }
            }
        }
    }
}

async fn get_client(user_id: i32, state: &Arc<AppState>) -> Result<MatrixClient> {
    tracing::info!("🔄 Starting get_client for user_id: {}", user_id);

    // Reuse existing client if one is already initialized for this user.
    // Creating multiple clients for the same user causes SQLite store file contention
    // (file-level lock) which blocks Tokio worker threads and freezes the server.
    {
        let clients = state.matrix_clients.lock().await;
        if let Some(existing_client) = clients.get(&user_id) {
            tracing::info!("Reusing existing Matrix client for user {}", user_id);
            return Ok((**existing_client).clone());
        }
    }

    // Get user profile from database (needed for user.id)
    let user = state.user_core.find_by_id(user_id).unwrap().unwrap();
    tracing::debug!("Found user: id={}", user.id);

    // Initialize the Matrix client
    let homeserver_url =
        std::env::var("MATRIX_HOMESERVER").map_err(|_| anyhow!("MATRIX_HOMESERVER not set"))?;

    // Get or register Matrix credentials (from PG)
    let pg_creds = state
        .user_repository
        .get_matrix_credentials(user.id)
        .map_err(|e| anyhow!("Failed to get matrix credentials from PG: {}", e))?;

    let (username, password, device_id, access_token) =
        match pg_creds.and_then(|(u, p, d, a, _)| u.zip(p).map(|(u2, p2)| (u2, p2, d, a))) {
            Some((existing_username, encrypted_password, dev_id, enc_access_token)) => {
                tracing::debug!("Existing Matrix credentials found");
                let access_token = enc_access_token.as_ref().map(|t| decrypt(t)).transpose()?;
                (
                    existing_username,
                    decrypt(&encrypted_password)?,
                    dev_id,
                    access_token,
                )
            }
            _ => {
                tracing::info!("Registering new Matrix user");
                let (username, access_token, device_id, password) =
                    register_user(&homeserver_url).await?;
                state.user_repository.set_matrix_credentials(
                    user.id,
                    &username,
                    &access_token,
                    &device_id,
                    &password,
                )?;
                (username, password, Some(device_id), Some(access_token))
            }
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
    let url = Url::parse(&homeserver_url).map_err(|e| anyhow!("Invalid homeserver URL: {}", e))?;
    let domain = url
        .host_str()
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

    // Attempt to restore session
    let mut session_restored = false;
    if let Some(stored_session) = client.matrix_auth().session() {
        tracing::debug!("🔄 Found session in store, attempting to restore");
        if let Err(e) = client
            .matrix_auth()
            .restore_session(stored_session.clone(), RoomLoadSettings::default())
            .await
        {
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
                    response.device_id.expect("default").as_str(),
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
                    device_id: matrix_sdk::ruma::OwnedDeviceId::from(device_id.clone().unwrap()),
                },
                tokens: matrix_sdk::authentication::SessionTokens {
                    access_token: access_token.clone(),
                    refresh_token: None,
                },
            };
            if client
                .matrix_auth()
                .restore_session(session.clone(), RoomLoadSettings::default())
                .await
                .is_ok()
            {
                tracing::debug!("✅ Token-based session restored");
                // Verify session
                if let Ok(response) = client.whoami().await {
                    state.user_repository.set_matrix_credentials(
                        user.id,
                        &username,
                        access_token.as_str(),
                        response.device_id.expect("default").as_str(),
                        &password,
                    )?;
                    session_restored = true;
                }
            }
        }

        // Fallback to password login if token-based login fails
        if !session_restored {
            tracing::debug!("🔄 Attempting password-based login");
            login_with_password(
                &client,
                state,
                &username,
                &password,
                device_id.as_deref(),
                user.id,
            )
            .await?;
        }
    }
    tracing::info!("✅ Authentication complete - client is logged already in");
    // here we should have client store, our db and server synced with the same device id and access token

    // Handle encryption keys if user has E2EE enabled
    if user.matrix_e2ee_enabled {
        tracing::info!(
            "Setting up encryption keys and secret storage for user {}",
            user_id
        );
        let mut redo_secret_storage = false;

        // Get the encrypted recovery key from PG
        let encrypted_recovery_key = state
            .user_repository
            .get_matrix_credentials(user.id)
            .map_err(|e| anyhow!("Failed to get matrix credentials from PG: {}", e))?
            .and_then(|(_, _, _, _, rk)| rk);

        // Handle cross-signing setup
        match setup_cross_signing(
            &client,
            &username,
            &password,
            encrypted_recovery_key.as_ref(),
        )
        .await
        {
            Ok(should_update_storage) => {
                if should_update_storage {
                    redo_secret_storage = true;
                }
            }
            Err(e) => return Err(e),
        }

        // Handle backups setup
        match setup_backups(&client, encrypted_recovery_key.as_ref()).await {
            Ok(should_update_storage) => {
                if should_update_storage {
                    redo_secret_storage = true;
                }
            }
            Err(e) => return Err(e),
        }

        if redo_secret_storage {
            if let Some(ref encrypted_key) = encrypted_recovery_key {
                let passphrase = decrypt(encrypted_key)?;
                client
                    .encryption()
                    .recovery()
                    .enable()
                    .with_passphrase(&passphrase)
                    .await?;
            } else {
                let recovery = client.encryption().recovery();
                let enable = recovery.enable().wait_for_backups_to_upload();
                let recovery_key = enable.await?;
                state
                    .user_repository
                    .set_matrix_secret_storage_recovery_key(user.id, &recovery_key)?;
            }
        }

        // Initial sync for room keys
        client
            .sync_once(matrix_sdk::config::SyncSettings::new())
            .await?;
        sleep(Duration::from_secs(2)).await;
    } else {
        tracing::debug!("Skipping E2EE setup for user {} (not enabled)", user_id);
    }

    tracing::info!("✅ Matrix client fully initialized for user {}", user_id);

    // Store in the shared client map so future calls reuse this client
    // instead of creating a new one (which causes SQLite store contention)
    {
        let mut clients = state.matrix_clients.lock().await;
        clients.insert(user_id, Arc::new(client.clone()));
    }

    Ok(client)
}

/// Get a cached Matrix client from AppState, with fallback to creating a new client
/// Note: The fallback client is not stored in the cache - that's managed by the scheduler
pub async fn get_cached_client(user_id: i32, state: &Arc<AppState>) -> Result<Arc<MatrixClient>> {
    // Get the matrix clients map from AppState
    let matrix_clients = state.matrix_clients.lock().await;

    // Try to get the client for this user
    if let Some(client) = matrix_clients.get(&user_id) {
        tracing::debug!("Found cached Matrix client for user {}", user_id);
        Ok(client.clone())
    } else {
        tracing::debug!(
            "No cached Matrix client found for user {}, creating temporary client",
            user_id
        );
        // Drop the lock before the potentially long-running get_client operation
        drop(matrix_clients);

        // Create a new client as fallback
        match get_client(user_id, state).await {
            Ok(client) => {
                tracing::debug!(
                    "Successfully created temporary Matrix client for user {}",
                    user_id
                );
                Ok(Arc::new(client))
            }
            Err(e) => {
                tracing::error!(
                    "Failed to create temporary Matrix client for user {}: {}",
                    user_id,
                    e
                );
                Err(anyhow!("Failed to create Matrix client: {}", e))
            }
        }
    }
}
