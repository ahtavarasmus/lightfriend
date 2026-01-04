use reqwest;
use std::sync::Arc;
use std::convert::Infallible;
use crate::handlers::auth_middleware::AuthUser;
use axum::{
    extract::{Query, State},
    response::{Json, Redirect, sse::{Event, KeepAlive, Sse}},
    http::StatusCode,
};
use futures::stream::Stream;
use tower_sessions::{session_store::SessionStore, session::{Id, Record}};
use oauth2::{
    PkceCodeVerifier,
    CsrfToken,
    PkceCodeChallenge,
    Scope,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use time::OffsetDateTime;
use tracing::{info, error, warn};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};

use crate::{
    AppState,
    models::user_models::NewTesla,
    utils::encryption::{encrypt, decrypt},
};

/// JWT claims structure for Tesla access token
#[derive(Debug, Deserialize)]
struct TeslaJwtClaims {
    /// Scopes are in the "scp" claim as an array of strings
    #[serde(default)]
    scp: Vec<String>,
    /// Alternative: some tokens use "scope" as space-separated string
    #[serde(default)]
    scope: Option<String>,
}

/// Extract scopes from a Tesla JWT access token without verifying signature
/// Tesla tokens are JWTs that contain the granted scopes in the "scp" claim
fn extract_scopes_from_jwt(access_token: &str) -> Option<String> {
    // Check if it's a JWT (has 3 parts)
    let token_parts: Vec<&str> = access_token.split('.').collect();
    if token_parts.len() != 3 {
        // Not a JWT - Tesla sometimes uses opaque tokens
        return None;
    }

    // Create a validation that doesn't verify the signature (we just want to read claims)
    let mut validation = Validation::new(Algorithm::RS256);
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    validation.set_audience::<&str>(&[]); // Don't validate audience

    // Decode without verification (we trust Tesla issued the token)
    match decode::<TeslaJwtClaims>(
        access_token,
        &DecodingKey::from_secret(&[]), // Not used when signature validation is disabled
        &validation,
    ) {
        Ok(token_data) => {
            let claims = token_data.claims;

            // First try the scp array (standard Tesla format)
            if !claims.scp.is_empty() {
                let scopes = claims.scp.join(" ");
                info!("Extracted scopes from JWT scp claim: {}", scopes);
                return Some(scopes);
            }

            // Fallback to scope string
            if let Some(scope) = claims.scope {
                if !scope.is_empty() {
                    info!("Extracted scopes from JWT scope claim: {}", scope);
                    return Some(scope);
                }
            }

            // JWT decoded but no scopes found
            None
        }
        Err(_) => {
            // Failed to decode JWT - not an error, Tesla may use different token format
            None
        }
    }
}

// Helper to create error redirect for OAuth callback failures
fn tesla_error_redirect(error_msg: &str) -> Redirect {
    let frontend_url = std::env::var("FRONTEND_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    let encoded_error = urlencoding::encode(error_msg);
    Redirect::to(&format!("{}?tesla=error&message={}", frontend_url, encoded_error))
}

#[derive(Debug, Deserialize)]
pub struct TeslaCallbackParams {
    code: String,
    state: String,
}

#[derive(Serialize)]
pub struct TeslaStatusResponse {
    has_tesla: bool,
}

#[derive(Debug, Deserialize)]
pub struct TeslaLoginParams {
    /// Comma-separated scopes to request (e.g., "vehicle_device_data,vehicle_cmds")
    pub scopes: Option<String>,
}

// Tesla OAuth login endpoint - requires Tier 2
pub async fn tesla_login(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(params): Query<TeslaLoginParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Tesla OAuth login initiated for user {}", auth_user.user_id);

    // Check if user has Tier 2 subscription
    let user = state.user_core.find_by_id(auth_user.user_id)
        .map_err(|e| {
            error!("Failed to get user: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to get user information"}))
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"}))
            )
        })?;

    if user.sub_tier != Some("tier 2".to_string()) && user.sub_tier != Some("tier 3".to_string()) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Tesla integration requires a paid subscription"}))
        ));
    }

    // Generate session key and CSRF token
    let session_key = Uuid::new_v4().to_string();
    let csrf_token = CsrfToken::new_random();
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Create session record
    let mut record = Record {
        id: Id(Uuid::new_v4().as_u128() as i128),
        data: Default::default(),
        expiry_date: OffsetDateTime::now_utc() + time::Duration::hours(1),
    };

    record.data.insert("session_key".to_string(), json!(session_key.clone()));
    record.data.insert("pkce_verifier".to_string(), json!(pkce_verifier.secret().to_string()));
    record.data.insert("csrf_token".to_string(), json!(csrf_token.secret().to_string()));
    record.data.insert("user_id".to_string(), json!(auth_user.user_id));

    // Build scopes list early so we can store it in session
    let valid_scopes = ["vehicle_device_data", "vehicle_cmds", "vehicle_charging_cmds"];
    let requested_scopes: Vec<String> = if let Some(scopes_str) = &params.scopes {
        scopes_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| valid_scopes.contains(&s.as_str()))
            .collect()
    } else {
        // Default to all scopes if none specified
        valid_scopes.iter().map(|s| s.to_string()).collect()
    };

    // Store requested scopes in session so callback can use them
    record.data.insert("requested_scopes".to_string(), json!(requested_scopes.join(" ")));

    // Store session
    if let Err(e) = state.session_store.create(&mut record).await {
        error!("Failed to store session record: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to store session record: {}", e)}))
        ));
    }

    let state_token = format!("{}:{}", record.id.0, csrf_token.secret());

    // Build authorization URL with user-selected scopes
    // Always include openid and offline_access for token refresh
    let mut auth_url_builder = state
        .tesla_oauth_client
        .authorize_url(|| CsrfToken::new(state_token.clone()))
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("offline_access".to_string()))
        .add_extra_param("prompt", "consent")  // Force consent screen to show every time
        .set_pkce_challenge(pkce_challenge);

    // Add user-selected scopes (already computed above and stored in session)
    for scope in &requested_scopes {
        auth_url_builder = auth_url_builder.add_scope(Scope::new(scope.clone()));
    }

    let (auth_url, _) = auth_url_builder.url();

    info!("Tesla OAuth URL generated with state: {} and scopes: {:?}", state_token, requested_scopes);

    Ok(Json(json!({
        "auth_url": auth_url.to_string(),
        "message": "Tesla OAuth flow initiated successfully"
    })))
}

