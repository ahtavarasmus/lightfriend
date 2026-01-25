use crate::handlers::auth_middleware::AuthUser;
use crate::AppState;
use crate::UserCoreOps;
use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;

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
    match state
        .user_core
        .update_preferred_number(auth_user.user_id, &req.twilio_phone)
    {
        Ok(_) => {
            tracing::debug!(
                "Successfully updated Twilio phone for user: {}",
                auth_user.user_id
            );

            if let Ok((account_sid, auth_token)) =
                state.user_core.get_twilio_credentials(auth_user.user_id)
            {
                let phone = req.twilio_phone.clone();
                let user_id = auth_user.user_id;
                let state_clone = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = crate::api::twilio_utils::set_twilio_webhook(
                        &account_sid,
                        &auth_token,
                        &phone,
                        user_id,
                        state_clone,
                    )
                    .await
                    {
                        tracing::error!("Failed to set Twilio webhook for phone {}: {}", phone, e);
                        // Proceed anyway(probably user hasn't given their twilio credentials yet, we will try again when they do)
                    } else {
                        tracing::debug!("Successfully set Twilio webhook for phone: {}", phone);
                    }
                });
            } else {
                tracing::warn!(
                    "Twilio credentials not found for user {}, skipping webhook update",
                    auth_user.user_id
                );
            }

            Ok(StatusCode::OK)
        }
        Err(e) => {
            tracing::error!("Failed to update Twilio phone: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update Twilio phone"})),
            ))
        }
    }
}

pub async fn update_twilio_creds(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<UpdateTwilioCredsRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let user_opt = match state.user_core.find_by_id(auth_user.user_id) {
        Ok(opt) => opt,
        Err(e) => {
            tracing::error!("Failed to fetch user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to fetch user"})),
            ));
        }
    };

    let user = match user_opt {
        Some(u) => u,
        None => {
            tracing::error!("User not found: {}", auth_user.user_id);
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            ));
        }
    };

    match state.user_core.update_twilio_credentials(
        auth_user.user_id,
        &req.account_sid,
        &req.auth_token,
    ) {
        Ok(_) => {
            tracing::debug!(
                "Successfully updated Twilio credentials for user: {}",
                auth_user.user_id
            );

            if let Some(phone) = user.preferred_number {
                let account_sid = req.account_sid.clone();
                let auth_token = req.auth_token.clone();
                let phone = phone.clone();
                let user_id = auth_user.user_id;
                let state_clone = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = crate::api::twilio_utils::set_twilio_webhook(
                        &account_sid,
                        &auth_token,
                        &phone,
                        user_id,
                        state_clone,
                    )
                    .await
                    {
                        tracing::error!("Failed to set Twilio webhook for phone {}: {}", phone, e);
                        // Proceed anyway(probably user hasn't inputted their twilio number yet, we try again when they do)
                    } else {
                        tracing::debug!("Successfully set Twilio webhook for phone: {}", phone);
                    }
                });
            }

            Ok(StatusCode::OK)
        }
        Err(e) => {
            tracing::error!("Failed to update Twilio credentials: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update Twilio credentials"})),
            ))
        }
    }
}

/// Clear BYOT Twilio credentials (manual removal)
pub async fn clear_twilio_creds(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    match state.user_core.clear_twilio_credentials(auth_user.user_id) {
        Ok(_) => {
            tracing::info!(
                "Successfully cleared BYOT credentials for user: {}",
                auth_user.user_id
            );
            Ok(StatusCode::OK)
        }
        Err(e) => {
            tracing::error!("Failed to clear BYOT credentials: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to clear BYOT credentials"})),
            ))
        }
    }
}

pub async fn update_textbee_creds(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<UpdateTextBeeCredsRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    match state.user_core.update_textbee_credentials(
        auth_user.user_id,
        &req.textbee_device_id,
        &req.textbee_api_key,
    ) {
        Ok(_) => {
            println!(
                "Successfully updated TextBee credentials for user: {}",
                auth_user.user_id
            );
            Ok(StatusCode::OK)
        }
        Err(e) => {
            tracing::error!("Failed to update TextBee credentials: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update TextBee credentials"})),
            ))
        }
    }
}
