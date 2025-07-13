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
use crate::api::twilio_sms;
use serde_json::json;
use jsonwebtoken::{encode, Header, EncodingKey};
use chrono::{Duration, Utc};
use serde::Deserialize;
use std::num::NonZeroU32;
use governor::{Quota, RateLimiter};
use rand::distributions::Alphanumeric;
use uuid::Uuid;
use std::env;

#[derive(Deserialize)]
pub struct BroadcastMessageRequest {
    message: String,
}

#[derive(Serialize, Deserialize)]
pub struct PairingVerificationRequest {
    pairing_code: String,
    server_instance_id: String,
}

#[derive(Deserialize, Serialize)]
pub struct PairingVerificationResponse {
    valid: bool,
    number: String,
    message: String,
}

#[derive(Deserialize)]
pub struct SelfHostedSignupRequest {
    pairing_code: String,
    password: Option<String>,
}

#[derive(Deserialize)]
pub struct SelfHostedLoginRequest {
    password: String,
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

#[derive(Serialize)]
pub struct GeneratePairingCodeResponse {
    pairing_code: String,
}

#[derive(Serialize)]
pub struct SelfHostedStatusResponse {
    status: String,
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
        Ok(valid) => {
            println!("Password verification result: {}", valid);
            if valid {
                println!("Password verified successfully, generating tokens");
                // Generate access token (short-lived)
                let access_token = encode(
                    &Header::default(),
                    &json!({
                        "sub": user.id,
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
                println!("Access token generated successfully");

                // Generate refresh token (long-lived)
                let refresh_token = encode(
                    &Header::default(),
                    &json!({
                        "sub": user.id,
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
                println!("Refresh token generated successfully");
                
                // Create response with HttpOnly cookies
                let mut response = Response::new(
                    axum::body::Body::from(
                        Json(json!({"message": "Login successful", "token": access_token})).to_string()
                    )
                );
                println!("Created base response");
                
                let cookie_options = "; HttpOnly; Secure; SameSite=Strict; Path=/";
                response.headers_mut().insert(
                    "Set-Cookie",
                    format!("access_token={}{}; Max-Age=900", access_token, cookie_options)
                        .parse()
                        .unwrap(),
                );
                println!("Added access token cookie");
                
                response.headers_mut().insert(
                    "Set-Cookie",
                    format!("refresh_token={}{}; Max-Age=604800", refresh_token, cookie_options)
                        .parse()
                        .unwrap(),
                );
                println!("Added refresh token cookie");
                
                // Set content type header
                response.headers_mut().insert(
                    "Content-Type",
                    "application/json".parse().unwrap()
                );
                println!("Added content type header");

                println!("Returning successful response");
                Ok(response)
            } else {
                println!("Password verification failed");
                Err((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "Invalid credentials"}))
                ))
            }
        }
        Err(err) => {
            println!("Password verification error occurred: {:?}", err);
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

    // Get or create a conversation for sending the OTP
    let conversation = match state.user_conversations.get_conversation(&state, &user, user.preferred_number.clone().unwrap_or_else(|| {
        std::env::var("FIN_PHONE").expect("FIN_PHONE must be set")
    })).await {
        Ok(conv) => conv,
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to create conversation for OTP"}))
            ));
        }
    };

    // Send OTP via conversation message
    let message = format!("Your Lightfriend password reset code is: {}. Valid for 5 minutes.", otp);
    if let Err(_) = crate::api::twilio_utils::send_conversation_message(
        &state,
        &conversation.conversation_sid,
        &conversation.twilio_number,
        &message,
        false, // Don't redact OTP messages
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

    // Set preferred number
    state.user_core.set_preferred_number_to_default(user.id, &reg_req.phone_number)
        .map_err(|e| {
            println!("Failed to set preferred number: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to set preferred number") })),
            )
        })?;

    println!("Preferred number set successfully, generating tokens");
    let user = state.user_core.find_by_email(&reg_req.email)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to retrieve user")}))
        ))?
        .ok_or_else(|| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "User not found after registration"}))
        ))?;

    // Generate access token (short-lived)
    let access_token = encode(
        &Header::default(),
        &json!({
            "sub": user.id,
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
            "sub": user.id,
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
            Json(json!({
                "message": "User registered and logged in successfully! Redirecting...",
                "token": access_token
            })).to_string()
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

    println!("Registration and login completed successfully");
    Ok(response)
}

pub async fn self_hosted_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SelfHostedStatusResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Check if environment is self-hosted
    let env_type = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "normal".to_string());
    
    if env_type == "self_hosted" {
        // Check if there are any users in the database
        match state.user_core.get_all_users() {
            Ok(users) => {
                let status = if users.is_empty() {
                    "self-hosted-signup"
                } else {
                    "self-hosted-login"
                };
                println!("status: {}", status);
                Ok(Json(SelfHostedStatusResponse { status: status.to_string() }))
            }
            Err(e) => {
                println!("Database error while checking users: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Database error"}))
                ))
            }
        }
    } else {
        println!("status: normal");
        Ok(Json(SelfHostedStatusResponse {
            status: "normal".to_string()
        }))
    }
}