// Tesla OAuth callback endpoint
// Returns Redirect for both success and error cases so user gets proper feedback
pub async fn tesla_callback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TeslaCallbackParams>,
) -> Redirect {
    info!("Tesla OAuth callback received with state: {}", params.state);

    // Parse state token
    let state_parts: Vec<&str> = params.state.split(':').collect();
    if state_parts.len() != 2 {
        error!("Invalid state format: {}", params.state);
        return tesla_error_redirect("Invalid OAuth state format");
    }

    let session_id_str = state_parts[0];
    let state_csrf = state_parts[1];

    // Parse session ID
    let session_id = match session_id_str.parse::<i128>() {
        Ok(id) => id,
        Err(e) => {
            error!("Invalid session ID format: {}", e);
            return tesla_error_redirect("Invalid session ID");
        }
    };

    // Retrieve session
    let session_record = match state.session_store.load(&Id(session_id)).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            error!("Session not found for ID: {}", session_id);
            return tesla_error_redirect("Session expired. Please try connecting again.");
        }
        Err(e) => {
            error!("Failed to load session: {}", e);
            return tesla_error_redirect("Failed to load session");
        }
    };

    // Validate CSRF token
    let stored_csrf = match session_record.data.get("csrf_token").and_then(|v| v.as_str()) {
        Some(csrf) => csrf,
        None => {
            error!("CSRF token not found in session");
            return tesla_error_redirect("Security token missing");
        }
    };

    if state_csrf != stored_csrf {
        error!("CSRF token mismatch");
        return tesla_error_redirect("Security token mismatch");
    }

    // Get user ID and PKCE verifier from session
    let user_id = match session_record.data.get("user_id").and_then(|v| v.as_i64()) {
        Some(id) => id as i32,
        None => {
            error!("User ID not found in session");
            return tesla_error_redirect("User session invalid");
        }
    };

    let pkce_verifier_secret = match session_record.data.get("pkce_verifier").and_then(|v| v.as_str()) {
        Some(secret) => secret.to_string(),
        None => {
            error!("PKCE verifier not found in session");
            return tesla_error_redirect("Security verification failed");
        }
    };

    // Get requested scopes from session (these are the scopes user selected in the UI)
    let requested_scopes = session_record.data.get("requested_scopes")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "vehicle_device_data vehicle_cmds vehicle_charging_cmds".to_string());
    info!("Retrieved requested scopes from session for user {}: {}", user_id, requested_scopes);

    // Exchange authorization code for tokens
    // Note: Tesla uses a different domain for token exchange
    let pkce_verifier = PkceCodeVerifier::new(pkce_verifier_secret);

    // Build custom HTTP client for token exchange
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    // Create a custom token exchange request since Tesla uses a different domain
    // We need to use fleet-auth.prd.vn.cloud.tesla.com for token exchange
    let token_url = "https://fleet-auth.prd.vn.cloud.tesla.com/oauth2/v3/token";
    let client_id = std::env::var("TESLA_CLIENT_ID")
        .unwrap_or_else(|_| "default-tesla-client-id-for-testing".to_string());
    let client_secret = std::env::var("TESLA_CLIENT_SECRET")
        .unwrap_or_else(|_| "default-tesla-secret-for-testing".to_string());
    let server_url = std::env::var("SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    let tesla_redirect_url = std::env::var("TESLA_REDIRECT_URL")
        .unwrap_or_else(|_| server_url.clone());
    let redirect_uri = format!("{}/api/auth/tesla/callback", tesla_redirect_url);

    // Get the audience URL from env or default to EU region
    let audience_url = std::env::var("TESLA_API_BASE")
        .unwrap_or_else(|_| "https://fleet-api.prd.eu.vn.cloud.tesla.com".to_string());

    // Manual token exchange request for Tesla's specific requirements
    // Use the scopes that user selected (from session), plus openid and offline_access
    let scope_for_token = format!("openid offline_access {}", requested_scopes);
    let token_params = [
        ("grant_type", "authorization_code"),
        ("code", &params.code),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
        ("redirect_uri", &redirect_uri),
        ("code_verifier", pkce_verifier.secret()),
        ("scope", &scope_for_token),
        ("audience", &audience_url),
    ];

    let token_response = match http_client
        .post(token_url)
        .form(&token_params)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to send token exchange request: {}", e);
            return tesla_error_redirect("Failed to exchange authorization code with Tesla");
        }
    };

    if !token_response.status().is_success() {
        let error_text = token_response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        error!("Token exchange failed for user {}: {}", user_id, error_text);
        return tesla_error_redirect("Tesla rejected the authorization. Please try again.");
    }

    let token_data: serde_json::Value = match token_response.json().await {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to parse token response: {}", e);
            return tesla_error_redirect("Invalid response from Tesla");
        }
    };

    let access_token = match token_data["access_token"].as_str() {
        Some(token) => token,
        None => {
            error!("No access token in Tesla response for user {}", user_id);
            return tesla_error_redirect("Tesla did not provide access token");
        }
    };

    let refresh_token = match token_data["refresh_token"].as_str() {
        Some(token) => token,
        None => {
            error!("No refresh token in Tesla response for user {}", user_id);
            return tesla_error_redirect("Tesla did not provide refresh token");
        }
    };

    let expires_in = token_data["expires_in"].as_i64()
        .unwrap_or(3600) as i32;

    // Determine granted scopes - try multiple sources:
    // 1. JWT token claims (scp or scope)
    // 2. Token response body (scope field)
    // 3. Fall back to requested_scopes from session (what user selected in UI)
    let granted_scopes = extract_scopes_from_jwt(access_token)
        .or_else(|| token_data["scope"].as_str().map(|s| s.to_string()))
        .or_else(|| {
            // Use the scopes user requested - Tesla approved them or would have failed
            info!("Using requested_scopes as granted_scopes for user {}", user_id);
            Some(requested_scopes.clone())
        });
    info!("Tesla granted scopes for user {}: {:?}", user_id, granted_scopes);

    // Encrypt tokens
    let encrypted_access_token = match encrypt(access_token) {
        Ok(encrypted) => encrypted,
        Err(e) => {
            error!("Failed to encrypt access token: {}", e);
            return tesla_error_redirect("Failed to secure Tesla credentials");
        }
    };

    let encrypted_refresh_token = match encrypt(refresh_token) {
        Ok(encrypted) => encrypted,
        Err(e) => {
            error!("Failed to encrypt refresh token: {}", e);
            return tesla_error_redirect("Failed to secure Tesla credentials");
        }
    };

    // Determine user's Tesla region based on their phone number
    // This is more reliable than Tesla's region detection API which can be flaky
    let region = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) if user.phone_number.starts_with("+1") => {
            // North American phone number (+1 for US/Canada)
            info!("User {} has +1 phone number, using NA region", user_id);
            "https://fleet-api.prd.na.vn.cloud.tesla.com".to_string()
        }
        Ok(Some(user)) if user.phone_number.starts_with("+86") => {
            // China phone number
            info!("User {} has +86 phone number, using China region", user_id);
            "https://fleet-api.prd.cn.vn.cloud.tesla.cn".to_string()
        }
        _ => {
            // Default to EU region for all other countries
            info!("User {} using EU region (default)", user_id);
            "https://fleet-api.prd.eu.vn.cloud.tesla.com".to_string()
        }
    };
    info!("Using Tesla region: {} for user {}", region, user_id);

    // Get current timestamp
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Store tokens in database
    let new_tesla = NewTesla {
        user_id,
        encrypted_access_token,
        encrypted_refresh_token,
        status: "active".to_string(),
        last_update: current_time,
        created_on: current_time,
        expires_in,
        region,
        granted_scopes,
    };

    if let Err(e) = state.user_repository.create_tesla_connection(new_tesla) {
        error!("Failed to store Tesla connection for user {}: {}", user_id, e);
        return tesla_error_redirect("Failed to save Tesla connection. Please try again.");
    }

    // Clean up session (non-critical)
    let _ = state.session_store.delete(&Id(session_id)).await;

    info!("Tesla OAuth connection successfully established for user {}", user_id);

    // Redirect to frontend home page with success query param
    let frontend_url = std::env::var("FRONTEND_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    Redirect::to(&format!("{}?tesla=success", frontend_url))
}

