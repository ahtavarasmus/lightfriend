use crate::handlers::auth_middleware::AuthUser;
use crate::UserCoreOps;
use axum::{extract::State, http::StatusCode, response::Response, Json};
use chrono::{Duration, Utc};
use governor::{Quota, RateLimiter};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::Deserialize;
use serde_json::json;
use std::env;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    handlers::auth_dtos::{LoginRequest, NewUser, RegisterRequest, UserResponse},
    AppState,
};

#[derive(Deserialize)]
pub struct CompletePasswordResetRequest {
    pub token: String,
    pub new_password: String,
}

use serde::Serialize;
#[derive(Serialize)]
pub struct PasswordResetResponse {
    message: String,
}

pub async fn get_users(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
) -> Result<Json<Vec<UserResponse>>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Attempting to get all users");
    let users_list = state.user_core.get_all_users().map_err(|e| {
        tracing::error!("Database error while fetching users: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database error"})),
        )
    })?;

    tracing::debug!("Converting users to response format");
    let mut users_response = Vec::with_capacity(users_list.len());

    for user in users_list {
        // Get user settings, providing defaults if not found
        let settings = state.user_core.get_user_settings(user.id).map_err(|e| {
            tracing::error!("Database error while fetching user settings: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            )
        })?;

        // Check if user has their own Twilio credentials (for BYOT plan)
        let has_twilio_credentials = state.user_core.has_twilio_credentials(user.id);

        users_response.push(UserResponse {
            id: user.id,
            email: user.email,
            phone_number: user.phone_number.clone(),
            nickname: user.nickname,
            time_to_live: user.time_to_live,
            verified: user.verified,
            credits: user.credits,
            notify: settings.notify,
            preferred_number: user.preferred_number,
            sub_tier: user.sub_tier,
            credits_left: user.credits_left,
            discount_tier: user.discount_tier,
            plan_type: user.plan_type,
            has_twilio_credentials,
        });
    }

    tracing::debug!("Successfully retrieved {} users", users_response.len());
    Ok(Json(users_response))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(login_req): Json<LoginRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Login attempt for email: {}", login_req.email);

    // Define rate limit: 5 attempts per minute
    let quota = Quota::per_minute(NonZeroU32::new(5).unwrap());
    let limiter_key = login_req.email.clone(); // Use email as the key

    // Get or create a keyed rate limiter for this email
    let entry = state
        .login_limiter
        .entry(limiter_key.clone())
        .or_insert_with(|| RateLimiter::keyed(quota)); // Bind the Entry here
    let limiter = entry.value(); // Now borrow from the bound value

    // Check if rate limit is exceeded
    if limiter.check_key(&limiter_key).is_err() {
        tracing::warn!("Rate limit exceeded for login attempt");
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Too many login attempts, try again later"})),
        ));
    }

    // Constant-time login: Always run bcrypt verification to prevent timing attacks
    // that could reveal whether an email exists in the database
    const DUMMY_HASH: &str = "$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/X4.VTtYA/lVQ.S1Gu";

    let (user, hash_to_verify) = match state.user_core.find_by_email(&login_req.email) {
        Ok(Some(user)) => {
            let hash = user.password_hash.clone();
            (Some(user), hash)
        }
        Ok(None) => (None, DUMMY_HASH.to_string()),
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            ));
        }
    };

    // Always run bcrypt verification (constant-time behavior)
    let password_valid = bcrypt::verify(&login_req.password, &hash_to_verify).unwrap_or(false);

    // Check both conditions: user must exist AND password must be valid
    let user = match user {
        Some(u) if password_valid => u,
        _ => {
            // Generic error message to not reveal whether user exists
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid credentials"})),
            ));
        }
    };

    // Password is valid, proceed with 2FA check
    // Check if 2FA methods are enabled for this user
    let totp_enabled = state
        .totp_repository
        .is_totp_enabled(user.id)
        .unwrap_or(false);
    let webauthn_enabled = state
        .webauthn_repository
        .has_passkeys(user.id)
        .unwrap_or(false);

    if totp_enabled || webauthn_enabled {
        // Generate a temporary token for the 2FA step
        let login_token: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        // Set expiry to 5 minutes from now
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + 300; // 5 minutes

        // Store the pending login (used by both TOTP and WebAuthn)
        state
            .pending_totp_logins
            .insert(login_token.clone(), (user.id, expiry));

        // Return response indicating 2FA is required with available methods
        let mut response = Response::new(axum::body::Body::from(
            serde_json::to_string(&json!({
                "requires_2fa": true,
                "totp_enabled": totp_enabled,
                "webauthn_enabled": webauthn_enabled,
                "login_token": login_token,
                "message": "Please complete 2FA verification"
            }))
            .unwrap(),
        ));
        *response.status_mut() = StatusCode::OK;
        response
            .headers_mut()
            .insert("Content-Type", "application/json".parse().unwrap());
        return Ok(response);
    }

    generate_tokens_and_response(user.id)
}

