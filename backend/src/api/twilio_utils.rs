use crate::models::user_models::{NewMessageStatusLog, User};
use crate::schema::message_status_log;
use crate::AppState;
use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Request, StatusCode},
    middleware,
    response::Response,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use diesel::prelude::*;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use sha1::Sha1;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing;
use url::form_urlencoded;

#[derive(Deserialize)]
pub struct MessageResponse {
    sid: String,
}

#[derive(Deserialize)]
struct PhoneNumbersResponse {
    incoming_phone_numbers: Vec<PhoneNumberInfo>,
}

#[derive(Deserialize)]
struct PhoneNumberInfo {
    sid: String,
}

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
    let client = Client::new();
    let webhook_url = format!("{}/api/sms/server", env::var("SERVER_URL")?);

    // Find the phone number SID
    let params = [("PhoneNumber", phone_number)];
    let response = client
        .get(format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/IncomingPhoneNumbers.json",
            account_sid
        ))
        .basic_auth(account_sid, Some(auth_token))
        .query(&params)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to list phone numbers: {}", response.status()).into());
    }

    let data: PhoneNumbersResponse = response.json().await?;
    let phone_sid = data
        .incoming_phone_numbers
        .first()
        .ok_or("No matching phone number found")?
        .sid
        .clone();

    // Update the webhook
    let update_params = [("SmsUrl", webhook_url.as_str()), ("SmsMethod", "POST")];
    let update_response = client
        .post(format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/IncomingPhoneNumbers/{}.json",
            account_sid, phone_sid
        ))
        .basic_auth(account_sid, Some(auth_token))
        .form(&update_params)
        .send()
        .await?;

    if !update_response.status().is_success() {
        return Err(format!("Failed to update webhook: {}", update_response.status()).into());
    }

    // If Twilio update succeeds, add to ElevenLabs
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
        match state.user_core.get_twilio_credentials(user.id) {
            Ok((_, token)) => token,
            Err(_) => return Err(StatusCode::UNAUTHORIZED),
        }
    } else if crate::utils::country::is_local_number_country(&user.phone_number)
        || crate::utils::country::is_notification_only_country(&user.phone_number)
    {
        std::env::var("TWILIO_AUTH_TOKEN").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        match state.user_core.get_twilio_credentials(user.id) {
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

pub async fn delete_twilio_message_media(
    state: &Arc<AppState>,
    media_sid: &str,
    user: &User,
) -> Result<(), Box<dyn std::error::Error>> {
    // BYOT users with their own credentials always use their own account
    let (account_sid, auth_token) = if state.user_core.is_byot_user(user.id) {
        state.user_core.get_twilio_credentials(user.id)?
    } else if crate::utils::country::is_local_number_country(&user.phone_number)
        || crate::utils::country::is_notification_only_country(&user.phone_number)
    {
        (
            env::var("TWILIO_ACCOUNT_SID")?,
            env::var("TWILIO_AUTH_TOKEN")?,
        )
    } else {
        state.user_core.get_twilio_credentials(user.id)?
    };
    let client = Client::new();

    let response = client
        .delete(format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/Messages/Media/{}",
            account_sid, media_sid
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to delete message media: {} - {}",
            response.status(),
            response.text().await?
        )
        .into());
    }

    tracing::debug!("Successfully deleted message media: {}", media_sid);
    Ok(())
}

use std::time::Duration;
use tokio::time::sleep;

pub async fn delete_twilio_message(
    state: &Arc<AppState>,
    message_sid: &str,
    user: &User,
) -> Result<(), Box<dyn Error>> {
    tracing::debug!("deleting incoming message");

    // BYOT users with their own credentials always use their own account
    let (account_sid, auth_token) = if state.user_core.is_byot_user(user.id) {
        state.user_core.get_twilio_credentials(user.id)?
    } else if crate::utils::country::is_local_number_country(&user.phone_number)
        || crate::utils::country::is_notification_only_country(&user.phone_number)
    {
        (
            env::var("TWILIO_ACCOUNT_SID")?,
            env::var("TWILIO_AUTH_TOKEN")?,
        )
    } else {
        state.user_core.get_twilio_credentials(user.id)?
    };

    let client = Client::new();

    // Wait 1-2 minutes to avoid 'resource not complete' errors
    sleep(Duration::from_secs(60)).await;

    let mut attempts = 0;
    loop {
        let response = client
            .delete(format!(
                "https://api.twilio.com/2010-04-01/Accounts/{}/Messages/{}.json",
                account_sid, message_sid
            ))
            .basic_auth(&account_sid, Some(&auth_token))
            .send()
            .await?;

        if response.status().is_success() {
            tracing::info!("Incoming message deleted: {}", message_sid);
            return Ok(());
        } else if attempts < 3 {
            attempts += 1;
            let wait_secs = 60 * attempts as u64;
            tracing::warn!("Retry deletion after {} seconds", wait_secs);
            sleep(Duration::from_secs(wait_secs)).await;
        } else {
            return Err(format!("Failed to delete: {}", response.status()).into());
        }
    }
}