pub async fn self_hosted_signup(
    State(state): State<Arc<AppState>>,
    Json(signup_req): Json<SelfHostedSignupRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Verify we're in self-hosted mode
    let env_type = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "normal".to_string());
    if env_type != "self_hosted" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Self-hosted signup is only available in self-hosted mode"}))
        ));
    }

    // Check if any users exist
    let users = state.user_core.get_all_users().map_err(|e| {
        println!("Database error while checking users: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database error"}))
        )
    })?;

    if !users.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Self-hosted instance is already set up"}))
        ));
    }

    // If password is provided, this is the second step (password creation)
    if let Some(password) = signup_req.password {

        // Hash the password
        let password_hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST).map_err(|e| {
            println!("Password hashing failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Password hashing failed"}))
            )
        })?;

        let user = state.user_core.find_by_email("admin@local")
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to retrieve user"}))
            ))?
            .ok_or_else(|| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "User not found after creation"}))
            ))?;

        if user.password_hash == "".to_string() {
            // Update the password since it's empty
            state.user_core.update_password(user.email.as_str(), password_hash.as_str()).map_err(|e| {
                println!("Failed to set password for the self hosted: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to set password for the self hosted instance"}))
                )
            })?;
        } else {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Password can only be set once"}))
            ));
        }

        // Generate tokens
        let access_token = encode(
            &Header::default(),
            &json!({
                "sub": user.id,
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

        let refresh_token = encode(
            &Header::default(),
            &json!({
                "sub": user.id,
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

        // Create response with tokens
        let mut response = Response::new(
            axum::body::Body::from(
                Json(json!({
                    "message": "Self-hosted setup complete",
                    "token": access_token
                })).to_string()
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
    } else {
        // This is the first step (pairing code verification)
        // Generate a server instance ID
        let server_instance_id = Uuid::new_v4().to_string();

        // Send verification request to main server
        let server_url = env::var("SERVER_URL").expect("SERVER_URL must be set");
        let client = reqwest::Client::new();
        let verification_response = client
            .post(&format!("{}/api/check-pairing", server_url))
            .json(&PairingVerificationRequest {
                pairing_code: signup_req.pairing_code.clone(),
                server_instance_id: server_instance_id.clone(),
            })
            .send()
            .await
            .map_err(|e| {
                println!("Failed to send verification request: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to verify pairing code"}))
                )
            })?;

        if !verification_response.status().is_success() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid pairing code"}))
            ));
        }

        let verification_result = verification_response.json::<PairingVerificationResponse>().await
            .map_err(|e| {
                println!("Failed to parse verification response: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to verify pairing code"}))
                )
            })?;

        if !verification_result.valid {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": verification_result.message}))
            ));
        }

        // Create the admin user
        let new_user = NewUser {
            email: "admin@local".to_string(),
            password_hash: "".to_string(),
            phone_number: verification_result.number,
            time_to_live: 0, 
            verified: true,
            credits: 0.00,
            credits_left: 0.00,
            charge_when_under: false,
            waiting_checks_count: 0,
            discount: false,
            sub_tier: Some("self-hosted".to_string()),
        };

        state.user_core.create_user(new_user).map_err(|e| {
            println!("User creation failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to create admin user"}))
            )
        })?;

        state.user_core.update_instance_id_to_self_hosted(server_instance_id.as_str()).map_err(|e| {
            println!("Failed to set profile for the self hosted: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to set server instance ID"}))
            )
        })?;

        Ok(Response::new(
            axum::body::Body::from(
                Json(json!({
                    "message": "Pairing code verified. Please create a password."
                })).to_string()
            )
        ))
    }
}