#[derive(serde::Deserialize)]
pub struct SendOtpRequest {
    phone_number: String,
}

#[derive(serde::Deserialize)]
pub struct VerifyOtpRequest {
    phone_number: String,
    otp: String,
}

pub async fn request_phone_verify(
    State(state): State<Arc<AppState>>,
    Json(reset_req): Json<SendOtpRequest>,
) -> Result<Json<PasswordResetResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Define rate limit: 3 attempts per hour per phone_number
    let quota = Quota::per_hour(NonZeroU32::new(3).unwrap());
    let limiter_key = reset_req.phone_number.clone();
    // Get or create a rate limiter for this phone_number
    let entry = state
        .phone_verify_limiter
        .entry(limiter_key.clone())
        .or_insert_with(|| RateLimiter::keyed(quota));
    let limiter = entry.value();
    // Check if rate limit is exceeded
    if limiter.check_key(&limiter_key).is_err() {
        tracing::warn!("Rate limit exceeded for phone verify request: [redacted phone]");
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Too many verification attempts. Please try again later."})),
        ));
    }
    // Find user by phone_number
    let user = match state
        .user_core
        .find_by_phone_number(&reset_req.phone_number)
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "No user found with this phone number"})),
            ));
        }
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            ));
        }
    };
    // Generate 6-digit OTP
    let otp: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Uniform::new(0, 10))
        .take(6)
        .map(|d| d.to_string())
        .collect();
    // Store OTP with expiration (5 minutes from now)
    let expiration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 300; // 5 minutes
               // Remove any existing OTP for this phone_number first
    state.phone_verify_otps.remove(&reset_req.phone_number);
    // Insert the new OTP
    state
        .phone_verify_otps
        .insert(reset_req.phone_number.clone(), (otp.clone(), expiration));
    tracing::debug!(
        "Stored OTP {} for phone {} with expiration {}",
        otp,
        reset_req.phone_number,
        expiration
    );
    let message = format!(
        "Your Lightfriend verification code is: {}. Valid for 5 minutes.",
        otp
    );
    if state
        .twilio_message_service
        .send_sms(&message, None, &user)
        .await
        .is_err()
    {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to send OTP"})),
        ));
    }
    Ok(Json(PasswordResetResponse {
        message: "Verification code sent to your phone".to_string(),
    }))
}