// Tesla disconnect endpoint
pub async fn tesla_disconnect(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<StatusCode, (StatusCode, String)> {
    info!("Disconnecting Tesla for user {}", auth_user.user_id);

    // Delete connection from database
    state.user_repository
        .delete_tesla_connection(auth_user.user_id)
        .map_err(|e| {
            error!("Failed to delete Tesla connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to delete Tesla connection: {}", e),
            )
        })?;

    info!("Tesla connection successfully removed for user {}", auth_user.user_id);
    Ok(StatusCode::OK)
}

// Tesla status endpoint
pub async fn tesla_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<TeslaStatusResponse>, (StatusCode, String)> {
    let has_tesla = state.user_repository.has_active_tesla(auth_user.user_id).unwrap_or(false);

    Ok(Json(TeslaStatusResponse { has_tesla }))
}

#[derive(Serialize)]
pub struct TeslaScopesResponse {
    pub granted_scopes: Option<String>,
    pub has_vehicle_device_data: bool,
    pub has_vehicle_cmds: bool,
    pub has_vehicle_charging_cmds: bool,
}

/// Tesla scopes endpoint - returns granted scopes for feature gating in UI
pub async fn tesla_scopes(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<TeslaScopesResponse>, (StatusCode, String)> {
    let scopes = state.user_repository
        .get_tesla_granted_scopes(auth_user.user_id)
        .unwrap_or(None);

    let scope_str = scopes.as_deref().unwrap_or("");
    let has_vehicle_device_data = scope_str.contains("vehicle_device_data");
    let has_vehicle_cmds = scope_str.contains("vehicle_cmds");
    let has_vehicle_charging_cmds = scope_str.contains("vehicle_charging_cmds");

    Ok(Json(TeslaScopesResponse {
        granted_scopes: scopes,
        has_vehicle_device_data,
        has_vehicle_cmds,
        has_vehicle_charging_cmds,
    }))
}

/// Refresh Tesla scopes by decoding the current JWT access token
/// This is useful for users who connected before scope tracking was added
pub async fn tesla_refresh_scopes(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<TeslaScopesResponse>, (StatusCode, Json<serde_json::Value>)> {
    info!("Refreshing Tesla scopes for user {}", auth_user.user_id);

    // Get valid access token (will refresh if expired)
    let access_token = get_valid_tesla_access_token(&state, auth_user.user_id).await
        .map_err(|(status, msg)| (status, Json(json!({"error": msg}))))?;

    // Extract scopes from the JWT
    let scopes = extract_scopes_from_jwt(&access_token);

    if let Some(ref scope_str) = scopes {
        // Update scopes in database
        if let Err(e) = state.user_repository.update_tesla_granted_scopes(auth_user.user_id, scope_str.clone()) {
            error!("Failed to update Tesla scopes in DB: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to save scopes"}))
            ));
        }
        info!("Updated Tesla scopes for user {}: {}", auth_user.user_id, scope_str);
    } else {
        warn!("Could not extract scopes from Tesla JWT for user {}", auth_user.user_id);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Could not extract scopes from token. You may need to reconnect Tesla with desired permissions.",
                "needs_reconnect": true
            }))
        ));
    }

    let scope_str = scopes.as_deref().unwrap_or("");
    let has_vehicle_device_data = scope_str.contains("vehicle_device_data");
    let has_vehicle_cmds = scope_str.contains("vehicle_cmds");
    let has_vehicle_charging_cmds = scope_str.contains("vehicle_charging_cmds");

    Ok(Json(TeslaScopesResponse {
        granted_scopes: scopes,
        has_vehicle_device_data,
        has_vehicle_cmds,
        has_vehicle_charging_cmds,
    }))
}