pub async fn check_pairing(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PairingVerificationRequest>,
) -> Result<Json<PairingVerificationResponse>, (StatusCode, Json<serde_json::Value>)> {
    println!("Attempting to verify pairing code for server instance: {}", req.server_instance_id);

    // Verify the pairing code and update server instance ID
    match state.user_core.verify_pairing_code(&req.pairing_code, &req.server_instance_id) {
        Ok((true, number)) => {
            println!("Pairing code verified successfully for server instance: {}", req.server_instance_id);
            Ok(Json(PairingVerificationResponse {
                valid: true,
                number: number.unwrap_or("".to_string()),
                message: "Pairing code verified successfully".to_string(),
            }))
        },
        Ok((false, number)) => {
            println!("Invalid pairing code provided for server instance: {}", req.server_instance_id);
            Ok(Json(PairingVerificationResponse {
                valid: false,
                number: number.unwrap_or("".to_string()),
                message: "Invalid pairing code".to_string(),
            }))
        },
        Err(e) => {
            println!("Database error while verifying pairing code: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to verify pairing code due to database error"}))
            ))
        }
    }
}

// generates a new pairing code for the user. normally this is done on stripe subscription webhook.
pub async fn generate_pairing_code(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser, 
) -> Result<Json<GeneratePairingCodeResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Generate a random 6-character pairing code
    let pairing_code: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect::<String>()
        .to_uppercase();

    // Save the new pairing code 
    if let Err(e) = state.user_core.set_server_instance_id(auth_user.user_id, &pairing_code.as_str()) {
        println!("Failed to save pairing code: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to generate pairing code"}))
        ));
    }

    Ok(Json(GeneratePairingCodeResponse {
        pairing_code
    }))
}

pub async fn self_hosted_login(
    State(state): State<Arc<AppState>>,
    Json(login_req): Json<SelfHostedLoginRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Verify we're in self-hosted mode
    let env_type = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "normal".to_string());
    if env_type != "self_hosted" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Self-hosted login is only available in self-hosted mode"}))
        ));
    }

    // Find the admin user
    let user = match state.user_core.find_by_email("admin@local") {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Self-hosted instance not set up"}))
            ));
        }
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"}))
            ));
        }
    };

    // Verify password
    match bcrypt::verify(&login_req.password, &user.password_hash) {
        Ok(valid) => {
            if !valid {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "Invalid password"}))
                ));
            }
        }
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Password verification failed"}))
            ));
        }
    }

    // Generate tokens
    let access_token = encode(
        &Header::default(),
        &json!({
            "sub": user.id,
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

    let refresh_token = encode(
        &Header::default(),
        &json!({
            "sub": user.id,
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

    // Create response with tokens
    let mut response = Response::new(
        axum::body::Body::from(
            Json(json!({
                "message": "Login successful",
                "token": access_token
            })).to_string()
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