pub async fn verify_phone_verify(
    State(state): State<Arc<AppState>>,
    Json(verify_req): Json<VerifyOtpRequest>,
) -> Result<Json<PasswordResetResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Define rate limit: 3 attempts per 60 minutes per phone_number
    let quota = Quota::with_period(std::time::Duration::from_secs(60 * 60))
        .unwrap()
        .allow_burst(NonZeroU32::new(3).unwrap());
    let limiter_key = verify_req.phone_number.clone();
    // Get or create a rate limiter for this phone_number
    let entry = state
        .phone_verify_verify_limiter
        .entry(limiter_key.clone())
        .or_insert_with(|| RateLimiter::keyed(quota));
    let limiter = entry.value();
    // Check if rate limit is exceeded
    if limiter.check_key(&limiter_key).is_err() {
        tracing::warn!("Rate limit exceeded for phone verify verification: [redacted phone]");
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Too many verification attempts. Please try again later."})),
        ));
    }
    tracing::debug!(
        "Verifying OTP {} for phone {}",
        verify_req.otp,
        verify_req.phone_number
    );

    // Remove the OTP data immediately to prevent any hanging references
    let otp_data = match state.phone_verify_otps.remove(&verify_req.phone_number) {
        Some((_, data)) => data, // The first element is the key (phone), second is the value tuple
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "No valid OTP found for this phone number"})),
            ));
        }
    };
    let (stored_otp, expiration_time) = otp_data;
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    if current_time > expiration_time {
        tracing::debug!(
            "OTP expired: current_time {} > expiration {}",
            current_time,
            expiration_time
        );
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "OTP has expired"})),
        ));
    }
    if verify_req.otp != stored_otp {
        tracing::debug!(
            "OTP mismatch: provided {} != stored {}",
            verify_req.otp,
            stored_otp
        );
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid OTP"})),
        ));
    }
    // Find user by phone_number to verify
    let user = match state
        .user_core
        .find_by_phone_number(&verify_req.phone_number)
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "No user found with this phone number"})),
            ));
        }
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            ));
        }
    };
    // Verify the user
    if let Err(e) = state.user_core.verify_user(user.id) {
        tracing::error!("Error verifying user: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to verify user"})),
        ));
    }
    tracing::info!("User verified successfully");

    // Create success response
    let response = PasswordResetResponse {
        message: "Phone number has been verified successfully.".to_string(),
    };

    Ok(Json(response))
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(reg_req): Json<RegisterRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Registration attempt for email: {}", reg_req.email);
    use regex::Regex;
    let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    if !email_regex.is_match(&reg_req.email) {
        tracing::debug!("Invalid email format: {}", reg_req.email);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid email format"})),
        ));
    }
    // Check if email exists
    tracing::debug!("Checking if email exists...");
    if state.user_core.email_exists(&reg_req.email).map_err(|e| {
        tracing::error!("Database error while checking email: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error") })),
        )
    })? {
        tracing::debug!("Email {} already exists", reg_req.email);
        return Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": "Email already exists" })),
        ));
    }
    tracing::debug!("Email is available");
    let phone_regex = Regex::new(r"^\+[1-9]\d{1,14}$").unwrap();
    if !phone_regex.is_match(&reg_req.phone_number) {
        tracing::debug!("Invalid phone number format: {}", reg_req.phone_number);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Phone number must be in E.164 format (e.g., +1234567890)"})),
        ));
    }
    if reg_req.password.len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Password must be 8+ characters" })),
        ));
    }
    // Check if phone number exists
    tracing::debug!("Checking if phone number exists...");
    if state
        .user_core
        .phone_number_exists(&reg_req.phone_number)
        .map_err(|e| {
            tracing::error!("Database error while checking phone number: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error") })),
            )
        })?
    {
        tracing::debug!("Phone number {} already exists", reg_req.phone_number);
        return Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": "Phone number already registered" })),
        ));
    }
    tracing::debug!("Phone number is available");
    // Hash password
    tracing::debug!("Hashing password...");
    let password_hash = bcrypt::hash(&reg_req.password, bcrypt::DEFAULT_COST).map_err(|e| {
        tracing::error!("Password hashing failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Password hashing failed") })),
        )
    })?;
    tracing::debug!("Password hashed successfully");
    // Create and insert user
    tracing::debug!("Creating new user...");
    // Calculate timestamp 5 minutes from now
    let five_minutes_from_now = Utc::now()
        .checked_add_signed(Duration::minutes(5))
        .expect("Failed to calculate timestamp")
        .timestamp() as i32;
    tracing::debug!("Set the time to live due in 5 minutes");
    let reg_r = reg_req.clone();
    let new_user = NewUser {
        email: reg_r.email,
        password_hash,
        phone_number: reg_r.phone_number,
        time_to_live: five_minutes_from_now,
        verified: true, // No phone verification required
        credits: 0.00,
        credits_left: 0.00,
        charge_when_under: false,
        waiting_checks_count: 0,
        discount: false,
        sub_tier: None,
    };
    state.user_core.create_user(new_user).map_err(|e| {
        tracing::error!("User creation failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("User creation failed") })),
        )
    })?;
    tracing::info!("User registered successfully, setting preferred number");

    // Get the newly created user to get their ID
    let user = state
        .user_core
        .find_by_email(&reg_req.email)
        .map_err(|_e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to retrieve user")})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "User not found after registration"})),
            )
        })?;
    // Set preferred number based on detected country
    if let Some(country) = crate::utils::country::get_country_code_from_phone(&reg_req.phone_number)
    {
        let _ = state
            .user_core
            .set_preferred_number_for_country(user.id, &country);
    }
    generate_tokens_and_response(user.id)
}

