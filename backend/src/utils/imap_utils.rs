use crate::AppState;
use std::sync::Arc;
use tracing::{info, error};
use std::env;
use openai_api_rs;
use serde_json;
use reqwest;
use serde::Deserialize;

use crate::handlers::imap_handlers::ImapEmailPreview;

use reqwest::multipart;

#[derive(Debug, Deserialize)]
struct TwilioMediaResponse {
    sid: String,
    links: TwilioMediaLinks,
}

#[derive(Debug, Deserialize)]
struct TwilioMediaLinks {
    #[serde(default)]
    content_direct_temporary: Option<String>,
    content: String,
}

pub async fn upload_media_to_twilio(
    content_type: String,
    data: Vec<u8>,
    filename: String,
    service_sid: String,
) -> Result<String, Box<dyn std::error::Error>> {
    let twilio_account_sid = env::var("TWILIO_ACCOUNT_SID")
        .map_err(|_| "TWILIO_ACCOUNT_SID not set")?;
    let twilio_auth_token = env::var("TWILIO_AUTH_TOKEN")
        .map_err(|_| "TWILIO_AUTH_TOKEN not set")?;
    
    let client = reqwest::Client::new();
    
    let url = format!(
        "https://mcs.us1.twilio.com/v1/Services/{}/Media",
        service_sid
    );

    tracing::info!("Uploading media to Twilio. Content-Type: {}, Filename: {}", content_type, filename);

    // Create multipart form data
    let part = multipart::Part::bytes(data)
        .file_name(filename.clone())
        .mime_str(&content_type)?;
    
    let form = multipart::Form::new()
        .part("file", part);
    
    let response = client
        .post(&url)
        .basic_auth(&twilio_account_sid, Some(&twilio_auth_token))
        .multipart(form)
        .send()
        .await?;
    
    let status = response.status();
    let headers = response.headers().clone();
    
    tracing::debug!("Twilio response status: {}", status);
    tracing::debug!("Twilio response headers: {:?}", headers);
    
    if !status.is_success() {
        let error_text = response.text().await?;
        tracing::error!("Twilio upload failed with status {} and error: {}", status, error_text);
        return Err(format!("Failed to upload media: {} - {}", status, error_text).into());
    }
    
    let response_text = response.text().await?;
    tracing::debug!("Twilio response body: {}", response_text);
    
    match serde_json::from_str::<TwilioMediaResponse>(&response_text) {
        Ok(media_response) => {
            tracing::info!("Successfully uploaded media to Twilio");
            match media_response.links.content_direct_temporary {
                Some(url) => Ok(url),
                None => Ok(format!( // ← new fallback
                    "https://mcs.us1.twilio.com{}",
                    media_response.links.content
                )),
            }
        },
        Err(e) => {
            tracing::error!("Failed to parse Twilio response: {}", e);
            tracing::error!("Raw response: {}", response_text);
            Err(format!("Failed to parse Twilio response: {} - Raw response: {}", e, response_text).into())
        }
    }
}


pub async fn send_notification_about_email(
    state: &Arc<AppState>,
    user_id: i32,
    notification: &str,
) {
    // Get user info
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::error!("User {} not found for notification", user_id);
            return;
        }
        Err(e) => {
            tracing::error!("Failed to get user {}: {}", user_id, e);
            return;
        }
    };

    // Get user settings (assuming state has a user_settings repository or similar)
    let user_settings = match state.user_core.get_user_settings(user_id) {
        Ok(settings) => settings,
        Err(e) => {
            tracing::error!("Failed to get settings for user {}: {}", user_id, e);
            return;
        }
    };

    // Get the user's preferred number or default
    let sender_number = match user.preferred_number.clone() {
        Some(number) => {
            tracing::info!("Using user's preferred number: {}", number);
            number
        }
        None => {
            let number = std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set");
            tracing::info!("Using default SHAZAM_PHONE_NUMBER: {}", number);
            number
        }
    };

    // Get the conversation for the user
    let conversation = match state.user_conversations.get_conversation(&user, sender_number).await {
        Ok(conv) => conv,
        Err(e) => {
            tracing::error!("Failed to ensure conversation exists: {}", e);
            return;
        }
    };

    // Check user's notification preference from settings
    let notification_type = user_settings.notification_type.as_deref().unwrap_or("sms");
    match notification_type {
        "call" => {
            // For calls, we need a brief intro and detailed message
            let notification_first_message = "Hello, I have a critical Email to tell you about.".to_string();

            // Create dynamic variables (optional, can be customized based on needs)
            let mut dynamic_vars = std::collections::HashMap::new();

            match crate::api::elevenlabs::make_notification_call(
                &state.clone(),
                user.phone_number.clone(),
                user.preferred_number
                    .unwrap_or_else(|| std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set")),
                "email".to_string(), // Notification type
                notification_first_message,
                notification.clone().to_string(),
                user.id.to_string(),
                user_settings.timezone,
            ).await {
                Ok(mut response) => {
                    // Add dynamic variables to the client data
                    if let Some(client_data) = response.get_mut("client_data") {
                        if let Some(obj) = client_data.as_object_mut() {
                            obj.extend(dynamic_vars.into_iter().map(|(k, v)| (k, serde_json::Value::String(v))));
                        }
                    }
                    tracing::debug!("Successfully initiated call notification for user {}", user.id);
                }
                Err((_, json_err)) => {
                    error!("Failed to initiate call notification: {:?}", json_err);
                    println!("Failed to send call notification for user {}", user_id);
                }
            }
        }
        _ => {
            // Default to SMS notification
            match crate::api::twilio_utils::send_conversation_message(
                &conversation.conversation_sid,
                &conversation.twilio_number,
                &notification,
                true,
                None,
                &user,
            ).await {
                Ok(_) => {
                    tracing::info!("Successfully sent notification about an email to user {}", user_id);
                    println!("SMS notification sent successfully for user {}", user_id);
                }
                Err(e) => {
                    tracing::error!("Failed to send notification about an email: {}", e);
                    println!("Failed to send SMS notification for user {}", user_id);
                }
            }
        }
    }
}
