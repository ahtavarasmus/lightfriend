use crate::api::twilio_client::{TwilioClient, TwilioCredentials};
use crate::AppState;
use crate::UserCoreOps;
use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Request, StatusCode},
    middleware,
    response::Response,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use sha1::Sha1;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::sync::Arc;
use tracing;
use url::form_urlencoded;

#[derive(Deserialize)]
struct ElevenLabsResponse {
    phone_number_id: String,
}

pub async fn set_twilio_webhook(
    account_sid: &str,
    auth_token: &str,
    phone_number: &str,
    user_id: i32,
    state: Arc<AppState>,
) -> Result<(), Box<dyn Error>> {
    let webhook_url = format!("{}/api/sms/server", env::var("SERVER_URL")?);

    // Use TwilioClient to configure the webhook
    let credentials = TwilioCredentials::new(account_sid.to_string(), auth_token.to_string());
    state
        .twilio_client
        .configure_webhook(&credentials, phone_number, &webhook_url)
        .await
        .map_err(|e| -> Box<dyn Error> { Box::new(e) })?;

    // If Twilio update succeeds, add to ElevenLabs
    let client = Client::new();
    let eleven_key = env::var("ELEVENLABS_API_KEY")?;

    // Check for existing phone number ID and delete if exists
    let existing_id = state.user_core.get_elevenlabs_phone_number_id(user_id)?;
    if let Some(id) = existing_id {
        let delete_url = format!("https://api.elevenlabs.io/v1/convai/phone-numbers/{}", id);
        let delete_response = client
            .delete(delete_url)
            .header("xi-api-key", eleven_key.clone())
            .send()
            .await?;

        let delete_status = delete_response.status();
        if !delete_status.is_success() {
            let error_text = delete_response.text().await.unwrap_or_default();
            tracing::error!(
                "Failed to delete existing phone number from ElevenLabs: {} - {}",
                delete_status,
                error_text
            );
            // Proceed anyway
        } else {
            tracing::debug!("Successfully deleted existing phone number from ElevenLabs");
        }
    }

    let label = format!("{} number", user_id);
    let el_body = json!({
        "phone_number": phone_number,
        "label": label,
        "sid": account_sid,
        "token": auth_token,
        "provider": "twilio"
    });

    let el_response = client
        .post("https://api.elevenlabs.io/v1/convai/phone-numbers")
        .header("xi-api-key", eleven_key.clone())
        .header("Content-Type", "application/json")
        .json(&el_body)
        .send()
        .await?;

    let status = el_response.status();
    if !status.is_success() {
        let error_text = el_response.text().await.unwrap_or_default();
        tracing::error!(
            "Failed to add phone number to ElevenLabs: {} - {}",
            status,
            error_text
        );
        // Proceed anyway, as per requirements
    } else {
        let el_data: ElevenLabsResponse = el_response.json().await?;
        if let Err(e) = state
            .user_core
            .set_elevenlabs_phone_number_id(user_id, &el_data.phone_number_id)
        {
            tracing::error!("Failed to set ElevenLabs phone number ID: {}", e);
        }
        tracing::debug!("Successfully added phone number to ElevenLabs");
        // Assign agent to the phone number
        let agent_id = env::var("AGENT_ID")?;
        let assign_body = json!({
            "agent_id": agent_id
        });

        let assign_response = client
            .patch(format!(
                "https://api.elevenlabs.io/v1/convai/phone-numbers/{}",
                el_data.phone_number_id
            ))
            .header("xi-api-key", eleven_key)
            .header("Content-Type", "application/json")
            .json(&assign_body)
            .send()
            .await?;

        let assign_status = assign_response.status();
        if !assign_status.is_success() {
            let error_text = assign_response.text().await.unwrap_or_default();
            tracing::error!(
                "Failed to assign agent to phone number in ElevenLabs: {} - {}",
                assign_status,
                error_text
            );
            // Proceed anyway
        } else {
            tracing::debug!("Successfully assigned agent to phone number in ElevenLabs");
        }
    }

    Ok(())
}