pub async fn refresh_token(
    State(_state): State<Arc<AppState>>,
    headers: reqwest::header::HeaderMap,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let refresh_token = match headers.get("cookie") {
        Some(cookie_header) => {
            let cookies = cookie_header.to_str().unwrap_or("");
            cookies
                .split(';')
                .find(|c| c.trim().starts_with("refresh_token="))
                .and_then(|c| c.split('=').nth(1))
                .map(|t| t.to_string())
                .ok_or((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "Missing refresh token"})),
                ))?
        }
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing cookies"})),
            ));
        }
    };

    // Validate refresh token
    let validation = Validation::default();
    let token_data = decode::<serde_json::Value>(
        &refresh_token,
        &DecodingKey::from_secret(
            env::var("JWT_REFRESH_KEY")
                .expect("JWT_REFRESH_KEY must be set")
                .as_ref(),
        ),
        &validation,
    )
    .map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid refresh token"})),
        )
    })?;

    let user_id: i32 = token_data.claims["sub"].as_i64().unwrap_or(0) as i32;
    if user_id == 0 {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid user in token"})),
        ));
    }

    // Optional: Rotate refresh token by generating a new one
    generate_tokens_and_response(user_id)
}

pub fn generate_tokens_and_response(
    user_id: i32,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Generate access token (1 hour)
    let access_token = encode(
        &Header::default(),
        &json!({
            "sub": user_id,
            "exp": (Utc::now() + Duration::hours(1)).timestamp(),
            "type": "access"
        }),
        &EncodingKey::from_secret(
            std::env::var("JWT_SECRET_KEY")
                .expect("JWT_SECRET_KEY must be set in environment")
                .as_bytes(),
        ),
    )
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Token generation failed"})),
        )
    })?;

    // Generate refresh token (90 days)
    let refresh_token = encode(
        &Header::default(),
        &json!({
            "sub": user_id,
            "exp": (Utc::now() + Duration::days(90)).timestamp(),
            "type": "refresh"
        }),
        &EncodingKey::from_secret(
            std::env::var("JWT_REFRESH_KEY")
                .expect("JWT_REFRESH_KEY must be set in environment")
                .as_bytes(),
        ),
    )
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Token generation failed"})),
        )
    })?;

    // Create response with HttpOnly cookies
    let mut response = Response::new(axum::body::Body::from(
        Json(json!({"message": "Tokens generated", "token": access_token.clone()})).to_string(),
    ));
    // Don't use Secure flag in development (HTTP), only in production (HTTPS)
    // Use SameSite=Lax to allow cookies on redirects (Strict blocks them)
    let is_development =
        std::env::var("ENVIRONMENT").unwrap_or_else(|_| "production".to_string()) == "development";
    let cookie_options = if is_development {
        "; HttpOnly; SameSite=Lax; Path=/"
    } else {
        "; HttpOnly; Secure; SameSite=Lax; Path=/"
    };

    response.headers_mut().insert(
        "Set-Cookie",
        format!(
            "access_token={}{}; Max-Age=3600",
            access_token, cookie_options
        )
        .parse()
        .unwrap(),
    );
    response.headers_mut().append(
        "Set-Cookie",
        format!(
            "refresh_token={}{}; Max-Age=7776000",
            refresh_token, cookie_options
        )
        .parse()
        .unwrap(),
    );
    response
        .headers_mut()
        .insert("Content-Type", "application/json".parse().unwrap());
    Ok(response)
}

