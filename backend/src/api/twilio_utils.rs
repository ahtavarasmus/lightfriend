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
use sha1::Sha1;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::sync::Arc;
use tracing;
use url::form_urlencoded;

pub async fn set_twilio_webhook(
    account_sid: &str,
    auth_token: &str,
    phone_number: &str,
    _user_id: i32,
    state: Arc<AppState>,
) -> Result<(), Box<dyn Error>> {
    let server_url = env::var("SERVER_URL")?;
    let sms_url = format!("{}/api/sms/server", server_url.trim_end_matches('/'));
    let voice_url = format!("{}/api/voice/incoming", server_url.trim_end_matches('/'));
    let credentials = TwilioCredentials::new(account_sid.to_string(), auth_token.to_string());
    state
        .twilio_client
        .configure_webhook(&credentials, phone_number, &sms_url, Some(&voice_url))
        .await
        .map_err(|e| -> Box<dyn Error> { Box::new(e) })?;
    Ok(())
}

/// Pure function: verify a Twilio webhook signature using HMAC-SHA1.
///
/// This implements Twilio's signature scheme:
///   1. Concatenate the full URL with the sorted form params (key + value, no separators)
///   2. HMAC-SHA1 with the auth token as the key
///   3. Base64-encode and compare with the X-Twilio-Signature header
///
/// Returns Ok(()) if the signature matches, Err with reason otherwise.
pub fn verify_twilio_signature(
    url: &str,
    params: &BTreeMap<String, String>,
    signature_b64: &str,
    auth_token: &str,
) -> Result<(), String> {
    let mut string_to_sign = String::from(url);
    for (key, value) in params.iter() {
        string_to_sign.push_str(key);
        string_to_sign.push_str(value);
    }

    let mut mac = Hmac::<Sha1>::new_from_slice(auth_token.as_bytes())
        .map_err(|e| format!("Failed to create HMAC: {}", e))?;
    mac.update(string_to_sign.as_bytes());

    let signature_bytes = BASE64
        .decode(signature_b64.as_bytes())
        .map_err(|e| format!("Failed to decode signature base64: {}", e))?;

    mac.verify_slice(&signature_bytes)
        .map_err(|_| "HMAC verification failed".to_string())
}

/// Pure function: compute the Twilio signature for a given URL+params+token.
/// Useful for tests.
pub fn compute_twilio_signature(
    url: &str,
    params: &BTreeMap<String, String>,
    auth_token: &str,
) -> String {
    let mut string_to_sign = String::from(url);
    for (key, value) in params.iter() {
        string_to_sign.push_str(key);
        string_to_sign.push_str(value);
    }
    let mut mac = Hmac::<Sha1>::new_from_slice(auth_token.as_bytes()).unwrap();
    mac.update(string_to_sign.as_bytes());
    BASE64.encode(mac.finalize().into_bytes())
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
        Ok(base_url) => {
            let request_path = parts.uri.path();
            tracing::info!(
                "✅ Successfully retrieved SERVER_URL, path: {}",
                request_path
            );
            format!("{}{}", base_url, request_path)
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

    let signature_bytes = match BASE64.decode(signature.as_bytes()) {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!("❌ Failed to decode Twilio signature: {}", e);
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    if mac.verify_slice(&signature_bytes).is_err() {
        tracing::error!("❌ Signature validation failed");
        return Err(StatusCode::UNAUTHORIZED);
    }

    tracing::info!("✅ Signature validation successful");

    // Rebuild request and pass to next handler
    let request = Request::from_parts(parts, Body::from(params_str));

    Ok(next.run(request).await)
}

/// Validate Twilio signature for status callbacks.
///
/// Status callbacks can come from either:
/// 1. The master Twilio account (signed with master TWILIO_AUTH_TOKEN), or
/// 2. A BYOT user's own Twilio account (signed with their auth token).
///
/// We extract MessageSid from the body, look up which user it belongs to,
/// and validate using the correct token.
pub async fn validate_twilio_status_callback_signature(
    State(state): State<Arc<AppState>>,
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

    // Look up which user this MessageSid belongs to so we can pick the right auth token.
    // BYOT users sign callbacks with their own auth token; everyone else uses the master.
    let auth_token = {
        use crate::repositories::twilio_status_repository::TwilioStatusRepository;
        use crate::repositories::twilio_status_repository_impl::DieselTwilioStatusRepository;

        let master_token = std::env::var("TWILIO_AUTH_TOKEN").ok();
        let mut chosen: Option<String> = None;

        if let Some(message_sid) = params.get("MessageSid") {
            let repo = DieselTwilioStatusRepository::new(state.pg_pool.clone());
            if let Ok(Some(user_info)) = repo.get_message_user_info(message_sid) {
                if state.user_core.is_byot_user(user_info.user_id) {
                    if let Ok((_, token)) = state
                        .user_repository
                        .get_twilio_credentials(user_info.user_id)
                    {
                        chosen = Some(token);
                    }
                }
            }
        }

        match chosen.or(master_token) {
            Some(t) => t,
            None => {
                tracing::error!("No usable auth token for status callback validation");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    };

    // Build the callback URL
    let url = match std::env::var("SERVER_URL") {
        Ok(url) => format!("{}/api/twilio/status-callback", url),
        Err(e) => {
            tracing::error!("Failed to get SERVER_URL: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Verify the signature using the chosen auth token
    if let Err(e) = verify_twilio_signature(&url, &params, &signature, &auth_token) {
        tracing::error!("Status callback signature validation failed: {}", e);
        return Err(StatusCode::UNAUTHORIZED);
    }

    tracing::debug!("Status callback signature validation successful");

    // Rebuild request and pass to next handler
    let request = Request::from_parts(parts, Body::from(params_str));
    Ok(next.run(request).await)
}
