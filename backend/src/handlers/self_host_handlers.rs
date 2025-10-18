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

use serde_json::json;
use serde::{Deserialize, Serialize};
use crate::handlers::auth_handlers::generate_tokens_and_response;

#[derive(Deserialize)]
pub struct UpdateTwilioPhoneRequest {
    twilio_phone: String,
}

#[derive(Deserialize)]
pub struct UpdateTwilioCredsRequest {
    account_sid: String,
    auth_token: String,
    server_url: Option<String>,
    messaging_service_sid: Option<String>,
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

            if let Ok((Some(account_sid), Some(auth_token), _, _)) = state.user_core.get_twilio_credentials() {
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
    let user = match state.user_core.get_user() {
        Ok(Some(opt)) => {
            opt
        },
        Ok(None) => {
            tracing::error!("User not found: {}", auth_user.user_id);
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"}))
            ));
        },
        Err(e) => {
            tracing::error!("Failed to fetch user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to fetch user"}))
            ));
        }
    };

    match state.user_core.update_twilio_credentials(Some(req.account_sid.as_str()), Some(req.auth_token.as_str()), req.server_url.as_deref(), req.messaging_service_sid.as_deref()) {
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
use axum::Json as AxumJson;
use tracing;

// Reuse or define this for the verify-token response (same as check-creds)
#[derive(Deserialize, Serialize)]
struct VerifyTokenResponse {
    user_id: String,
    preferred_number: String,
    phone_number_country: String,
    messaging_service_sid: Option<String>,
    twilio_account_sid: Option<String>,
    twilio_auth_token: Option<String>,
    server_url: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct TokenRequest {
    token: String,
}

pub async fn self_hosted_login(
    State(state): State<Arc<AppState>>,
    Json(token_req): Json<TokenRequest>,
) -> Result<Response, (StatusCode, AxumJson<serde_json::Value>)> {
    println!("Self-hosted token login attempt"); // No email to log for privacy

    let mut url = "https://lightfriend.ai/api/self-hosted/verify-token"; 
    if std::env::var("ENVIRONMENT").expect("ENVIRONMENT not set") == "development" {
        url = "http://localhost:3000/api/self-hosted/verify-token"; 
    }

    // Verify token against main server
    let client = Client::new();
    let verify_resp = client
        .post(url) // Assume this endpoint exists on main server
        .json(&token_req)
        .send()
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Verification service unavailable"}))
        ))?;

    if !verify_resp.status().is_success() {
        println!("Token verification failed");
        return Err((
            StatusCode::UNAUTHORIZED,
            AxumJson(json!({"error": "Invalid or expired login link"}))
        ));
    }

    let verify_data: VerifyTokenResponse = verify_resp
        .json()
        .await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Failed to process verification response"}))
        ))?;

    let main_phone_number = verify_data.preferred_number.clone();
    println!("Token verified successfully for main user_id: {}", verify_data.user_id);

    let mut is_new_user = false;
    let user = match state.user_core.get_user() {
        Ok(Some(u)) => u,
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
                    AxumJson(json!({"error": "User creation failed"})),
                )
            })?;
            println!("New self-hosted user created successfully");
            // Get the newly created user
            state.user_core.get_user()
                .map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": format!("Failed to retrieve user: {}", e)}))
                ))?
                .ok_or_else(|| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": "User not found after creation"}))
                ))?;
            is_new_user = true;
            // Refetch after setting flag
            state.user_core.get_user()
                .map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": format!("Failed to retrieve user after creation: {}", e)}))
                ))?
                .ok_or_else(|| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": "User not found after creation"}))
                ))?
        }
        Err(e) => {
            println!("Database error while checking for user id 1: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Database error"}))
            ));
        }
    };

    // Skip updates if existing user with id == 1
    if !(user.id == 1 && !is_new_user) {
        // Update Twilio creds (if present)
        if let Err(e) = state.user_core.update_twilio_credentials(
            verify_data.twilio_account_sid.as_deref(),
            verify_data.twilio_auth_token.as_deref(),
            verify_data.server_url.as_deref(),
            verify_data.messaging_service_sid.as_deref()
        ) {
            tracing::error!("Failed to update Twilio credentials: {}", e);
        }

        // Update preferred number
        if let Err(e) = state.user_core.update_preferred_number(verify_data.preferred_number.as_str()) {
            tracing::error!("Failed to update preferred number: {}", e);
        }
    }

    // Set phone country
    if let Err(e) = crate::handlers::profile_handlers::set_user_phone_country(&state, &user.phone_number).await {
        tracing::error!("Failed to set phone country during self-hosted login: {}", e);
        // Proceed anyway
    }

    generate_tokens_and_response(user.id)
}