// Helper function to get valid Tesla access token (with auto-refresh)
pub async fn get_valid_tesla_access_token(
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<String, (StatusCode, String)> {
    // Get token info from database
    let (encrypted_access_token, encrypted_refresh_token, expires_in, last_update) = state
        .user_repository
        .get_tesla_token_info(user_id)
        .map_err(|_| {
            (
                StatusCode::NOT_FOUND,
                "No Tesla connection found".to_string(),
            )
        })?;

    // Decrypt tokens
    let access_token = decrypt(&encrypted_access_token).map_err(|e| {
        error!("Failed to decrypt access token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to decrypt access token: {}", e),
        )
    })?;

    let refresh_token = decrypt(&encrypted_refresh_token).map_err(|e| {
        error!("Failed to decrypt refresh token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to decrypt refresh token: {}", e),
        )
    })?;

    // Check if token is expired (with 5 minute buffer)
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let token_expiry = last_update + expires_in;
    let needs_refresh = current_time >= (token_expiry - 300); // 5 minute buffer

    if !needs_refresh {
        return Ok(access_token);
    }

    info!("Tesla access token expired for user {}, refreshing...", user_id);

    // Refresh the token using Tesla's specific token endpoint
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    let token_url = "https://fleet-auth.prd.vn.cloud.tesla.com/oauth2/v3/token";
    let client_id = std::env::var("TESLA_CLIENT_ID")
        .unwrap_or_else(|_| "default-tesla-client-id-for-testing".to_string());
    let client_secret = std::env::var("TESLA_CLIENT_SECRET")
        .unwrap_or_else(|_| "default-tesla-secret-for-testing".to_string());

    // Get the user's region from the database
    let audience_url = state.user_repository.get_tesla_region(user_id).map_err(|e| {
        error!("Failed to get user's Tesla region: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get Tesla region: {}", e),
        )
    })?;

    let scope = "openid offline_access vehicle_device_data vehicle_cmds vehicle_charging_cmds";
    let refresh_params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", &refresh_token),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
        ("scope", scope),
        ("audience", &audience_url),
    ];

    let token_response = http_client
        .post(token_url)
        .form(&refresh_params)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to refresh Tesla token: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to refresh token: {}", e),
            )
        })?;

    if !token_response.status().is_success() {
        let error_text = token_response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        error!("Token refresh failed: {}", error_text);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Token refresh failed: {}", error_text),
        ));
    }

    let token_data: serde_json::Value = token_response.json().await
        .map_err(|e| {
            error!("Failed to parse refresh token response: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to parse token response: {}", e),
            )
        })?;

    let new_access_token = token_data["access_token"].as_str()
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "No access token in refresh response".to_string(),
            )
        })?;

    // Tesla returns a new refresh token on refresh
    let new_refresh_token = token_data["refresh_token"].as_str()
        .unwrap_or(&refresh_token); // Keep old if not provided

    let new_expires_in = token_data["expires_in"].as_i64()
        .unwrap_or(3600) as i32;

    // Encrypt new tokens
    let encrypted_access_token = encrypt(new_access_token).map_err(|e| {
        error!("Failed to encrypt new access token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to encrypt new access token: {}", e),
        )
    })?;

    let encrypted_refresh_token = encrypt(new_refresh_token).map_err(|e| {
        error!("Failed to encrypt new refresh token: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to encrypt new refresh token: {}", e),
        )
    })?;

    // Update tokens in database
    state
        .user_repository
        .update_tesla_access_token(
            user_id,
            encrypted_access_token,
            encrypted_refresh_token,
            new_expires_in,
            current_time,
        )
        .map_err(|e| {
            error!("Failed to update Tesla tokens: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to update Tesla tokens: {}", e),
            )
        })?;

    info!("Tesla access token successfully refreshed for user {}", user_id);
    Ok(new_access_token.to_string())
}

// Get partner authentication token (for app-level operations like registration)
// Uses client_credentials grant instead of authorization_code
pub async fn get_partner_access_token() -> Result<String, Box<dyn std::error::Error>> {
    info!("Requesting Tesla partner authentication token");

    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let token_url = "https://fleet-auth.prd.vn.cloud.tesla.com/oauth2/v3/token";
    let client_id = std::env::var("TESLA_CLIENT_ID")
        .unwrap_or_else(|_| "default-tesla-client-id-for-testing".to_string());
    let client_secret = std::env::var("TESLA_CLIENT_SECRET")
        .unwrap_or_else(|_| "default-tesla-secret-for-testing".to_string());
    let audience_url = std::env::var("TESLA_API_BASE")
        .unwrap_or_else(|_| "https://fleet-api.prd.eu.vn.cloud.tesla.com".to_string());

    // Partner token uses client_credentials grant (no user authorization)
    let token_params = [
        ("grant_type", "client_credentials"),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
        ("scope", "openid vehicle_device_data vehicle_cmds vehicle_charging_cmds"),
        ("audience", &audience_url),
    ];

    let token_response = http_client
        .post(token_url)
        .form(&token_params)
        .send()
        .await?;

    if !token_response.status().is_success() {
        let error_text = token_response.text().await?;
        error!("Partner token request failed: {}", error_text);
        return Err(format!("Partner token request failed: {}", error_text).into());
    }

    let token_data: serde_json::Value = token_response.json().await?;

    let access_token = token_data["access_token"]
        .as_str()
        .ok_or("No access token in partner token response")?
        .to_string();

    info!("Successfully obtained Tesla partner authentication token");
    Ok(access_token)
}

// Serve Tesla public key for vehicle command signing
// This endpoint is required by Tesla at /.well-known/appspecific/com.tesla.3p.public-key.pem
pub async fn serve_tesla_public_key() -> Result<(StatusCode, String), (StatusCode, String)> {
    use crate::utils::tesla_keys;

    match tesla_keys::get_public_key() {
        Ok(public_key) => {
            info!("Serving Tesla public key");
            Ok((StatusCode::OK, public_key))
        }
        Err(e) => {
            error!("Failed to get Tesla public key: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to retrieve public key: {}", e)
            ))
        }
    }
}

// Get virtual key pairing link/QR code for adding key to vehicle
// Users must open this link in their Tesla mobile app to authorize commands
pub async fn get_virtual_key_link(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Generating virtual key pairing link for user {}", auth_user.user_id);

    // Check if user has Tesla connected
    let has_tesla = state.user_repository
        .has_active_tesla(auth_user.user_id)
        .unwrap_or(false);

    if !has_tesla {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "No Tesla connection found. Please connect your Tesla account first."}))
        ));
    }

    // Get domain from environment variable and strip protocol
    // Use TESLA_REDIRECT_URL for the virtual key pairing link (e.g., lightfriend.app)
    // This should be the domain registered with Tesla for key pairing
    let domain = std::env::var("TESLA_REDIRECT_URL")
        .or_else(|_| std::env::var("SERVER_URL"))
        .or_else(|_| std::env::var("SERVER_URL_OAUTH"))
        .unwrap_or_else(|_| "localhost:3000".to_string());

    // Remove protocol (https:// or http://) if present
    let domain = domain
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_string();

    // Generate the Tesla virtual key pairing link
    // Add VIN parameter if provided for vehicle-specific pairing
    let pairing_link = if let Some(vin) = params.get("vin") {
        info!("Generating vehicle-specific pairing link for VIN: {}", vin);
        format!("https://www.tesla.com/_ak/{}?vin={}", domain, vin)
    } else {
        format!("https://www.tesla.com/_ak/{}", domain)
    };

    info!("Generated virtual key pairing link: {}", pairing_link);

    Ok(Json(json!({
        "pairing_link": pairing_link,
        "domain": domain,
        "instructions": "Open this link on your mobile device or scan the QR code in your Tesla mobile app to authorize vehicle commands. This is required before you can control your vehicle remotely.",
        "qr_code_url": format!("https://api.qrserver.com/v1/create-qr-code/?size=300x300&data={}", urlencoding::encode(&pairing_link))
    })))
}