pub async fn auth_status(
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    Ok(Json(json!({
        "authenticated": true,
        "user_id": auth_user.user_id,
        "is_admin": auth_user.is_admin
    })))
}

pub async fn logout() -> Result<Response, StatusCode> {
    // Create response that clears both authentication cookies
    let mut response = Response::new(axum::body::Body::from(
        Json(json!({"message": "Logged out successfully"})).to_string(),
    ));

    let is_development =
        std::env::var("ENVIRONMENT").unwrap_or_else(|_| "production".to_string()) == "development";
    let cookie_clear_options = if is_development {
        "; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"
    } else {
        "; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0"
    };

    // Clear both cookies by setting Max-Age=0
    response.headers_mut().insert(
        "Set-Cookie",
        format!("access_token={}", cookie_clear_options)
            .parse()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    response.headers_mut().append(
        "Set-Cookie",
        format!("refresh_token={}", cookie_clear_options)
            .parse()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    response.headers_mut().insert(
        "Content-Type",
        "application/json"
            .parse()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );

    Ok(response)
}

// ==================== Magic Link Handlers ====================

use axum::extract::Path;

#[derive(Deserialize)]
pub struct SetPasswordRequest {
    pub token: String,
    pub password: String,
}

/// Validate magic link token and check if password needs to be set
/// GET /api/auth/magic/:token
pub async fn validate_magic_link(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Find user by magic_token
    let user = state
        .user_core
        .find_by_magic_token(&token)
        .map_err(|e| {
            tracing::error!("Database error finding user by magic token: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Invalid or expired magic link"})),
            )
        })?;

    // Check if password is set (password_hash is not empty/placeholder)
    let needs_password = user.password_hash.is_empty() || user.password_hash == "NOT_SET";

    if needs_password {
        // User needs to set password - return token for frontend to use
        let response_body = json!({
            "needs_password": true,
            "token": token
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(response_body.to_string()))
            .unwrap())
    } else {
        // User already has password - auto-login and return JWT
        generate_tokens_and_response(user.id)
    }
}

/// Set password using magic token
/// POST /api/auth/set-password
pub async fn set_password_from_magic_link(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetPasswordRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Validate password length
    if req.password.len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Password must be at least 8 characters"})),
        ));
    }

    // Find user by magic_token
    let user = state
        .user_core
        .find_by_magic_token(&req.token)
        .map_err(|e| {
            tracing::error!("Database error finding user by magic token: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Invalid or expired token"})),
            )
        })?;

    // Hash the new password
    let password_hash = bcrypt::hash(&req.password, bcrypt::DEFAULT_COST).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to hash password"})),
        )
    })?;

    // Update the password
    state
        .user_core
        .update_password(user.id, &password_hash)
        .map_err(|e| {
            tracing::error!("Failed to update password: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update password"})),
            )
        })?;

    tracing::info!("User {} set their password via magic link", user.id);

    // Generate JWT tokens and return
    generate_tokens_and_response(user.id)
}

