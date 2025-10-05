use std::sync::Arc;
use crate::handlers::auth_middleware::AuthUser;
use axum::{
    Json,
    extract::State,
    response::Response,
    http::StatusCode,
};
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::json;
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};
use chrono::{Duration, Utc};
use serde::Deserialize;
use std::num::NonZeroU32;
use governor::{Quota, RateLimiter};
use std::env;

#[derive(Deserialize)]
pub struct BroadcastMessageRequest {
    message: String,
}

use crate::{
    handlers::auth_dtos::{LoginRequest, RegisterRequest, UserResponse, NewUser},
    AppState
};

#[derive(Deserialize)]
pub struct ErrorResponse {
    error: String,
}

#[derive(Deserialize)]
pub struct PasswordResetRequest {
    email: String,
}

#[derive(Deserialize)]
pub struct VerifyPasswordResetRequest {
    email: String,
    otp: String,
    new_password: String,
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
    println!("Attempting to get all users");
    let users_list = state.user_core.get_all_users().map_err(|e| {
        tracing::error!("Database error while fetching users: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database error"}))
        )
    })?;
    
    println!("Converting users to response format");
    let mut users_response = Vec::with_capacity(users_list.len());
    
    for user in users_list {
        // Get user settings, providing defaults if not found
        let settings = state.user_core.get_user_settings(user.id).map_err(|e| {
            tracing::error!("Database error while fetching user settings: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"}))
            )
        })?;

        users_response.push(UserResponse {
            id: user.id,
            email: user.email,
            phone_number: user.phone_number,
            nickname: user.nickname,
            time_to_live: user.time_to_live,
            verified: user.verified,
            credits: user.credits,
            notify: settings.notify,
            preferred_number: user.preferred_number,
            sub_tier: user.sub_tier,
            credits_left: user.credits_left,
            discount: user.discount,
            discount_tier: user.discount_tier,
        });
    }

    println!("Successfully retrieved {} users", users_response.len());
    Ok(Json(users_response))
}


pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(login_req): Json<LoginRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    println!("Login attempt for email: {}", login_req.email); // Debug log

    // Define rate limit: 5 attempts per minute
    let quota = Quota::per_minute(NonZeroU32::new(5).unwrap());
    let limiter_key = login_req.email.clone(); // Use email as the key

    // Get or create a keyed rate limiter for this email
    let entry = state.login_limiter
        .entry(limiter_key.clone())
        .or_insert_with(|| RateLimiter::keyed(quota)); // Bind the Entry here
    let limiter = entry.value(); // Now borrow from the bound value

    // Check if rate limit is exceeded
    if limiter.check_key(&limiter_key).is_err() {
        println!("Rate limit exceeded for email: [redacted]");
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Too many login attempts, try again later"})),
        ));
    }

    let user = match state.user_core.find_by_email(&login_req.email) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "User not found"}))
            ));
        }
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"}))
            ));
        }
    };
   
    match bcrypt::verify(&login_req.password, &user.password_hash) {
        Ok(true) => {
            generate_tokens_and_response(user.id)
        }
        _ => {
            Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid credentials"}))
            ))
        }
    }
}


pub async fn request_password_reset(
    State(state): State<Arc<AppState>>,
    Json(reset_req): Json<PasswordResetRequest>,
) -> Result<Json<PasswordResetResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Define rate limit: 3 attempts per hour per email
    let quota = Quota::per_hour(NonZeroU32::new(3).unwrap());
    let limiter_key = reset_req.email.clone();

    // Get or create a rate limiter for this email
    let entry = state.password_reset_limiter
        .entry(limiter_key.clone())
        .or_insert_with(|| RateLimiter::keyed(quota));
    let limiter = entry.value();

    // Check if rate limit is exceeded
    if limiter.check_key(&limiter_key).is_err() {
        println!("Rate limit exceeded for password reset request: [redacted email]");
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Too many password reset attempts. Please try again later."}))
        ));
    }
    // Find user by email
    let user = match state.user_core.find_by_email(&reset_req.email) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"}))
            ));
        }
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"}))
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
        .as_secs() + 300; // 5 minutes

    // Remove any existing OTP for this email first
    state.password_reset_otps.remove(&reset_req.email);

    // Insert the new OTP
    state.password_reset_otps.insert(
        reset_req.email.clone(),
        (otp.clone(), expiration)
    );

    println!("Stored OTP {} for email {} with expiration {}", otp, reset_req.email, expiration);

    let message = format!("Your Lightfriend password reset code is: {}. Valid for 5 minutes.", otp);
    if let Err(_) = crate::api::twilio_utils::send_conversation_message(
        &state,
        &message,
        None,
        &user
    ).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to send OTP"}))
        ));
    }

    Ok(Json(PasswordResetResponse {
        message: "Password reset code sent to your phone".to_string()
    }))
}