#[derive(Debug, Deserialize)]
pub struct TeslaCommandRequest {
    pub command: String,
    pub vehicle_id: Option<String>,
}

pub async fn tesla_command(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<TeslaCommandRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Tesla command request from user {}: {}", auth_user.user_id, payload.command);

    // Wrap command in JSON format expected by handle_tesla_command
    let args_json = json!({"command": payload.command}).to_string();

    // skip_notification=true because this is from the dashboard, no need to SMS the user
    let result = crate::tool_call_utils::tesla::handle_tesla_command(
        &state,
        auth_user.user_id,
        &args_json,
        true, // skip notification for dashboard calls
    ).await;

    info!("Tesla command result: {}", result);

    Ok(Json(json!({
        "success": true,
        "message": result
    })))
}

pub async fn tesla_battery_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Tesla battery status request from user {}", auth_user.user_id);

    // Check if user has Tesla connected
    let has_tesla = state.user_repository
        .has_active_tesla(auth_user.user_id)
        .unwrap_or(false);

    if !has_tesla {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Tesla not connected"})),
        ));
    }

    // Get valid access token (with auto-refresh)
    let access_token = match get_valid_tesla_access_token(&state, auth_user.user_id).await {
        Ok(token) => token,
        Err((status, msg)) => {
            return Err((status, Json(json!({"error": msg}))));
        }
    };

    // Get region from database
    let region = state.user_repository
        .get_tesla_region(auth_user.user_id)
        .unwrap_or_else(|_| "na".to_string());

    // Create Tesla client
    let tesla_client = crate::api::tesla::TeslaClient::new_with_proxy(&region);

    // Get vehicles
    let vehicles = match tesla_client.get_vehicles(&access_token).await {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to get Tesla vehicles: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to get vehicles"})),
            ));
        }
    };

    if vehicles.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "No vehicles found"})),
        ));
    }

    // Try to use selected vehicle, fall back to first vehicle if none selected
    let selected_vin = state.user_repository
        .get_selected_vehicle_vin(auth_user.user_id)
        .ok()
        .flatten();

    let vehicle = if let Some(vin) = selected_vin.as_ref() {
        match vehicles.iter().find(|v| &v.vin == vin) {
            Some(v) => {
                info!("Using selected vehicle with VIN: {}", vin);
                v
            }
            None => {
                info!("Selected vehicle VIN {} not found, falling back to first vehicle", vin);
                &vehicles[0]
            }
        }
    } else {
        info!("No vehicle selected, using first vehicle");
        &vehicles[0]
    };

    let vehicle_vin = &vehicle.vin;

    // Wake up vehicle if asleep (using deduplication to prevent parallel wake attempts)
    if vehicle.state != "online" {
        info!("Vehicle is asleep (state: {}), waking up...", vehicle.state);
        match tesla_client.wake_up_deduplicated(&access_token, vehicle_vin, &state.tesla_waking_vehicles).await {
            Ok(true) => {
                info!("Vehicle successfully woken up");
            }
            Ok(false) => {
                error!("Vehicle wake-up returned false");
                return Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({"error": "Couldn't wake vehicle. This may be a Tesla server or connectivity issue."})),
                ));
            }
            Err(e) => {
                error!("Failed to wake up vehicle: {}", e);
                return Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({"error": format!("Couldn't reach vehicle. This may be a Tesla server or connectivity issue. {}", e)})),
                ));
            }
        }
    } else {
        info!("Vehicle is already online");
    }

    // Get vehicle data (includes charge_state, climate_state, vehicle_state in one call)
    let vehicle_data = match tesla_client.get_vehicle_data(&access_token, vehicle_vin).await {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to get Tesla vehicle data: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get vehicle data: {}", e)})),
            ));
        }
    };

    // Extract charge state
    let (battery_level, battery_range, charging_state, charge_limit_soc, charge_rate, charger_power, time_to_full_charge, charge_energy_added) =
        if let Some(charge_state) = &vehicle_data.charge_state {
            (
                Some(charge_state.battery_level),
                Some(charge_state.battery_range),
                Some(charge_state.charging_state.clone()),
                Some(charge_state.charge_limit_soc),
                charge_state.charge_rate,
                charge_state.charger_power,
                charge_state.time_to_full_charge,
                charge_state.charge_energy_added,
            )
        } else {
            (None, None, None, None, None, None, None, None)
        };

    // Determine if region uses miles (NA) or kilometers (EU/CN)
    let uses_miles = region.contains(".na.");

    // Extract climate data
    let (inside_temp, outside_temp, is_climate_on, is_front_defroster_on, is_rear_defroster_on) = if let Some(climate) = &vehicle_data.climate_state {
        (
            climate.inside_temp,
            climate.outside_temp,
            climate.is_climate_on,
            climate.is_front_defroster_on,
            climate.is_rear_defroster_on,
        )
    } else {
        (None, None, None, None, None)
    };

    // Extract vehicle state
    let locked = vehicle_data.vehicle_state.as_ref()
        .and_then(|vs| vs.locked);

    Ok(Json(json!({
        "battery_level": battery_level,
        "battery_range": battery_range,
        "charging_state": charging_state,
        "charge_limit_soc": charge_limit_soc,
        "charge_rate": charge_rate,
        "charger_power": charger_power,
        "time_to_full_charge": time_to_full_charge,
        "charge_energy_added": charge_energy_added,
        "uses_miles": uses_miles,
        "inside_temp": inside_temp,
        "outside_temp": outside_temp,
        "is_climate_on": is_climate_on,
        "is_front_defroster_on": is_front_defroster_on,
        "is_rear_defroster_on": is_rear_defroster_on,
        "locked": locked
    })))
}

#[derive(Debug, Deserialize)]
pub struct SetChargeLimitRequest {
    pub percent: i32,
}

