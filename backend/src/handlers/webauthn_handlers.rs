use axum::{
    extract::State,
    http::StatusCode,
    Json,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use webauthn_rs::prelude::*;

use crate::AppState;
use crate::handlers::auth_middleware::AuthUser;
use crate::utils::webauthn_config::get_webauthn;
use crate::handlers::auth_handlers::generate_tokens_and_response;

// ============ DTOs ============

#[derive(Deserialize)]
pub struct RegisterStartRequest {
    pub device_name: String,
}

#[derive(Serialize)]
pub struct RegisterStartResponse {
    pub options: CreationChallengeResponse,
}

#[derive(Deserialize)]
pub struct RegisterFinishRequest {
    pub device_name: String,
    pub response: RegisterPublicKeyCredential,
}

#[derive(Serialize, Clone)]
pub struct PasskeyInfo {
    pub credential_id: String,
    pub device_name: String,
    pub created_at: i32,
    pub last_used_at: Option<i32>,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub enabled: bool,
    pub passkey_count: i64,
}

#[derive(Deserialize)]
pub struct AuthStartRequest {
    pub context: Option<String>,
}

#[derive(Serialize)]
pub struct AuthStartResponse {
    pub options: RequestChallengeResponse,
}

#[derive(Deserialize)]
pub struct AuthFinishRequest {
    pub response: PublicKeyCredential,
    pub context: Option<String>,
}

#[derive(Deserialize)]
pub struct DeletePasskeyRequest {
    pub credential_id: String,
}

#[derive(Deserialize)]
pub struct RenamePasskeyRequest {
    pub credential_id: String,
    pub new_name: String,
}

#[derive(Deserialize)]
pub struct VerifyLoginRequest {
    pub login_token: String,
    pub response: PublicKeyCredential,
}

// ============ Protected Handlers ============

/// GET /api/webauthn/status - Check if user has passkeys
pub async fn get_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<StatusResponse>, (StatusCode, Json<serde_json::Value>)> {
    let count = state.webauthn_repository
        .get_passkey_count(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get passkey count: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?;

    Ok(Json(StatusResponse {
        enabled: count > 0,
        passkey_count: count,
    }))
}

/// GET /api/webauthn/passkeys - List all passkeys for user
pub async fn list_passkeys(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<PasskeyInfo>>, (StatusCode, Json<serde_json::Value>)> {
    let credentials = state.webauthn_repository
        .get_credentials_by_user(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get credentials: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?;

    let passkeys: Vec<PasskeyInfo> = credentials
        .into_iter()
        .map(|c| PasskeyInfo {
            credential_id: c.credential_id,
            device_name: c.device_name,
            created_at: c.created_at,
            last_used_at: c.last_used_at,
        })
        .collect();

    Ok(Json(passkeys))
}

/// POST /api/webauthn/register/start - Start passkey registration
pub async fn register_start(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<RegisterStartRequest>,
) -> Result<Json<RegisterStartResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Check if TOTP is enabled - required before adding passkeys
    let totp_enabled = state.totp_repository
        .is_totp_enabled(auth_user.user_id)
        .unwrap_or(false);

    if !totp_enabled {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "You must enable authenticator app (TOTP) before adding passkeys. This ensures you have a fallback authentication method."
            }))
        ));
    }

    // Get user email for display
    let user = state.user_core
        .find_by_id(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get user: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?
        .ok_or_else(|| {
            (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "User not found"})))
        })?;

    // Get existing credentials to exclude from registration
    let existing_creds = state.webauthn_repository
        .get_credentials_by_user(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get existing credentials: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?;

    // Convert existing credentials to exclude list
    let exclude_credentials: Vec<CredentialID> = existing_creds
        .iter()
        .filter_map(|c| {
            base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &c.credential_id)
                .ok()
                .map(|bytes| CredentialID::from(bytes))
        })
        .collect();

    let webauthn = get_webauthn();

    // Create user unique ID from user_id (deterministic)
    // Pad the user_id to 16 bytes for UUID
    let mut uuid_bytes = [0u8; 16];
    let user_id_bytes = auth_user.user_id.to_le_bytes();
    uuid_bytes[..4].copy_from_slice(&user_id_bytes);
    let user_unique_id = Uuid::from_bytes(uuid_bytes);

    // Start registration
    let (ccr, reg_state) = webauthn
        .start_passkey_registration(
            user_unique_id,
            &user.email,
            &user.email,
            Some(exclude_credentials),
        )
        .map_err(|e| {
            tracing::error!("Failed to start registration: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "WebAuthn error"})))
        })?;

    // Store registration state (serialized) in challenge table
    let state_json = serde_json::to_string(&reg_state)
        .map_err(|e| {
            tracing::error!("Failed to serialize registration state: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Serialization error"})))
        })?;

    // Store challenge with device name as context
    state.webauthn_repository
        .create_challenge(
            auth_user.user_id,
            &state_json,
            "registration",
            Some(req.device_name),
            300, // 5 minute TTL
        )
        .map_err(|e| {
            tracing::error!("Failed to store challenge: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?;

    Ok(Json(RegisterStartResponse { options: ccr }))
}

/// POST /api/webauthn/register/finish - Complete passkey registration
pub async fn register_finish(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<RegisterFinishRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Get the stored registration state
    let challenge = state.webauthn_repository
        .get_valid_challenge(auth_user.user_id, "registration")
        .map_err(|e| {
            tracing::error!("Failed to get challenge: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "No pending registration"})))
        })?;

    // Deserialize registration state
    let reg_state: PasskeyRegistration = serde_json::from_str(&challenge.challenge)
        .map_err(|e| {
            tracing::error!("Failed to deserialize registration state: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "State error"})))
        })?;

    let webauthn = get_webauthn();

    // Finish registration
    let passkey = webauthn
        .finish_passkey_registration(&req.response, &reg_state)
        .map_err(|e| {
            tracing::error!("Failed to finish registration: {:?}", e);
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("Registration failed: {:?}", e)})))
        })?;

    // Extract credential data
    let credential_id = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        passkey.cred_id().as_ref()
    );

    // Serialize the passkey for storage
    let passkey_json = serde_json::to_string(&passkey)
        .map_err(|e| {
            tracing::error!("Failed to serialize passkey: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Serialization error"})))
        })?;

    // Get device name from challenge context or request
    let device_name = challenge.context.unwrap_or(req.device_name);

    // Store the credential
    state.webauthn_repository
        .create_credential(
            auth_user.user_id,
            &credential_id,
            &passkey_json,
            &device_name,
            0, // Initial counter
            None, // Transports - could extract from passkey if needed
            None, // AAGUID
        )
        .map_err(|e| {
            tracing::error!("Failed to store credential: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?;

    // Delete the challenge
    let _ = state.webauthn_repository.delete_challenges_by_type(auth_user.user_id, "registration");

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Passkey registered successfully",
        "device_name": device_name
    })))
}