pub async fn send_conversation_message(
    state: &Arc<AppState>,
    body: &str,
    media_sid: Option<&String>,
    user: &User,
) -> Result<String, Box<dyn Error>> {
    let history_entry = crate::models::user_models::NewMessageHistory {
        user_id: user.id,
        role: "assistant".to_string(),
        encrypted_content: body.to_string(),
        tool_name: None,
        tool_call_id: None,
        tool_calls_json: None,
        created_at: chrono::Utc::now().timestamp() as i32,
        conversation_id: "".to_string(),
    };

    if let Err(e) = state.user_repository.create_message_history(&history_entry) {
        tracing::error!(
            "Failed to store WhatsApp confirmation message in history: {}",
            e
        );
    }

    let running_environment = env::var("ENVIRONMENT").map_err(|_| "ENVIRONMENT not set")?;
    if running_environment == "development" {
        tracing::info!("NOT SENDING MESSAGE SINCE ENVIRONMENT IS DEVELOPMENT");
        return Ok("dev not sending anything".to_string());
    }

    // Twilio send logic
    // BYOT users with their own credentials always use their own account
    // Otherwise, use global credentials for local-number and notification-only countries
    let (account_sid, auth_token) = if state.user_core.is_byot_user(user.id) {
        // BYOT user - use their own Twilio account
        state.user_core.get_twilio_credentials(user.id)?
    } else if crate::utils::country::is_local_number_country(&user.phone_number)
        || crate::utils::country::is_notification_only_country(&user.phone_number)
    {
        (
            env::var("TWILIO_ACCOUNT_SID")?,
            env::var("TWILIO_AUTH_TOKEN")?,
        )
    } else {
        // Non-supported country must have their own credentials
        state.user_core.get_twilio_credentials(user.id)?
    };

    let client = Client::new();

    // Get or set country
    let mut country = user.phone_number_country.clone();
    if country.is_none() {
        match crate::handlers::profile_handlers::set_user_phone_country(
            state,
            user.id,
            &user.phone_number,
        )
        .await
        {
            Ok(c) => country = c,
            Err(e) => {
                tracing::error!("Failed to set phone country: {}", e);
                // Fallback to preferred_number logic
                country = Some("Other".to_string());
            }
        }
    }

    // Determine From strategy
    // IMPORTANT: For notification-only countries without BYOT credentials,
    // we must ALWAYS use messaging service - never send from US number directly
    let preferred = user.preferred_number.as_deref().unwrap_or("");
    let has_byot_credentials = state.user_core.is_byot_user(user.id);
    let is_notification_only =
        crate::utils::country::is_notification_only_country(&user.phone_number);

    let mut from_number = String::new();
    let mut use_messaging_service = false;
    let mut update_preferred = false;

    // Notification-only countries without BYOT: check if user selected US or local number
    if is_notification_only && !has_byot_credentials {
        let us_phone = env::var("USA_PHONE").ok();
        // If preferred is US number or empty, use messaging service. Otherwise use selected local number.
        if preferred.is_empty() || us_phone.as_deref() == Some(preferred) {
            use_messaging_service = true;
            tracing::info!(
                "Using US messaging service for notification-only country user {}",
                user.id
            );
        } else {
            // User selected a non-US local number (FI, NL, GB, AU)
            from_number = preferred.to_string();
            tracing::info!(
                "Using selected local number {} for notification-only user {}",
                from_number,
                user.id
            );
        }
    } else if let Some(c) = country.clone() {
        match c.as_str() {
            "US" => {
                use_messaging_service = true;
            }
            "CA" => {
                from_number = if !preferred.is_empty() {
                    preferred.to_string()
                } else {
                    update_preferred = true;
                    env::var("CAN_PHONE").expect("CAN_PHONE not set")
                };
            }
            "FI" => {
                from_number = if !preferred.is_empty() {
                    preferred.to_string()
                } else {
                    update_preferred = true;
                    env::var("FIN_PHONE").expect("FIN_PHONE not set")
                };
            }
            "NL" => {
                from_number = if !preferred.is_empty() {
                    preferred.to_string()
                } else {
                    update_preferred = true;
                    env::var("NL_PHONE").expect("NL_PHONE not set")
                };
            }
            "GB" => {
                from_number = if !preferred.is_empty() {
                    preferred.to_string()
                } else {
                    update_preferred = true;
                    env::var("GB_PHONE").expect("GB_PHONE not set")
                };
            }
            "AU" => {
                from_number = if !preferred.is_empty() {
                    preferred.to_string()
                } else {
                    update_preferred = true;
                    env::var("AUS_PHONE").expect("AUS_PHONE not set")
                };
            }
            _ => {
                // For other countries with BYOT credentials, use their preferred number
                if has_byot_credentials && !preferred.is_empty() {
                    from_number = preferred.to_string();
                } else {
                    tracing::info!("Using empty from_number for unsupported country: {}", c);
                }
            }
        }
    }

    if update_preferred && !from_number.is_empty() {
        let _ = state
            .user_core
            .update_preferred_number(user.id, &from_number)
            .map_err(|e| {
                tracing::error!(
                    "Failed to update preferred_number for user {}: {:?}",
                    user.id,
                    e
                )
            });
    }

    // Build form_data
    let mut form_data = vec![("To", user.phone_number.as_str()), ("Body", body)];
    let sid =
        env::var("TWILIO_MESSAGING_SERVICE_SID").expect("TWILIO_MESSAGING_SERVICE_SID not set");

    // Add StatusCallback URL for delivery status tracking
    let server_url = env::var("SERVER_URL").unwrap_or_default();
    let status_callback_url = format!("{}/api/twilio/status-callback", server_url);

    if use_messaging_service {
        form_data.push(("MessagingServiceSid", sid.as_str()));
    } else if !from_number.is_empty() {
        form_data.push(("From", from_number.as_str()));
    } else {
        tracing::warn!(
            "No valid From available for user {} and country {:?}",
            user.id,
            country
        );
        // Fallback or error as needed
    }

    // Only add StatusCallback if SERVER_URL is configured
    if !server_url.is_empty() {
        form_data.push(("StatusCallback", status_callback_url.as_str()));
    }

    // Handle media_sid if provided
    let media_url: String;
    if let Some(media_id) = media_sid {
        // Construct the MediaUrl using the media_sid (corrected to without .json and proper path)
        // Note: This assumes media_sid is a valid Media SID hosted on Twilio. However, Twilio API URLs require authentication,
        // so this may not work for sending MMS as the MediaUrl must be publicly accessible. Consider hosting media externally (e.g., S3) for reliability.
        media_url = format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/Media/{}",
            account_sid, media_id
        );
        form_data.push(("MediaUrl", &media_url));
    }

    let resp = client
        .post(format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json",
            account_sid
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .form(&form_data)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        tracing::error!("Twilio send error: status {}, body: {}", status, text);
        return Err(format!("Failed to send message: {}", text).into());
    }

    let response: MessageResponse = resp.json().await?;

    tracing::debug!(
        "Successfully sent message{} with SID: {}",
        if media_sid.is_some() {
            " with media"
        } else {
            ""
        },
        response.sid
    );

    // Log initial message status to database for tracking
    if let Ok(mut conn) = state.db_pool.get() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;
        let new_status = NewMessageStatusLog {
            message_sid: response.sid.clone(),
            user_id: user.id,
            direction: "outbound".to_string(),
            to_number: user.phone_number.clone(),
            from_number: if from_number.is_empty() {
                None
            } else {
                Some(from_number.clone())
            },
            status: "queued".to_string(),
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
            price: None, // Price comes from Twilio callback after delivery
            price_unit: None,
        };

        if let Err(e) = diesel::insert_into(message_status_log::table)
            .values(&new_status)
            .execute(&mut conn)
        {
            tracing::error!(
                "Failed to log message status for SID {}: {}",
                response.sid,
                e
            );
        } else {
            tracing::info!("Logged initial message status for SID {}", response.sid);
        }
    }

    // Message deletion now happens in the status callback handler
    // when the message reaches a final status (delivered/failed/undelivered)

    Ok(response.sid)
}