pub async fn set_charge_limit(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<SetChargeLimitRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Set charge limit request from user {}: {}%", auth_user.user_id, payload.percent);

    // Validate percent is within Tesla's limits (50-100)
    if payload.percent < 50 || payload.percent > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Charge limit must be between 50 and 100 percent"})),
        ));
    }

    // Check if user has Tesla connected
    let has_tesla = state.user_repository
        .has_active_tesla(auth_user.user_id)
        .unwrap_or(false);

    if !has_tesla {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Tesla not connected"})),
        ));
    }

    // Get valid access token (with auto-refresh)
    let access_token = match get_valid_tesla_access_token(&state, auth_user.user_id).await {
        Ok(token) => token,
        Err((status, msg)) => {
            return Err((status, Json(json!({"error": msg}))));
        }
    };

    // Get region from database
    let region = state.user_repository
        .get_tesla_region(auth_user.user_id)
        .unwrap_or_else(|_| "na".to_string());

    // Get selected vehicle
    let vehicle_info = state.user_repository
        .get_selected_vehicle_info(auth_user.user_id)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to get vehicle info"}))))?
        .ok_or_else(|| (StatusCode::BAD_REQUEST, Json(json!({"error": "No vehicle selected"}))))?;

    let (vehicle_vin, _, _) = vehicle_info;

    // Create Tesla client
    let tesla_client = crate::api::tesla::TeslaClient::new_with_proxy(&region);

    // Wake up vehicle if needed
    match tesla_client.wake_up_deduplicated(&access_token, &vehicle_vin, &state.tesla_waking_vehicles).await {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to wake up vehicle for charge limit: {}", e);
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": format!("Couldn't reach vehicle: {}", e)})),
            ));
        }
    }

    // Set charge limit
    match tesla_client.set_charge_limit(&access_token, &vehicle_vin, payload.percent).await {
        Ok(_) => {
            info!("Successfully set charge limit to {}% for user {}", payload.percent, auth_user.user_id);
            Ok(Json(json!({
                "success": true,
                "message": format!("Charge limit set to {}%", payload.percent)
            })))
        }
        Err(e) => {
            error!("Failed to set charge limit: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to set charge limit: {}", e)})),
            ))
        }
    }
}

pub async fn tesla_list_vehicles(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Tesla list vehicles request from user {}", auth_user.user_id);

    // Check if user has Tesla connected
    let has_tesla = state.user_repository
        .has_active_tesla(auth_user.user_id)
        .unwrap_or(false);

    if !has_tesla {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Tesla not connected"})),
        ));
    }

    // Get valid access token (with auto-refresh)
    let access_token = match get_valid_tesla_access_token(&state, auth_user.user_id).await {
        Ok(token) => token,
        Err((status, msg)) => {
            return Err((status, Json(json!({"error": msg}))));
        }
    };

    // Get region from database
    let region = state.user_repository
        .get_tesla_region(auth_user.user_id)
        .unwrap_or_else(|_| "na".to_string());

    // Create Tesla client
    let tesla_client = crate::api::tesla::TeslaClient::new_with_proxy(&region);

    // Get vehicles
    let vehicles = match tesla_client.get_vehicles(&access_token).await {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to get Tesla vehicles: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to get vehicles"})),
            ));
        }
    };

    // Get currently selected vehicle
    let mut selected_vin = state.user_repository
        .get_selected_vehicle_vin(auth_user.user_id)
        .ok()
        .flatten();

    // Auto-select first vehicle if none is selected
    if selected_vin.is_none() && !vehicles.is_empty() {
        let first_vehicle = &vehicles[0];
        let vin = first_vehicle.vin.clone();
        let name = first_vehicle.display_name.as_ref().unwrap_or(&"Unknown".to_string()).clone();
        let vehicle_id = first_vehicle.id.to_string();

        info!("Auto-selecting first vehicle for user {}: {} (VIN: {})", auth_user.user_id, name, vin);

        // Save selection to database
        if let Err(e) = state.user_repository.set_selected_vehicle(
            auth_user.user_id,
            vin.clone(),
            name,
            vehicle_id
        ) {
            error!("Failed to auto-select vehicle: {}", e);
        } else {
            selected_vin = Some(vin);
        }
    }

    // Get virtual key paired status
    let is_paired = state.user_repository
        .get_tesla_key_paired_status(auth_user.user_id)
        .unwrap_or(false);

    // Format response
    let vehicle_list: Vec<serde_json::Value> = vehicles.iter().map(|v| {
        json!({
            "vin": v.vin,
            "id": v.id.to_string(),
            "vehicle_id": v.vehicle_id.to_string(),
            "name": v.display_name.as_ref().unwrap_or(&"Unknown".to_string()),
            "state": v.state,
            "selected": selected_vin.as_ref().map_or(false, |s| s == &v.vin),
            "paired": is_paired  // Add pairing status
        })
    }).collect();

    Ok(Json(json!({
        "vehicles": vehicle_list,
        "selected_vin": selected_vin
    })))
}

#[derive(Debug, Deserialize)]
pub struct SelectVehicleRequest {
    pub vin: String,
    pub name: String,
    pub vehicle_id: String,
}

pub async fn tesla_select_vehicle(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<SelectVehicleRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Tesla select vehicle request from user {}: VIN {}", auth_user.user_id, payload.vin);

    // Check if user has Tesla connected
    let has_tesla = state.user_repository
        .has_active_tesla(auth_user.user_id)
        .unwrap_or(false);

    if !has_tesla {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Tesla not connected"})),
        ));
    }

    // Update selected vehicle in database
    match state.user_repository.set_selected_vehicle(
        auth_user.user_id,
        payload.vin.clone(),
        payload.name.clone(),
        payload.vehicle_id.clone(),
    ) {
        Ok(_) => {
            info!("Successfully updated selected vehicle for user {}: {} (VIN: {})",
                  auth_user.user_id, payload.name, payload.vin);
            Ok(Json(json!({
                "success": true,
                "message": format!("Selected vehicle: {}", payload.name)
            })))
        }
        Err(e) => {
            error!("Failed to update selected vehicle: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update selected vehicle"})),
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct MarkPairedRequest {
    pub paired: bool,
}

pub async fn tesla_mark_paired(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<MarkPairedRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    info!("Tesla mark paired request from user {}: paired={}", auth_user.user_id, payload.paired);

    // Check if user has Tesla connected
    let has_tesla = state.user_repository
        .has_active_tesla(auth_user.user_id)
        .unwrap_or(false);

    if !has_tesla {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Tesla not connected"})),
        ));
    }

    // Update paired status in database
    match state.user_repository.mark_tesla_key_paired(auth_user.user_id, payload.paired) {
        Ok(_) => {
            info!("Successfully updated Tesla key paired status for user {}: {}",
                  auth_user.user_id, payload.paired);
            Ok(Json(json!({
                "success": true,
                "paired": payload.paired,
                "message": if payload.paired {
                    "Virtual key marked as paired"
                } else {
                    "Virtual key marked as not paired"
                }
            })))
        }
        Err(e) => {
            error!("Failed to update paired status: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update paired status"})),
            ))
        }
    }
}

