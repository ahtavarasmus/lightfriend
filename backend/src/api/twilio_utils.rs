use reqwest::Client;
use serde::Deserialize;
use std::env;
use crate::models::user_models::User;
use std::error::Error;
use axum::{
    http::{Request, StatusCode},
    middleware,
    response::Response,
    body::{Body, to_bytes},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::collections::BTreeMap;
use url::form_urlencoded;
use tracing;

#[derive(Deserialize)]
pub struct MessageResponse {
    sid: String,
}

#[derive(Deserialize)]
pub struct ConversationResponse {
    sid: String,
    chat_service_sid: String,
}

#[derive(Deserialize, Debug)]
pub struct ParticipantResponse {
    pub sid: String,
    pub messaging_binding: Option<MessagingBinding>,
}

#[derive(Deserialize, Debug)]
pub struct MessagingBinding {
    pub address: Option<String>,
    pub proxy_address: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ParticipantsListResponse {
    participants: Vec<ParticipantResponse>,
}

pub async fn fetch_conversation_participants(conversation_sid: &str) -> Result<Vec<ParticipantResponse>, Box<dyn Error>> {
    let account_sid = env::var("TWILIO_ACCOUNT_SID")?;
    let auth_token = env::var("TWILIO_AUTH_TOKEN")?;
    let client = Client::new();

    let response: ParticipantsListResponse = client
        .get(format!(
            "https://conversations.twilio.com/v1/Conversations/{}/Participants",
            conversation_sid
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .send()
        .await?
        .json()
        .await?;

    Ok(response.participants)
}


pub async fn delete_twilio_conversation(conversation_sid: &str) -> Result<(), Box<dyn Error>> {
    let account_sid = env::var("TWILIO_ACCOUNT_SID")?;
    let auth_token = env::var("TWILIO_AUTH_TOKEN")?;
    let client = Client::new();

    let response = client
        .delete(format!(
            "https://conversations.twilio.com/v1/Conversations/{}",
            conversation_sid
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(error_text.into());
    }

    println!("Successfully deleted conversation: {}", conversation_sid);
    Ok(())
}


pub async fn create_twilio_conversation_for_participant(user: &User, proxy_address: String) -> Result<(String, String), Box<dyn Error>> {
    let account_sid = env::var("TWILIO_ACCOUNT_SID")?;
    let auth_token = env::var("TWILIO_AUTH_TOKEN")?;
    let client = Client::new();

    // Create a new conversation
    let conversation: ConversationResponse = client
        .post("https://conversations.twilio.com/v1/Conversations")
        .basic_auth(&account_sid, Some(&auth_token))
        .form(&[("FriendlyName", format!("Chat with {}", user.email))])
        .send()
        .await?
        .json()
        .await?;

    println!("conversation.conversation_sid: {}",conversation.sid);

    // Add the user as participant
    let participant_response = client
        .post(format!(
            "https://conversations.twilio.com/v1/Conversations/{}/Participants",
            conversation.sid
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .form(&[
            ("MessagingBinding.Address", &user.phone_number),
            ("MessagingBinding.ProxyAddress", &proxy_address),
        ])
        .send()
        .await?;


    let status = participant_response.status();
    if !status.is_success() {
        let error_text = participant_response.text().await?;
        println!("Participant addition response: {}", error_text);
        
        // Check if this is the "participant already exists" error
        if status.as_u16() == 409 {
            // Extract the existing conversation SID from the error message
            if let Some(conv_sid) = error_text.find("Conversation ").and_then(|i| {
                let start = i + "Conversation ".len();
                error_text[start..].find('"').map(|end| error_text[start..start+end].to_string())
            }) {
                // Fetch the conversation details to get the service_sid
                let conversation: ConversationResponse = client
                    .get(format!(
                        "https://conversations.twilio.com/v1/Conversations/{}",
                        conv_sid
                    ))
                    .basic_auth(&account_sid, Some(&auth_token))
                    .send()
                    .await?
                    .json()
                    .await?;
                
                println!("Found existing conversation: {}", conv_sid);
                return Ok((conv_sid, conversation.chat_service_sid));
            }
        }
        
        return Err(error_text.into());
    }

    println!("Successfully added participant to conversation");

    Ok((conversation.sid, conversation.chat_service_sid))
}


pub async fn validate_twilio_signature(
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
            },
            Err(e) => {
                tracing::info!("❌ Error converting X-Twilio-Signature to string: {}", e);
                return Err(StatusCode::UNAUTHORIZED);
            }
        },
        None => {
            tracing::info!("❌ No X-Twilio-Signature header found");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Get the Twilio auth token
    let auth_token = match std::env::var("TWILIO_AUTH_TOKEN") {
        Ok(token) => {
            tracing::info!("✅ Successfully retrieved TWILIO_AUTH_TOKEN");
            token
        },
        Err(e) => {
            tracing::info!("❌ Failed to get TWILIO_AUTH_TOKEN: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Get the server URL
    let url = match std::env::var("SERVER_URL") {
        Ok(url) => {
            tracing::info!("✅ Successfully retrieved SERVER_URL");
            url + "/api/sms/server"
        },
        Err(e) => {
            tracing::info!("❌ Failed to get SERVER_URL: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Get request body for validation
    let (parts, body) = request.into_parts();
    let body_bytes = match to_bytes(body, 1024 * 1024).await {  // 1MB limit
        Ok(bytes) => {
            tracing::info!("✅ Successfully read request body");
            bytes
        },
        Err(e) => {
            tracing::info!("❌ Failed to read request body: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Convert body to string and parse form data
    let params_str = match String::from_utf8(body_bytes.to_vec()) {
        Ok(s) => s,
        Err(e) => {
            tracing::info!("❌ Failed to convert body to UTF-8: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Parse form parameters into a sorted map
    let params: BTreeMap<String, String> = form_urlencoded::parse(params_str.as_bytes())
        .into_owned()
        .collect();

    // Build the string to sign
    let mut string_to_sign = url;
    for (key, value) in params.iter() {
        string_to_sign.push_str(key);
        string_to_sign.push_str(value);
    }

    tracing::info!("Built string to sign");

    // Create HMAC-SHA1
    let mut mac = match Hmac::<Sha1>::new_from_slice(auth_token.as_bytes()) {
        Ok(mac) => mac,
        Err(e) => {
            tracing::info!("❌ Failed to create HMAC: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    mac.update(string_to_sign.as_bytes());

    // Get the result and encode as base64
    let result = BASE64.encode(mac.finalize().into_bytes());

    // Compare signatures
    if result != signature {
        tracing::info!("❌ Signature validation failed");
        return Err(StatusCode::UNAUTHORIZED);
    }

    tracing::info!("✅ Signature validation successful");

    // Rebuild request and pass to next handler
    let request = Request::from_parts(parts, Body::from(params_str));

    Ok(next.run(request).await)
}

pub async fn send_conversation_message(
    conversation_sid: &str, 
    twilio_number: &String,
    body: &str
) -> Result<String, Box<dyn Error>> {
    let account_sid = env::var("TWILIO_ACCOUNT_SID")?;
    let auth_token = env::var("TWILIO_AUTH_TOKEN")?;
    let client = Client::new();

    let form_data = vec![
        ("Body", body), 
        ("Author", "lightfriend"), // Use a consistent author name
        ("From", twilio_number),   // Specify the actual phone number in From field
        ("X-Twilio-Webhook-Enabled", "true") // Enable webhooks for delivery status
    ];

    let response: MessageResponse = client
        .post(format!(
            "https://conversations.twilio.com/v1/Conversations/{}/Messages",
            conversation_sid
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .form(&form_data)
        .send()
        .await?
        .json()
        .await?;
    println!("successfully sent the conversation message with SID: {}", response.sid);
    tracing::info!("Message sent successfully to conversation {} with message SID: {}", conversation_sid, response.sid);
    println!("successfully sent the conversation message with SID: {}", response.sid);
    tracing::info!("Message sent successfully to conversation {} with message SID: {}", conversation_sid, response.sid);

    Ok(response.sid)
}