pub async fn validate_twilio_signature(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: middleware::Next,
) -> Result<Response, StatusCode> {
    tracing::info!("\n=== Starting Twilio Signature Validation ===");

    // Get the Twilio signature from headers
    let signature = match request.headers().get("X-Twilio-Signature") {
        Some(header) => match header.to_str() {
            Ok(s) => {
                tracing::info!("✅ Successfully retrieved X-Twilio-Signature");
                s.to_string()
            }
            Err(e) => {
                tracing::error!("❌ Error converting X-Twilio-Signature to string: {}", e);
                return Err(StatusCode::UNAUTHORIZED);
            }
        },
        None => {
            tracing::error!("❌ No X-Twilio-Signature header found");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Get request body for validation
    let (parts, body) = request.into_parts();
    let body_bytes = match to_bytes(body, 1024 * 1024).await {
        // 1MB limit
        Ok(bytes) => {
            tracing::info!("✅ Successfully read request body");
            bytes
        }
        Err(e) => {
            tracing::error!("❌ Failed to read request body: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Convert body to string and parse form data
    let params_str = match String::from_utf8(body_bytes.to_vec()) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("❌ Failed to convert body to UTF-8: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Parse form parameters into a sorted map
    let params: BTreeMap<String, String> = form_urlencoded::parse(params_str.as_bytes())
        .into_owned()
        .collect();

    let from_phone = match params.get("From") {
        Some(phone) => {
            tracing::info!("✅ Found From phone number: {}", phone);
            phone
        }
        None => {
            tracing::error!("❌ No From phone number found in payload");
            return Err(StatusCode::BAD_REQUEST);
        }
    };
    let user = match state.user_core.find_by_phone_number(from_phone) {
        Ok(Some(user)) => {
            tracing::info!("✅ Found user {} for phone {}", user.id, from_phone);
            user
        }
        Ok(None) => {
            tracing::error!("❌ No user found for phone {}", from_phone);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Err(e) => {
            tracing::error!("❌ Failed to query user by phone {}: {}", from_phone, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // BYOT users with their own credentials always use their own account
    let auth_token = if state.user_core.is_byot_user(user.id) {
        match state.user_repository.get_twilio_credentials(user.id) {
            Ok((_, token)) => token,
            Err(_) => return Err(StatusCode::UNAUTHORIZED),
        }
    } else if crate::utils::country::is_local_number_country(&user.phone_number)
        || crate::utils::country::is_notification_only_country(&user.phone_number)
    {
        std::env::var("TWILIO_AUTH_TOKEN").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        match state.user_repository.get_twilio_credentials(user.id) {
            Ok((_, token)) => token,
            Err(_) => return Err(StatusCode::UNAUTHORIZED),
        }
    };

    let url = match std::env::var("SERVER_URL") {
        Ok(url) => {
            tracing::info!("✅ Successfully retrieved SERVER_URL");
            url + "/api/sms/server"
        }
        Err(e) => {
            tracing::error!("❌ Failed to get SERVER_URL: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Build the string to sign
    let mut string_to_sign = url;
    for (key, value) in params.iter() {
        string_to_sign.push_str(key);
        string_to_sign.push_str(value);
    }

    // Create HMAC-SHA1
    let mut mac = match Hmac::<Sha1>::new_from_slice(auth_token.as_bytes()) {
        Ok(mac) => mac,
        Err(e) => {
            tracing::error!("❌ Failed to create HMAC: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    mac.update(string_to_sign.as_bytes());

    // Get the result and encode as base64
    let result = BASE64.encode(mac.finalize().into_bytes());

    // Compare signatures
    if result != signature {
        tracing::error!("❌ Signature validation failed");
        return Err(StatusCode::UNAUTHORIZED);
    }

    tracing::info!("✅ Signature validation successful");

    // Rebuild request and pass to next handler
    let request = Request::from_parts(parts, Body::from(params_str));

    Ok(next.run(request).await)
}

/// Validate Twilio signature for status callbacks
///
/// This is a simpler version that doesn't need to look up users since:
/// 1. Status callbacks come from our main Twilio account
/// 2. The From number is Lightfriend's number, not the user's
pub async fn validate_twilio_status_callback_signature(
    request: Request<Body>,
    next: middleware::Next,
) -> Result<Response, StatusCode> {
    tracing::debug!("=== Starting Twilio Status Callback Signature Validation ===");

    // Get the Twilio signature from headers
    let signature = match request.headers().get("X-Twilio-Signature") {
        Some(header) => match header.to_str() {
            Ok(s) => s.to_string(),
            Err(e) => {
                tracing::error!("Error converting X-Twilio-Signature to string: {}", e);
                return Err(StatusCode::UNAUTHORIZED);
            }
        },
        None => {
            tracing::error!("No X-Twilio-Signature header found for status callback");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Get the auth token - always use main account for status callbacks
    let auth_token = match std::env::var("TWILIO_AUTH_TOKEN") {
        Ok(token) => token,
        Err(e) => {
            tracing::error!("Failed to get TWILIO_AUTH_TOKEN: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Get request body for validation
    let (parts, body) = request.into_parts();
    let body_bytes = match to_bytes(body, 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!("Failed to read request body: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let params_str = match String::from_utf8(body_bytes.to_vec()) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to convert body to UTF-8: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Parse form parameters into a sorted map
    let params: BTreeMap<String, String> = form_urlencoded::parse(params_str.as_bytes())
        .into_owned()
        .collect();

    // Build the callback URL
    let url = match std::env::var("SERVER_URL") {
        Ok(url) => format!("{}/api/twilio/status-callback", url),
        Err(e) => {
            tracing::error!("Failed to get SERVER_URL: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Build the string to sign (URL + sorted params)
    let mut string_to_sign = url;
    for (key, value) in params.iter() {
        string_to_sign.push_str(key);
        string_to_sign.push_str(value);
    }

    // Create HMAC-SHA1
    let mut mac = match Hmac::<Sha1>::new_from_slice(auth_token.as_bytes()) {
        Ok(mac) => mac,
        Err(e) => {
            tracing::error!("Failed to create HMAC: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    mac.update(string_to_sign.as_bytes());
    let result = BASE64.encode(mac.finalize().into_bytes());

    // Compare signatures
    if result != signature {
        tracing::error!("Status callback signature validation failed");
        return Err(StatusCode::UNAUTHORIZED);
    }

    tracing::debug!("Status callback signature validation successful");

    // Rebuild request and pass to next handler
    let request = Request::from_parts(parts, Body::from(params_str));
    Ok(next.run(request).await)
}