// Climate monitoring status endpoint
pub async fn get_climate_notify_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let is_active = state.tesla_monitoring_tasks.contains_key(&auth_user.user_id);
    Ok(Json(json!({ "active": is_active })))
}

// Start climate monitoring from UI
pub async fn start_climate_notify(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check if already monitoring
    if state.tesla_monitoring_tasks.contains_key(&auth_user.user_id) {
        return Ok(Json(json!({ "success": true, "message": "Already monitoring" })));
    }

    // Get Tesla connection info
    let has_tesla = state.user_repository.has_active_tesla(auth_user.user_id)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to check Tesla connection"}))))?;

    if !has_tesla {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Tesla not connected"}))));
    }

    // Get access token
    let access_token = match crate::handlers::tesla_auth::get_valid_tesla_access_token(&state, auth_user.user_id).await {
        Ok(token) => token,
        Err((_, msg)) => return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg})))),
    };

    // Get region
    let region = state.user_repository.get_tesla_region(auth_user.user_id)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to get Tesla region"}))))?;

    // Get selected vehicle
    let vehicle_info = state.user_repository.get_selected_vehicle_info(auth_user.user_id)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to get vehicle info"}))))?
        .ok_or_else(|| (StatusCode::BAD_REQUEST, Json(json!({"error": "No vehicle selected"}))))?;

    let (vehicle_vin, vehicle_name, _) = vehicle_info;

    // Spawn monitoring task
    let state_clone = state.clone();
    let user_id = auth_user.user_id;
    let handle = tokio::spawn(async move {
        info!("Starting UI-initiated climate monitoring for user {}", user_id);
        let tesla_client = crate::api::tesla::TeslaClient::new_with_proxy(&region);

        let monitoring_result = tesla_client.monitor_climate_ready(&access_token, &vehicle_vin).await
            .map_err(|e| e.to_string());

        match monitoring_result {
            Ok(Some(temp)) => {
                let msg = format!("Your {} is ready to drive! Cabin temp is {:.1}°C.", &vehicle_name, temp);
                crate::proactive::utils::send_notification(
                    &state_clone,
                    user_id,
                    &msg,
                    "tesla_ready_to_drive".to_string(),
                    Some(format!("Your {} is warmed up and ready to drive!", &vehicle_name)),
                ).await;
            }
            Ok(None) => {
                let msg = format!("Your {} should be ready by now (climate running 20+ min).", &vehicle_name);
                crate::proactive::utils::send_notification(
                    &state_clone,
                    user_id,
                    &msg,
                    "tesla_ready_timeout".to_string(),
                    Some(format!("Your {} should be warmed up by now.", &vehicle_name)),
                ).await;
            }
            Err(error_msg) => {
                if error_msg.contains("turned off") {
                    crate::proactive::utils::send_notification(
                        &state_clone,
                        user_id,
                        "Tesla climate was turned off before reaching target temperature.",
                        "tesla_climate_stopped".to_string(),
                        Some(format!("Your {} climate was stopped early.", &vehicle_name)),
                    ).await;
                }
            }
        }

        state_clone.tesla_monitoring_tasks.remove(&user_id);
        info!("UI-initiated climate monitoring completed for user {}", user_id);
    });

    state.tesla_monitoring_tasks.insert(auth_user.user_id, handle);
    Ok(Json(json!({ "success": true, "message": "Climate monitoring started" })))
}

// Cancel climate monitoring from UI
pub async fn cancel_climate_notify(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if let Some((_, handle)) = state.tesla_monitoring_tasks.remove(&auth_user.user_id) {
        handle.abort();
        info!("Cancelled climate monitoring for user {}", auth_user.user_id);
        Ok(Json(json!({ "success": true, "message": "Climate monitoring cancelled" })))
    } else {
        Ok(Json(json!({ "success": true, "message": "No monitoring was active" })))
    }
}

// Charging monitoring status endpoint
pub async fn get_charging_notify_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let is_active = state.tesla_charging_monitor_tasks.contains_key(&auth_user.user_id);
    Ok(Json(json!({ "active": is_active })))
}

// Start charging monitoring from UI
pub async fn start_charging_notify(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Check if already monitoring
    if state.tesla_charging_monitor_tasks.contains_key(&auth_user.user_id) {
        return Ok(Json(json!({ "success": true, "message": "Already monitoring" })));
    }

    // Get Tesla connection info
    let has_tesla = state.user_repository.has_active_tesla(auth_user.user_id)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to check Tesla connection"}))))?;

    if !has_tesla {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "Tesla not connected"}))));
    }

    // Get access token
    let access_token = match crate::handlers::tesla_auth::get_valid_tesla_access_token(&state, auth_user.user_id).await {
        Ok(token) => token,
        Err((_, msg)) => return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg})))),
    };

    // Get region
    let region = state.user_repository.get_tesla_region(auth_user.user_id)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to get Tesla region"}))))?;

    // Get selected vehicle
    let vehicle_info = state.user_repository.get_selected_vehicle_info(auth_user.user_id)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to get vehicle info"}))))?
        .ok_or_else(|| (StatusCode::BAD_REQUEST, Json(json!({"error": "No vehicle selected"}))))?;

    let (vehicle_vin, vehicle_name, _) = vehicle_info;

    // Spawn monitoring task
    let state_clone = state.clone();
    let user_id = auth_user.user_id;
    let handle = tokio::spawn(async move {
        info!("Starting UI-initiated charging monitoring for user {}", user_id);
        let tesla_client = crate::api::tesla::TeslaClient::new_with_proxy(&region);

        let monitoring_result = tesla_client.monitor_charging_complete(&access_token, &vehicle_vin).await
            .map_err(|e| e.to_string());

        match monitoring_result {
            Ok(Some(battery_level)) => {
                // Check if user is present in the vehicle before sending notification
                let is_user_present = match tesla_client.get_vehicle_data(&access_token, &vehicle_vin).await {
                    Ok(data) => data.vehicle_state.and_then(|vs| vs.is_user_present).unwrap_or(false),
                    Err(_) => false,
                };

                if is_user_present {
                    info!("User is present in vehicle, skipping charging complete notification for user {}", user_id);
                } else {
                    let msg = format!("Your {} has finished charging! Battery is now at {}%.", &vehicle_name, battery_level);
                    crate::proactive::utils::send_notification(
                        &state_clone,
                        user_id,
                        &msg,
                        "tesla_charging_complete".to_string(),
                        Some(format!("Your {} is done charging!", &vehicle_name)),
                    ).await;
                }
            }
            Ok(None) => {
                // Timeout or charging stopped
                info!("Charging monitoring timed out or charging stopped for user {}", user_id);
            }
            Err(error_msg) => {
                error!("Charging monitoring error for user {}: {}", user_id, error_msg);
            }
        }

        state_clone.tesla_charging_monitor_tasks.remove(&user_id);
        info!("UI-initiated charging monitoring completed for user {}", user_id);
    });

    state.tesla_charging_monitor_tasks.insert(auth_user.user_id, handle);
    Ok(Json(json!({ "success": true, "message": "Charging monitoring started" })))
}