pub async fn verify_password_reset(
    State(state): State<Arc<AppState>>,
    Json(verify_req): Json<VerifyPasswordResetRequest>,
) -> Result<Json<PasswordResetResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Define rate limit: 3 attempts per 60 minutes per email
    let quota = Quota::with_period(std::time::Duration::from_secs(60 * 60))
        .unwrap()
        .allow_burst(NonZeroU32::new(3).unwrap());
    let limiter_key = verify_req.email.clone();

    // Get or create a rate limiter for this email
    let entry = state.password_reset_verify_limiter
        .entry(limiter_key.clone())
        .or_insert_with(|| RateLimiter::keyed(quota));
    let limiter = entry.value();

    // Check if rate limit is exceeded
    if limiter.check_key(&limiter_key).is_err() {
        println!("Rate limit exceeded for password reset verification: [redacted email]");
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Too many verification attempts. Please try again later."}))
        ));
    }
    println!("Verifying OTP {} for email {}", verify_req.otp, verify_req.email);
    
    // Remove the OTP data immediately to prevent any hanging references
    let otp_data = match state.password_reset_otps.remove(&verify_req.email) {
        Some((_, data)) => data,  // The first element is the key (email), second is the value tuple
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "No valid OTP found for this email"}))
            ));
        }
    };

    let (stored_otp, expiration_time) = otp_data;

    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if current_time > expiration_time {
        println!("OTP expired: current_time {} > expiration {}", current_time, expiration_time);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "OTP has expired"}))
        ));
    }

    if verify_req.otp != stored_otp {
        println!("OTP mismatch: provided {} != stored {}", verify_req.otp, stored_otp);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid OTP"}))
        ));
    }

    // Hash new password
    let password_hash = bcrypt::hash(&verify_req.new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| {
            println!("Password hashing failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Password hashing failed"}))
            )
        })?;

    // Update password in database
    if let Err(e) = state.user_core.update_password(&verify_req.email, &password_hash) {
        println!("Failed to update password: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to update password"}))
        ));
    }
    println!("New password updated successfully");

    // Also remove any rate limiting for this email
    state.login_limiter.remove(&verify_req.email);
    
    println!("Password reset completed successfully, sending response");
    
    // Create success response with explicit status code
    let response = PasswordResetResponse {
        message: "Password has been reset successfully. You can now log in with your new password.".to_string()
    };
    
    Ok(Json(response))
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
    let entry = state.phone_verify_limiter
        .entry(limiter_key.clone())
        .or_insert_with(|| RateLimiter::keyed(quota));
    let limiter = entry.value();
    // Check if rate limit is exceeded
    if limiter.check_key(&limiter_key).is_err() {
        println!("Rate limit exceeded for phone verify request: [redacted phone]");
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Too many verification attempts. Please try again later."}))
        ));
    }
    // Find user by phone_number
    let user = match state.user_core.find_by_phone_number(&reset_req.phone_number) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "No user found with this phone number"}))
            ));
        }
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"}))
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
        .as_secs() + 300; // 5 minutes
    // Remove any existing OTP for this phone_number first
    state.phone_verify_otps.remove(&reset_req.phone_number);
    // Insert the new OTP
    state.phone_verify_otps.insert(
        reset_req.phone_number.clone(),
        (otp.clone(), expiration)
    );
    println!("Stored OTP {} for phone {} with expiration {}", otp, reset_req.phone_number, expiration);
    let message = format!("Your Lightfriend verification code is: {}. Valid for 5 minutes.", otp);
    if let Err(_) = crate::api::twilio_utils::send_conversation_message(
        &state,
        &message,
        None,
        &user
    ).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to send OTP"}))
        ));
    }
    Ok(Json(PasswordResetResponse {
        message: "Verification code sent to your phone".to_string()
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
    let entry = state.phone_verify_verify_limiter
        .entry(limiter_key.clone())
        .or_insert_with(|| RateLimiter::keyed(quota));
    let limiter = entry.value();
    // Check if rate limit is exceeded
    if limiter.check_key(&limiter_key).is_err() {
        println!("Rate limit exceeded for phone verify verification: [redacted phone]");
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Too many verification attempts. Please try again later."}))
        ));
    }
    println!("Verifying OTP {} for phone {}", verify_req.otp, verify_req.phone_number);
   
    // Remove the OTP data immediately to prevent any hanging references
    let otp_data = match state.phone_verify_otps.remove(&verify_req.phone_number) {
        Some((_, data)) => data, // The first element is the key (phone), second is the value tuple
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "No valid OTP found for this phone number"}))
            ));
        }
    };
    let (stored_otp, expiration_time) = otp_data;
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    if current_time > expiration_time {
        println!("OTP expired: current_time {} > expiration {}", current_time, expiration_time);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "OTP has expired"}))
        ));
    }
    if verify_req.otp != stored_otp {
        println!("OTP mismatch: provided {} != stored {}", verify_req.otp, stored_otp);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid OTP"}))
        ));
    }
    // Find user by phone_number to verify
    let user = match state.user_core.find_by_phone_number(&verify_req.phone_number) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "No user found with this phone number"}))
            ));
        }
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"}))
            ));
        }
    };
    // Verify the user
    if let Err(e) = state.user_core.verify_user(user.id) {
        tracing::error!("Error verifying user: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to verify user"}))
        ));
    }
    println!("User verified successfully");
   
    // Create success response
    let response = PasswordResetResponse {
        message: "Phone number has been verified successfully.".to_string()
    };
   
    Ok(Json(response))
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(reg_req): Json<RegisterRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
   
    println!("Registration attempt for email: {}", reg_req.email);
    use regex::Regex;
    let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    if !email_regex.is_match(&reg_req.email) {
        println!("Invalid email format: {}", reg_req.email);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid email format"}))
        ));
    }
    // Check if email exists
    println!("Checking if email exists...");
    if state.user_core.email_exists(&reg_req.email).map_err(|e| {
        println!("Database error while checking email: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error") }))
        )
    })? {
        println!("Email {} already exists", reg_req.email);
        return Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": "Email already exists" })),
        ));
    }
    println!("Email is available");
    let phone_regex = Regex::new(r"^\+[1-9]\d{1,14}$").unwrap();
    if !phone_regex.is_match(&reg_req.phone_number) {
        println!("Invalid phone number format: {}", reg_req.phone_number);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Phone number must be in E.164 format (e.g., +1234567890)"}))
        ));
    }
    if reg_req.password.len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Password must be 8+ characters" })),
        ));
    }
    // Check if phone number exists
    println!("Checking if phone number exists...");
    if state.user_core.phone_number_exists(&reg_req.phone_number).map_err(|e| {
        println!("Database error while checking phone number: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error") }))
        )
    })? {
        println!("Phone number {} already exists", reg_req.phone_number);
        return Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": "Phone number already registered" })),
        ));
    }
    println!("Phone number is available");
    // Hash password
    println!("Hashing password...");
    let password_hash = bcrypt::hash(&reg_req.password, bcrypt::DEFAULT_COST)
        .map_err(|e| {
            println!("Password hashing failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Password hashing failed") })),
            )
        })?;
    println!("Password hashed successfully");
    // Create and insert user
    println!("Creating new user...");
    // Calculate timestamp 5 minutes from now
    let five_minutes_from_now = Utc::now()
        .checked_add_signed(Duration::minutes(5))
        .expect("Failed to calculate timestamp")
        .timestamp() as i32;
    println!("Set the time to live due in 5 minutes");
    let reg_r = reg_req.clone();
    let new_user = NewUser {
        email: reg_r.email,
        password_hash,
        phone_number: reg_r.phone_number,
        time_to_live: five_minutes_from_now,
        verified: false,
        credits: 0.00,
        credits_left: 0.00,
        charge_when_under: false,
        waiting_checks_count: 0,
        discount: false,
        sub_tier: None,
    };
    state.user_core.create_user(new_user).map_err(|e| {
        println!("User creation failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("User creation failed") })),
        )
    })?;
    println!("User registered successfully, setting preferred number");
   
    // Get the newly created user to get their ID
    let user = state.user_core.find_by_email(&reg_req.email)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to retrieve user")}))
        ))?
        .ok_or_else(|| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "User not found after registration"}))
        ))?;
    // Set phone number country
    if let Err(e) = crate::handlers::profile_handlers::set_user_phone_country(&state, user.id, &reg_req.phone_number).await {
        tracing::error!("Failed to set phone country during registration: {}", e);
        // Continue without failing registration
    }
    // Set preferred number if user has US number
    if reg_req.phone_number.starts_with("+1") {
        state.user_core.set_preferred_number_to_us_default(user.id)
        .map_err(|e| {
            println!("Failed to set preferred number: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to set preferred number") })),
            )
        })?;
        println!("Preferred number set successfully, generating tokens");
    }
    generate_tokens_and_response(user.id)
}