/// Get magic token from Stripe session ID (for redirect flow)
/// GET /api/auth/session-token/:session_id
pub async fn get_token_from_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Look up the token from the session_to_token map
    let token = state
        .session_to_token
        .get(&session_id)
        .map(|v| v.clone())
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Session not found or expired"})),
            )
        })?;

    // Remove the mapping (single use)
    state.session_to_token.remove(&session_id);

    // Check if this is an existing user checkout (should redirect to login instead of auto-login)
    if token == "EXISTING_USER" {
        return Ok(Json(json!({
            "existing_user": true
        })));
    }

    // Check if this is a new user checkout (should check email for magic link)
    if token == "NEW_USER_CHECK_EMAIL" {
        return Ok(Json(json!({
            "new_user_check_email": true
        })));
    }

    Ok(Json(json!({
        "token": token
    })))
}

// ==================== Waitlist Handler ====================

use crate::models::user_models::NewWaitlistEntry;
use crate::schema::waitlist;
use diesel::prelude::*;

#[derive(Deserialize)]
pub struct WaitlistRequest {
    pub email: String,
}

/// Add email to waitlist
/// POST /api/waitlist
pub async fn add_to_waitlist(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WaitlistRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Validate email format
    let email_regex = regex::Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
        .expect("Invalid regex");

    if !email_regex.is_match(&req.email) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid email format"})),
        ));
    }

    let email = req.email.to_lowercase();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let entry = NewWaitlistEntry {
        email: email.clone(),
        created_at: now,
    };

    let mut conn = state.db_pool.get().map_err(|e| {
        tracing::error!("Failed to get DB connection: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database error"})),
        )
    })?;

    // Insert, ignoring if already exists
    diesel::insert_into(waitlist::table)
        .values(&entry)
        .on_conflict(waitlist::email)
        .do_nothing()
        .execute(&mut conn)
        .map_err(|e| {
            tracing::error!("Failed to insert waitlist entry: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to add to waitlist"})),
            )
        })?;

    tracing::info!("Added {} to waitlist", email);

    Ok(Json(json!({
        "message": "Successfully added to waitlist",
        "email": email
    })))
}

/// Validate a password reset token.
///
/// Returns success if the token exists and hasn't expired,
/// without consuming the token.
pub async fn validate_reset_token(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(token): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Look up the token
    let token_data = state.pending_password_resets.get(&token);

    match token_data {
        Some(entry) => {
            let (_, expiry) = *entry;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            if now > expiry {
                // Token has expired - remove it
                drop(entry); // Release the lock before removing
                state.pending_password_resets.remove(&token);
                return Err((
                    StatusCode::GONE,
                    Json(json!({"error": "Reset link has expired. Please request a new one."})),
                ));
            }

            // Token is valid
            Ok(Json(json!({
                "valid": true,
                "message": "Token is valid"
            })))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Invalid or expired reset link."})),
        )),
    }
}

/// Complete password reset using a valid token.
///
/// Sets the new password and consumes the token (one-time use).
pub async fn complete_password_reset(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CompletePasswordResetRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Validate password length
    if req.new_password.len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Password must be at least 8 characters long"})),
        ));
    }

    // Look up and remove the token (consume it)
    let token_data = state.pending_password_resets.remove(&req.token);

    match token_data {
        Some((_, (user_id, expiry))) => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            if now > expiry {
                return Err((
                    StatusCode::GONE,
                    Json(json!({"error": "Reset link has expired. Please request a new one."})),
                ));
            }

            // Hash the new password
            let password_hash =
                bcrypt::hash(&req.new_password, bcrypt::DEFAULT_COST).map_err(|e| {
                    tracing::error!("Failed to hash password: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": "Failed to process password"})),
                    )
                })?;

            // Update the user's password
            state
                .user_core
                .update_password(user_id, &password_hash)
                .map_err(|e| {
                    tracing::error!("Failed to update password for user {}: {}", user_id, e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": "Failed to update password"})),
                    )
                })?;

            tracing::info!("Password reset completed for user {}", user_id);

            Ok(Json(json!({
                "message": "Password has been reset successfully. You can now log in with your new password."
            })))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Invalid or expired reset link."})),
        )),
    }
}