// Cancel charging monitoring from UI
pub async fn cancel_charging_notify(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if let Some((_, handle)) = state.tesla_charging_monitor_tasks.remove(&auth_user.user_id) {
        handle.abort();
        info!("Cancelled charging monitoring for user {}", auth_user.user_id);
        Ok(Json(json!({ "success": true, "message": "Charging monitoring cancelled" })))
    } else {
        Ok(Json(json!({ "success": true, "message": "No monitoring was active" })))
    }
}

/// SSE endpoint for streaming Tesla command progress
/// Streams status updates like "Waking up Tesla...", "Setting climate...", etc.
#[derive(Debug, Deserialize)]
pub struct TeslaCommandStreamRequest {
    pub command: String,
}

pub async fn tesla_command_stream(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(params): Query<TeslaCommandStreamRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let user_id = auth_user.user_id;
    let command = params.command.clone();

    info!("Tesla command stream started for user {}: {}", user_id, command);

    // Create the async stream that yields SSE events
    let stream = async_stream::stream! {
        // Send initial status
        yield Ok(Event::default().data(json!({
            "step": "starting",
            "message": "Connecting to Tesla..."
        }).to_string()));

        // Check if user has Tesla connected
        let has_tesla = match state.user_repository.has_active_tesla(user_id) {
            Ok(has) => has,
            Err(e) => {
                error!("Failed to check Tesla connection: {}", e);
                yield Ok(Event::default().data(json!({
                    "step": "error",
                    "message": "Failed to check Tesla connection"
                }).to_string()));
                return;
            }
        };

        if !has_tesla {
            yield Ok(Event::default().data(json!({
                "step": "error",
                "message": "No Tesla account connected"
            }).to_string()));
            return;
        }

        // Get valid access token (handles refresh if needed)
        let access_token = match get_valid_tesla_access_token(&state, user_id).await {
            Ok(token) => token,
            Err((_, msg)) => {
                error!("Failed to get Tesla access token: {}", msg);
                yield Ok(Event::default().data(json!({
                    "step": "error",
                    "message": format!("Failed to authenticate with Tesla: {}", msg)
                }).to_string()));
                return;
            }
        };

        // Get user's Tesla region
        let region = match state.user_repository.get_tesla_region(user_id) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to get Tesla region: {}", e);
                yield Ok(Event::default().data(json!({
                    "step": "error",
                    "message": "Failed to get Tesla region settings"
                }).to_string()));
                return;
            }
        };

        // Create Tesla client with user's region and proxy support
        let tesla_client = crate::api::tesla::TeslaClient::new_with_proxy(&region);

        // Get vehicles
        yield Ok(Event::default().data(json!({
            "step": "fetching",
            "message": "Getting vehicle list..."
        }).to_string()));

        let vehicles = match tesla_client.get_vehicles(&access_token).await.map_err(|e| e.to_string()) {
            Ok(v) => v,
            Err(err_msg) => {
                error!("Failed to get Tesla vehicles: {}", err_msg);
                yield Ok(Event::default().data(json!({
                    "step": "error",
                    "message": format!("Couldn't reach Tesla servers. This may be a temporary issue. {}", err_msg)
                }).to_string()));
                return;
            }
        };

        if vehicles.is_empty() {
            yield Ok(Event::default().data(json!({
                "step": "error",
                "message": "No vehicles found in your Tesla account"
            }).to_string()));
            return;
        }

        let vehicle = &vehicles[0];
        let vehicle_vin = &vehicle.vin;
        let vehicle_name = vehicle.display_name.as_deref().unwrap_or("your Tesla");

        // Check if vehicle needs waking
        if vehicle.state != "online" {
            yield Ok(Event::default().data(json!({
                "step": "waking",
                "message": format!("Waking up {}...", vehicle_name)
            }).to_string()));

            // Use deduplicated wake to prevent parallel wake attempts
            match tesla_client.wake_up_deduplicated(&access_token, vehicle_vin, &state.tesla_waking_vehicles).await.map_err(|e| e.to_string()) {
                Ok(true) => {
                    yield Ok(Event::default().data(json!({
                        "step": "awake",
                        "message": format!("{} is now awake", vehicle_name)
                    }).to_string()));
                }
                Ok(false) | Err(_) => {
                    yield Ok(Event::default().data(json!({
                        "step": "error",
                        "message": format!("Couldn't wake {}. This may be a Tesla server or connectivity issue.", vehicle_name)
                    }).to_string()));
                    return;
                }
            }
        }

        // Execute the command
        let command_display = match command.as_str() {
            "climate_on" => "Turning on climate control",
            "climate_off" => "Turning off climate control",
            "lock" => "Locking",
            "unlock" => "Unlocking",
            "defrost" => "Starting defrost mode",
            "remote_start" => "Starting remote drive",
            "cabin_overheat_on" => "Enabling Cabin Overheat Protection",
            "cabin_overheat_off" => "Disabling Cabin Overheat Protection",
            "cabin_overheat_fan_only" => "Setting Cabin Overheat Protection to Fan Only",
            _ => "Sending command"
        };

        yield Ok(Event::default().data(json!({
            "step": "executing",
            "message": format!("{}...", command_display)
        }).to_string()));

        // Execute via the tool handler (reuse existing logic)
        let args_json = json!({"command": command}).to_string();
        let result = crate::tool_call_utils::tesla::handle_tesla_command(
            &state,
            user_id,
            &args_json,
            true, // skip notification for dashboard calls
        ).await;

        // Determine if it was successful based on the result message
        let is_success = result.to_lowercase().contains("success")
            || result.to_lowercase().contains("started")
            || result.to_lowercase().contains("activated")
            || result.to_lowercase().contains("enabled")
            || result.to_lowercase().contains("disabled")
            || result.to_lowercase().contains("stopped")
            || result.to_lowercase().contains("locked")
            || result.to_lowercase().contains("unlocked");

        if is_success {
            yield Ok(Event::default().data(json!({
                "step": "done",
                "message": result
            }).to_string()));
        } else {
            yield Ok(Event::default().data(json!({
                "step": "error",
                "message": result
            }).to_string()));
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}
