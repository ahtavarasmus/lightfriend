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

            if let Ok((account_sid, auth_token)) = state
                .user_repository
                .get_twilio_credentials(auth_user.user_id)
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

    match state.user_repository.update_twilio_credentials(
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

/// Verify BYOT Twilio setup by making read-only API calls.
/// Checks:
///   1. Credentials authenticate successfully
///   2. The phone number exists in their Twilio account
///   3. SMS webhook is pointing at our server
///   4. Voice webhook is pointing at our server
///
/// Returns a detailed status report without sending any message.
pub async fn verify_byot_setup(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;

    // Fetch user
    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|e| {
            tracing::error!("Failed to fetch user: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to fetch user"})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        ))?;

    // Get phone number
    let phone_number = match user.preferred_number {
        Some(p) if !p.is_empty() => p,
        _ => {
            return Ok(Json(json!({
                "ok": false,
                "step": "phone_number",
                "error": "No BYOT phone number configured",
            })));
        }
    };

    // Get credentials
    let (account_sid, auth_token) = match state.user_repository.get_twilio_credentials(user_id) {
        Ok(creds) => creds,
        Err(_) => {
            return Ok(Json(json!({
                "ok": false,
                "step": "credentials",
                "error": "No Twilio credentials configured",
            })));
        }
    };

    // Call Twilio API to list incoming phone numbers and find ours
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/IncomingPhoneNumbers.json",
        account_sid
    );
    let response = match client
        .get(&url)
        .basic_auth(&account_sid, Some(&auth_token))
        .query(&[("PhoneNumber", phone_number.as_str())])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Ok(Json(json!({
                "ok": false,
                "step": "twilio_api",
                "error": format!("Network error contacting Twilio: {}", e),
            })));
        }
    };

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Ok(Json(json!({
            "ok": false,
            "step": "auth",
            "error": "Twilio rejected your credentials (check Account SID and Auth Token)",
        })));
    }
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Ok(Json(json!({
            "ok": false,
            "step": "twilio_api",
            "error": format!("Twilio API error {}: {}", status, body),
        })));
    }

    // Parse response and find the number
    let data: serde_json::Value = response.json().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to parse Twilio response: {}", e)})),
        )
    })?;

    let numbers = data["incoming_phone_numbers"].as_array();
    let number_info = numbers.and_then(|arr| arr.first());
    let number_info = match number_info {
        Some(n) => n,
        None => {
            return Ok(Json(json!({
                "ok": false,
                "step": "phone_number_lookup",
                "error": format!("Phone number {} not found in your Twilio account", phone_number),
            })));
        }
    };

    // Build expected webhook URLs from SERVER_URL
    let server_url = std::env::var("SERVER_URL").unwrap_or_default();
    let expected_sms = format!("{}/api/sms/server", server_url.trim_end_matches('/'));
    let expected_voice = format!("{}/api/voice/incoming", server_url.trim_end_matches('/'));

    let actual_sms = number_info["sms_url"].as_str().unwrap_or("").to_string();
    let actual_voice = number_info["voice_url"].as_str().unwrap_or("").to_string();

    let sms_ok = actual_sms == expected_sms;
    let voice_ok = actual_voice == expected_voice;
    let ok = sms_ok && voice_ok;

    Ok(Json(json!({
        "ok": ok,
        "phone_number": phone_number,
        "credentials_valid": true,
        "sms_webhook": {
            "ok": sms_ok,
            "expected": expected_sms,
            "actual": actual_sms,
        },
        "voice_webhook": {
            "ok": voice_ok,
            "expected": expected_voice,
            "actual": actual_voice,
        },
    })))
}

/// Clear BYOT Twilio credentials (manual removal)
pub async fn clear_twilio_creds(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    match state
        .user_repository
        .clear_twilio_credentials(auth_user.user_id)
    {
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