pub async fn refresh_token(
    State(state): State<Arc<AppState>>,
    headers: reqwest::header::HeaderMap,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let refresh_token = match headers.get("cookie") {
        Some(cookie_header) => {
            let cookies = cookie_header.to_str().unwrap_or("");
            cookies.split(';').find(|c| c.trim().starts_with("refresh_token="))
                .and_then(|c| c.split('=').nth(1))
                .map(|t| t.to_string())
                .ok_or((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "Missing refresh token"}))
                ))?
        }
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing cookies"}))
            ));
        }
    };

    // Validate refresh token
    let validation = Validation::default();
    let token_data = decode::<serde_json::Value>(
        &refresh_token,
        &DecodingKey::from_secret(env::var("JWT_REFRESH_KEY").expect("JWT_REFRESH_KEY must be set").as_ref()),
        &validation,
    ).map_err(|_| (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": "Invalid refresh token"}))
    ))?;

    let user_id: i32 = token_data.claims["sub"].as_i64().unwrap_or(0) as i32;
    if user_id == 0 {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid user in token"}))
        ));
    }

    // Optional: Rotate refresh token by generating a new one
    generate_tokens_and_response(user_id)
}

pub async fn testing_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(params): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Testing route called by user ID: {}", auth_user.user_id);
    println!("Received params: {:?}", params);

    let location = "Vuores, Tampere, Finland";

    match crate::utils::tool_exec::get_nearby_towns(location).await {
        Ok(towns) => {
            println!("Nearby towns: {:?}", towns);
            println!("Location: {}", location);
            Ok(Json(json!({"message": "Test successful"})))
        }
        Err(e) => {
            println!("Error in get_nearby_towns: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get nearby towns: {}", e)}))
            ))
        }
    }
}

