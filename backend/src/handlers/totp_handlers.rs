use std::sync::Arc;
use crate::handlers::auth_middleware::AuthUser;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use totp_rs::{Algorithm, TOTP, Secret};

use crate::AppState;

// ============ DTOs ============

#[derive(Serialize)]
pub struct TotpSetupResponse {
    pub qr_code_data_url: String,  // Base64 data URL for QR code image
    pub secret: String,             // Plain text secret for manual entry
}

#[derive(Deserialize)]
pub struct TotpVerifySetupRequest {
    pub code: String,
}

#[derive(Serialize)]
pub struct TotpVerifySetupResponse {
    pub success: bool,
    pub backup_codes: Vec<String>,
}

#[derive(Serialize)]
pub struct TotpStatusResponse {
    pub enabled: bool,
    pub remaining_backup_codes: i64,
}

#[derive(Deserialize)]
pub struct TotpDisableRequest {
    pub code: String,
}

#[derive(Deserialize)]
pub struct TotpLoginVerifyRequest {
    pub totp_token: String,
    pub code: String,
    pub is_backup_code: bool,
}

#[derive(Deserialize)]
pub struct RegenerateBackupCodesRequest {
    pub code: String,
}

#[derive(Serialize)]
pub struct RegenerateBackupCodesResponse {
    pub backup_codes: Vec<String>,
}

// ============ Handlers ============

/// Start TOTP setup - generate secret and return QR code
pub async fn setup_start(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<TotpSetupResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get user email for the TOTP label
    let user = state.user_core.find_by_id(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
        })?
        .ok_or_else(|| {
            (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"})))
        })?;

    // Generate a new TOTP secret
    let secret = Secret::generate_secret();
    let secret_base32 = secret.to_encoded().to_string();

    // Create TOTP instance
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,      // digits
        1,      // skew (allow 1 step before/after)
        30,     // step in seconds
        secret.to_bytes().unwrap(),
        Some("Lightfriend".to_string()),
        user.email.clone(),
    ).map_err(|e| {
        tracing::error!("TOTP creation error: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to create TOTP"})))
    })?;

    // Generate QR code as data URL
    let qr_code_data_url = totp.get_qr_base64()
        .map_err(|e| {
            tracing::error!("QR code generation error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to generate QR code"})))
        })?;

    // Store the secret (encrypted, not enabled yet)
    state.totp_repository.create_secret(auth_user.user_id, &secret_base32)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to store secret"})))
        })?;

    Ok(Json(TotpSetupResponse {
        qr_code_data_url: format!("data:image/png;base64,{}", qr_code_data_url),
        secret: secret_base32,
    }))
}

/// Verify TOTP code and enable 2FA
pub async fn setup_verify(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<TotpVerifySetupRequest>,
) -> Result<Json<TotpVerifySetupResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get the stored secret
    let secret_opt = state.totp_repository.get_secret(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
        })?;

    let secret_base32 = secret_opt.ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": "No TOTP setup in progress. Please start setup first."})))
    })?;

    // Get user email for TOTP verification
    let user = state.user_core.find_by_id(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
        })?
        .ok_or_else(|| {
            (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"})))
        })?;

    // Create TOTP instance and verify
    let secret = Secret::Encoded(secret_base32.clone());
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().unwrap(),
        Some("Lightfriend".to_string()),
        user.email,
    ).map_err(|e| {
        tracing::error!("TOTP creation error: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to verify TOTP"})))
    })?;

    // Verify the code
    let is_valid = totp.check_current(&req.code).unwrap_or(false);

    if !is_valid {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid verification code"}))));
    }

    // Enable TOTP
    state.totp_repository.enable_totp(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to enable TOTP"})))
        })?;

    // Generate backup codes
    let backup_codes = state.totp_repository.create_backup_codes(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to create backup codes"})))
        })?;

    Ok(Json(TotpVerifySetupResponse {
        success: true,
        backup_codes,
    }))
}

/// Get TOTP status
pub async fn get_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<TotpStatusResponse>, (StatusCode, Json<serde_json::Value>)> {
    let enabled = state.totp_repository.is_totp_enabled(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
        })?;

    let remaining_backup_codes = if enabled {
        state.totp_repository.get_remaining_backup_codes(auth_user.user_id)
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
            })?
    } else {
        0
    };

    Ok(Json(TotpStatusResponse {
        enabled,
        remaining_backup_codes,
    }))
}

