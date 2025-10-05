use crate::AppState;
use std::sync::Arc;
use crate::handlers::auth_dtos::NewUser;
use crate::handlers::auth_middleware::AuthUser;
use axum::{
    Json,
    extract::State,
    response::Response,
    http::StatusCode,
};

use rand::Rng;
use serde_json::json;
use jsonwebtoken::{encode, Header, EncodingKey};
use chrono::{Duration, Utc};
use std::num::NonZeroU32;
use rand::distributions::Alphanumeric;
use std::env;
use serde::{Deserialize, Serialize};
use crate::handlers::auth_handlers::generate_tokens_and_response;

#[derive(Deserialize)]
pub struct SelfHostPingRequest {
    instance_id: String,
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

#[derive(Serialize)]
pub struct GeneratePairingCodeResponse {
    pairing_code: String,
}

#[derive(Serialize)]
pub struct SelfHostedStatusResponse {
    status: String,
}


#[derive(Deserialize)]
pub struct UpdateServerIpRequest {
    server_ip: String,
}

#[derive(Deserialize)]
pub struct UpdateTwilioPhoneRequest {
    twilio_phone: String,
}

#[derive(Deserialize)]
pub struct UpdateTwilioCredsRequest {
    account_sid: String,
    auth_token: String,
}

#[derive(Deserialize)]
pub struct UpdateTextBeeCredsRequest {
    textbee_api_key: String,
    textbee_device_id: String,
}

pub async fn update_twilio_phone(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<UpdateTwilioPhoneRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    match state.user_core.update_preferred_number(&req.twilio_phone) {
        Ok(_) => {
            tracing::debug!("Successfully updated Twilio phone for user: {}", auth_user.user_id);

            if let Ok((account_sid, auth_token, _)) = state.user_core.get_twilio_credentials() {
                let phone = req.twilio_phone.clone();
                let user_id = auth_user.user_id;
                let state_clone = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = crate::api::twilio_utils::set_twilio_webhook(&account_sid, &auth_token, &phone, user_id, state_clone).await {
                        tracing::error!("Failed to set Twilio webhook for phone {}: {}", phone, e);
                        // Proceed anyway(probably user hasn't given their twilio credentials yet, we will try again when they do)
                    } else {
                        tracing::debug!("Successfully set Twilio webhook for phone: {}", phone);
                    }
                });
            } else {
                tracing::warn!("Twilio credentials not found for user {}, skipping webhook update", auth_user.user_id);
            }

            Ok(StatusCode::OK)
        },
        Err(e) => {
            tracing::error!("Failed to update Twilio phone: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update Twilio phone"}))
            ))
        }
    }
}

pub async fn update_twilio_creds(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<UpdateTwilioCredsRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let user_opt = match state.user_core.get_user() {
        Ok(opt) => opt,
        Err(e) => {
            tracing::error!("Failed to fetch user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to fetch user"}))
            ));
        }
    };

    let user = match user_opt {
        Some(u) => u,
        None => {
            tracing::error!("User not found: {}", auth_user.user_id);
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"}))
            ));
        }
    };

    match state.user_core.update_twilio_credentials(&req.account_sid, &req.auth_token) {
        Ok(_) => {
            tracing::debug!("Successfully updated Twilio credentials for user: {}", auth_user.user_id);

            if let Some(phone) = user.preferred_number {
                let account_sid = req.account_sid.clone();
                let auth_token = req.auth_token.clone();
                let phone = phone.clone();
                let user_id = auth_user.user_id;
                let state_clone = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = crate::api::twilio_utils::set_twilio_webhook(&account_sid, &auth_token, &phone, user_id, state_clone).await {
                        tracing::error!("Failed to set Twilio webhook for phone {}: {}", phone, e);
                        // Proceed anyway(probably user hasn't inputted their twilio number yet, we try again when they do)
                    } else {
                        tracing::debug!("Successfully set Twilio webhook for phone: {}", phone);
                    }
                });
            }

            Ok(StatusCode::OK)
        },
        Err(e) => {
            tracing::error!("Failed to update Twilio credentials: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update Twilio credentials"}))
            ))
        }
    }
}

pub async fn update_textbee_creds(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<UpdateTextBeeCredsRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    match state.user_core.update_textbee_credentials(auth_user.user_id, &req.textbee_device_id, &req.textbee_api_key) {
        Ok(_) => {
            println!("Successfully updated TextBee credentials for user: {}", auth_user.user_id);
            Ok(StatusCode::OK)
        },
        Err(e) => {
            tracing::error!("Failed to update TextBee credentials: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update TextBee credentials"}))
            ))
        }
    }
}

use reqwest::Client;
use crate::handlers::auth_dtos::LoginRequest;
// Add this struct for the check-creds response
#[derive(Deserialize, Serialize)]
struct CheckCredsResponse {
    user_id: String,
    phone_number: String,
    preferred_number: String,
    phone_number_country: String,
    messaging_service_sid: Option<String>,
    twilio_account_sid: Option<String>,
    twilio_auth_token: Option<String>,
}

pub async fn self_hosted_login(
    State(state): State<Arc<AppState>>,
    Json(login_req): Json<LoginRequest>,
) -> Result<Response, (axum::http::StatusCode, Json<serde_json::Value>)> {
    println!("Self-hosted login attempt for email: {}", login_req.email); // Debug log
    // Define rate limit: 5 attempts per minute
    // Verify credentials against main server
    let client = Client::new();
    let check_resp = client
        .post("https://lightfriend.ai/api/profile/check-creds")
        .json(&login_req)
        .send()
        .await
        .map_err(|_| (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Verification service unavailable"}))
        ))?;
    if !check_resp.status().is_success() {
        println!("Credential check failed for email: [redacted]");
        return Err((
            axum::http::StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid credentials"}))
        ));
    }
    let check_data: CheckCredsResponse = check_resp
        .json()
        .await
        .map_err(|_| (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to process verification response"}))
        ))?;
    let main_phone_number = check_data.phone_number;
    println!("Credentials verified successfully for main user_id: {}", check_data.user_id);
    let user = match state.user_core.get_user() {
        Ok(Some(mut u)) => u,
        Ok(None) => {
            // Create new user
            let new_user = NewUser {
                phone_number: main_phone_number.clone(),
                credits: 0.00,
                credits_left: 0.00,
            };
            state.user_core.create_user(new_user).map_err(|e| {
                println!("User creation failed: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "User creation failed"})),
                )
            })?;
            println!("New self-hosted user created successfully");
            // Get the newly created user
            state.user_core.get_user()
                .map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to retrieve user: {}", e)}))
                ))?
                .ok_or_else(|| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "User not found after creation"}))
                ))?
        }
        Err(e) => {
            println!("Database error while checking for user id 1: {}", e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"}))
            ));
        }
    };

    // Set phone country
    if let Err(e) = crate::handlers::profile_handlers::set_user_phone_country(&state, &user.phone_number).await {
        tracing::error!("Failed to set phone country during self-hosted login: {}", e);
        // Proceed anyway
    }
    generate_tokens_and_response(user.id)
}


fn generate_api_key(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}