pub fn generate_tokens_and_response(user_id: i32) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Generate access token (short-lived)
    let access_token = encode(
        &Header::default(),
        &json!({
            "sub": user_id,
            "exp": (Utc::now() + Duration::minutes(15)).timestamp(),
            "type": "access"
        }),
        &EncodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET_KEY must be set in environment")
            .as_bytes()),
    ).map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "Token generation failed"}))
    ))?;

    // Generate refresh token (long-lived)
    let refresh_token = encode(
        &Header::default(),
        &json!({
            "sub": user_id,
            "exp": (Utc::now() + Duration::days(7)).timestamp(),
            "type": "refresh"
        }),
        &EncodingKey::from_secret(std::env::var("JWT_REFRESH_KEY")
            .expect("JWT_REFRESH_KEY must be set in environment")
            .as_bytes()),
    ).map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "Token generation failed"}))
    ))?;

    // Create response with HttpOnly cookies
    let mut response = Response::new(
        axum::body::Body::from(
            Json(json!({"message": "Tokens generated", "token": access_token.clone()})).to_string()
        )
    );
    let cookie_options = "; HttpOnly; Secure; SameSite=Strict; Path=/";
    response.headers_mut().insert(
        "Set-Cookie",
        format!("access_token={}{}; Max-Age=900", access_token, cookie_options)
            .parse()
            .unwrap(),
    );
    response.headers_mut().insert(
        "Set-Cookie",
        format!("refresh_token={}{}; Max-Age=604800", refresh_token, cookie_options)
            .parse()
            .unwrap(),
    );
    response.headers_mut().insert(
        "Content-Type",
        "application/json".parse().unwrap()
    );
    Ok(response)
}