/// POST /api/webauthn/authenticate/start - Start authentication (for Tesla unlock)
pub async fn authenticate_start(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<AuthStartRequest>,
) -> Result<Json<AuthStartResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get user's credentials
    let credentials = state.webauthn_repository
        .get_credentials_by_user(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get credentials: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?;

    if credentials.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "No passkeys registered"}))));
    }

    // Deserialize credentials back to Passkey objects
    let passkeys: Vec<Passkey> = credentials
        .iter()
        .filter_map(|c| {
            let decrypted = state.webauthn_repository.get_decrypted_public_key(c).ok()?;
            serde_json::from_str(&decrypted).ok()
        })
        .collect();

    if passkeys.is_empty() {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to load credentials"}))));
    }

    let webauthn = get_webauthn();

    // Start authentication
    let (rcr, auth_state) = webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|e| {
            tracing::error!("Failed to start authentication: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "WebAuthn error"})))
        })?;

    // Store authentication state
    let state_json = serde_json::to_string(&auth_state)
        .map_err(|e| {
            tracing::error!("Failed to serialize auth state: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Serialization error"})))
        })?;

    state.webauthn_repository
        .create_challenge(
            auth_user.user_id,
            &state_json,
            "authentication",
            req.context,
            300, // 5 minute TTL
        )
        .map_err(|e| {
            tracing::error!("Failed to store challenge: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?;

    Ok(Json(AuthStartResponse { options: rcr }))
}

/// POST /api/webauthn/authenticate/finish - Complete authentication (for Tesla unlock)
pub async fn authenticate_finish(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<AuthFinishRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Get the stored authentication state
    let challenge = state.webauthn_repository
        .get_valid_challenge(auth_user.user_id, "authentication")
        .map_err(|e| {
            tracing::error!("Failed to get challenge: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "No pending authentication"})))
        })?;

    // Deserialize authentication state
    let auth_state: PasskeyAuthentication = serde_json::from_str(&challenge.challenge)
        .map_err(|e| {
            tracing::error!("Failed to deserialize auth state: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "State error"})))
        })?;

    let webauthn = get_webauthn();

    // Finish authentication
    let auth_result = webauthn
        .finish_passkey_authentication(&req.response, &auth_state)
        .map_err(|e| {
            tracing::error!("Failed to finish authentication: {:?}", e);
            (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Authentication failed"})))
        })?;

    // Update the credential counter
    let credential_id = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        auth_result.cred_id().as_ref()
    );

    let _ = state.webauthn_repository.update_counter(&credential_id, auth_result.counter() as i32);

    // Delete the challenge
    let _ = state.webauthn_repository.delete_challenges_by_type(auth_user.user_id, "authentication");

    Ok(Json(serde_json::json!({
        "success": true,
        "context": challenge.context
    })))
}

/// DELETE /api/webauthn/passkey - Delete a passkey
pub async fn delete_passkey(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<DeletePasskeyRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = state.webauthn_repository
        .delete_credential(auth_user.user_id, &req.credential_id)
        .map_err(|e| {
            tracing::error!("Failed to delete credential: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?;

    if deleted {
        Ok(Json(serde_json::json!({"success": true, "message": "Passkey deleted"})))
    } else {
        Err((StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Passkey not found"}))))
    }
}

/// PATCH /api/webauthn/passkey/rename - Rename a passkey
pub async fn rename_passkey(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<RenamePasskeyRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let updated = state.webauthn_repository
        .rename_credential(auth_user.user_id, &req.credential_id, &req.new_name)
        .map_err(|e| {
            tracing::error!("Failed to rename credential: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?;

    if updated {
        Ok(Json(serde_json::json!({"success": true, "new_name": req.new_name})))
    } else {
        Err((StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Passkey not found"}))))
    }
}

// ============ Public Handlers (for login flow) ============

/// POST /api/webauthn/verify-login - Verify WebAuthn during login
pub async fn verify_login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VerifyLoginRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Get pending login from the shared map
    let pending = state.pending_totp_logins.get(&req.login_token)
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid or expired login token"})))
        })?;

    let (user_id, expiry) = *pending;
    drop(pending);

    // Check expiry
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    if current_time > expiry {
        state.pending_totp_logins.remove(&req.login_token);
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Login token expired"}))));
    }

    // Get user's credentials
    let credentials = state.webauthn_repository
        .get_credentials_by_user(user_id)
        .map_err(|e| {
            tracing::error!("Failed to get credentials: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?;

    if credentials.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "No passkeys registered"}))));
    }

    // Deserialize credentials back to Passkey objects
    let passkeys: Vec<Passkey> = credentials
        .iter()
        .filter_map(|c| {
            let decrypted = state.webauthn_repository.get_decrypted_public_key(c).ok()?;
            serde_json::from_str(&decrypted).ok()
        })
        .collect();

    if passkeys.is_empty() {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to load credentials"}))));
    }

    // Get the stored authentication state from pending_webauthn_logins
    let challenge = state.webauthn_repository
        .get_valid_challenge(user_id, "login")
        .map_err(|e| {
            tracing::error!("Failed to get challenge: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "No pending authentication. Please start login again."})))
        })?;

    // Deserialize authentication state
    let auth_state: PasskeyAuthentication = serde_json::from_str(&challenge.challenge)
        .map_err(|e| {
            tracing::error!("Failed to deserialize auth state: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "State error"})))
        })?;

    let webauthn = get_webauthn();

    // Finish authentication
    let auth_result = webauthn
        .finish_passkey_authentication(&req.response, &auth_state)
        .map_err(|e| {
            tracing::error!("Failed to finish authentication: {:?}", e);
            (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Authentication failed"})))
        })?;

    // Update the credential counter
    let credential_id = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        auth_result.cred_id().as_ref()
    );
    let _ = state.webauthn_repository.update_counter(&credential_id, auth_result.counter() as i32);

    // Cleanup
    let _ = state.webauthn_repository.delete_challenges_by_type(user_id, "login");
    state.pending_totp_logins.remove(&req.login_token);

    // Generate tokens and return response
    generate_tokens_and_response(user_id)
        .map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to generate tokens"})))
        })
}

/// POST /api/webauthn/login/start - Start WebAuthn auth for login (public endpoint)
/// Called after password verification when webauthn_enabled is true
#[derive(Deserialize)]
pub struct LoginStartRequest {
    pub login_token: String,
}

pub async fn login_start(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginStartRequest>,
) -> Result<Json<AuthStartResponse>, (StatusCode, Json<serde_json::Value>)> {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Get pending login from the shared map
    let pending = state.pending_totp_logins.get(&req.login_token)
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Invalid or expired login token"})))
        })?;

    let (user_id, expiry) = *pending;
    drop(pending);

    // Check expiry
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    if current_time > expiry {
        state.pending_totp_logins.remove(&req.login_token);
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Login token expired"}))));
    }

    // Get user's credentials
    let credentials = state.webauthn_repository
        .get_credentials_by_user(user_id)
        .map_err(|e| {
            tracing::error!("Failed to get credentials: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?;

    if credentials.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "No passkeys registered"}))));
    }

    // Deserialize credentials back to Passkey objects
    let passkeys: Vec<Passkey> = credentials
        .iter()
        .filter_map(|c| {
            let decrypted = state.webauthn_repository.get_decrypted_public_key(c).ok()?;
            serde_json::from_str(&decrypted).ok()
        })
        .collect();

    if passkeys.is_empty() {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to load credentials"}))));
    }

    let webauthn = get_webauthn();

    // Start authentication
    let (rcr, auth_state) = webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|e| {
            tracing::error!("Failed to start authentication: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "WebAuthn error"})))
        })?;

    // Store authentication state with "login" context
    let state_json = serde_json::to_string(&auth_state)
        .map_err(|e| {
            tracing::error!("Failed to serialize auth state: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Serialization error"})))
        })?;

    state.webauthn_repository
        .create_challenge(
            user_id,
            &state_json,
            "login",
            Some("login".to_string()),
            300, // 5 minute TTL
        )
        .map_err(|e| {
            tracing::error!("Failed to store challenge: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Database error"})))
        })?;

    Ok(Json(AuthStartResponse { options: rcr }))
}