/// Disable TOTP
pub async fn disable(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<TotpDisableRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check if user has passkeys - cannot disable TOTP if passkeys are registered
    let passkey_count = state.webauthn_repository
        .get_passkey_count(auth_user.user_id)
        .unwrap_or(0);

    if passkey_count > 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Cannot disable authenticator app while passkeys are registered. Please remove all passkeys first."
            }))
        ));
    }

    // Get the stored secret
    let secret_opt = state.totp_repository.get_secret(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
        })?;

    let secret_base32 = secret_opt.ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(json!({"error": "TOTP not enabled"})))
    })?;

    // Get user email
    let user = state.user_core.find_by_id(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
        })?
        .ok_or_else(|| {
            (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"})))
        })?;

    // Verify the code before disabling
    let secret = Secret::Encoded(secret_base32);
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().unwrap(),
        Some("Lightfriend".to_string()),
        user.email,
    ).map_err(|e| {
        tracing::error!("TOTP creation error: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to verify TOTP"})))
    })?;

    let is_valid = totp.check_current(&req.code).unwrap_or(false);

    if !is_valid {
        // Also try backup code
        let backup_valid = state.totp_repository.verify_backup_code(auth_user.user_id, &req.code)
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
            })?;

        if !backup_valid {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid code"}))));
        }
    }

    // Disable TOTP
    state.totp_repository.disable_totp(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to disable TOTP"})))
        })?;

    Ok(Json(json!({"success": true, "message": "2FA has been disabled"})))
}

/// Regenerate backup codes
pub async fn regenerate_backup_codes(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<RegenerateBackupCodesRequest>,
) -> Result<Json<RegenerateBackupCodesResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Check if TOTP is enabled
    let enabled = state.totp_repository.is_totp_enabled(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
        })?;

    if !enabled {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "TOTP not enabled"}))));
    }

    // Get the stored secret
    let secret_base32 = state.totp_repository.get_secret(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
        })?
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, Json(json!({"error": "TOTP not configured"})))
        })?;

    // Get user email
    let user = state.user_core.find_by_id(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
        })?
        .ok_or_else(|| {
            (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"})))
        })?;

    // Verify the code
    let secret = Secret::Encoded(secret_base32);
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().unwrap(),
        Some("Lightfriend".to_string()),
        user.email,
    ).map_err(|e| {
        tracing::error!("TOTP creation error: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to verify TOTP"})))
    })?;

    let is_valid = totp.check_current(&req.code).unwrap_or(false);

    if !is_valid {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid verification code"}))));
    }

    // Generate new backup codes
    let backup_codes = state.totp_repository.create_backup_codes(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to create backup codes"})))
        })?;

    Ok(Json(RegenerateBackupCodesResponse { backup_codes }))
}

/// Verify TOTP code during login (public endpoint, uses totp_token)
pub async fn verify_login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TotpLoginVerifyRequest>,
) -> Result<axum::response::Response, (StatusCode, Json<serde_json::Value>)> {
    use std::time::{SystemTime, UNIX_EPOCH};
    use crate::handlers::auth_handlers::generate_tokens_and_response;

    // Validate the totp_token and get user_id
    let pending_login = state.pending_totp_logins.get(&req.totp_token)
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid or expired TOTP token"})))
        })?;

    let (user_id, expiry) = *pending_login;
    drop(pending_login); // Release the lock

    // Check if token has expired
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    if current_time > expiry {
        state.pending_totp_logins.remove(&req.totp_token);
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "TOTP token has expired"}))));
    }

    // If using backup code
    if req.is_backup_code {
        let backup_valid = state.totp_repository.verify_backup_code(user_id, &req.code)
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
            })?;

        if !backup_valid {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid backup code"}))));
        }
    } else {
        // Verify TOTP code
        let secret_opt = state.totp_repository.get_secret(user_id)
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
            })?;

        let secret_base32 = secret_opt.ok_or_else(|| {
            (StatusCode::BAD_REQUEST, Json(json!({"error": "TOTP not configured"})))
        })?;

        // Get user email
        let user = state.user_core.find_by_id(user_id)
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
            })?
            .ok_or_else(|| {
                (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"})))
            })?;

        let secret = Secret::Encoded(secret_base32);
        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret.to_bytes().unwrap(),
            Some("Lightfriend".to_string()),
            user.email,
        ).map_err(|e| {
            tracing::error!("TOTP creation error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to verify TOTP"})))
        })?;

        let is_valid = totp.check_current(&req.code).unwrap_or(false);

        if !is_valid {
            return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Invalid verification code"}))));
        }
    }

    // Remove the pending login token
    state.pending_totp_logins.remove(&req.totp_token);

    // Generate tokens and return response
    generate_tokens_and_response(user_id)
}
