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

#[derive(Deserialize)]
pub struct ConversationsListResponse {
    conversations: Vec<ConversationResponse>,
}

#[derive(Deserialize)]
pub struct MessagesListResponse {
    messages: Vec<MessageInfo>,
}

#[derive(Deserialize)]
pub struct MessageInfo {
    author: String,
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

pub async fn fetch_conversation_participants(user: &User, conversation_sid: &str) -> Result<Vec<ParticipantResponse>, Box<dyn Error>> {
    // Check if user has SMS discount tier and use their credentials if they do
    let (account_sid, auth_token) = if user.discount_tier.as_deref() == Some("msg") {
        (
            env::var(format!("TWILIO_ACCOUNT_SID_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_ACCOUNT_SID_{}", user.id))?,
            env::var(format!("TWILIO_AUTH_TOKEN_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_AUTH_TOKEN_{}", user.id))?,
        )
    } else {
        (
            env::var("TWILIO_ACCOUNT_SID")?,
            env::var("TWILIO_AUTH_TOKEN")?,
        )
    };

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


pub async fn delete_bot_conversations(user_phone: &str, user: &User) -> Result<(), Box<dyn Error>> {
    // Check if user has SMS discount tier and use their credentials if they do
    let (account_sid, auth_token) = if user.discount_tier.as_deref() == Some("msg") {
        (
            env::var(format!("TWILIO_ACCOUNT_SID_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_ACCOUNT_SID_{}", user.id))?,
            env::var(format!("TWILIO_AUTH_TOKEN_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_AUTH_TOKEN_{}", user.id))?,
        )
    } else {
        (
            env::var("TWILIO_ACCOUNT_SID")?,
            env::var("TWILIO_AUTH_TOKEN")?,
        )
    };
    let client = Client::new();

    // Fetch all conversations
    let response: ConversationsListResponse = client
        .get("https://conversations.twilio.com/v1/Conversations")
        .basic_auth(&account_sid, Some(&auth_token))
        .send()
        .await?
        .json()
        .await?;

    let mut deleted_count = 0;
    for conversation in response.conversations {
        // Check if user is a participant in this conversation
        let participants = fetch_conversation_participants(&user, &conversation.sid).await?;
        let user_is_participant = participants.iter().any(|p| {
            if let Some(binding) = &p.messaging_binding {
                if let Some(address) = &binding.address {
                    return address == user_phone;
                }
            }
            false
        });

        if user_is_participant {
            // Verify this is a conversation with the bot by checking messages
            let messages: MessagesListResponse = client
                .get(format!(
                    "https://conversations.twilio.com/v1/Conversations/{}/Messages",
                    conversation.sid
                ))
                .basic_auth(&account_sid, Some(&auth_token))
                .send()
                .await?
                .json()
                .await?;

            // Check if any message is from "lightfriend" (our bot)
            let has_bot_messages = messages.messages.iter().any(|msg| msg.author == "lightfriend");

            if has_bot_messages {
                match delete_twilio_conversation(&conversation.sid, &user).await {
                    Ok(_) => {
                        deleted_count += 1;
                        tracing::debug!("Deleted conversation: {}", conversation.sid);
                    }
                    Err(e) => {
                        eprintln!("Failed to delete conversation {}: {}", conversation.sid, e);
                    }
                }
            }
        }
    }

    tracing::debug!("Successfully deleted {} conversations for user {}", deleted_count, user_phone);
    Ok(())
}

pub async fn delete_twilio_conversation(conversation_sid: &str, user: &User) -> Result<(), Box<dyn Error>> {
    // Check if user has SMS discount tier and use their credentials if they do
    let (account_sid, auth_token) = if user.discount_tier.as_deref() == Some("msg") {
        (
            env::var(format!("TWILIO_ACCOUNT_SID_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_ACCOUNT_SID_{}", user.id))?,
            env::var(format!("TWILIO_AUTH_TOKEN_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_AUTH_TOKEN_{}", user.id))?,
        )
    } else {
        (
            env::var("TWILIO_ACCOUNT_SID")?,
            env::var("TWILIO_AUTH_TOKEN")?,
        )
    };
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

    tracing::debug!("Successfully deleted conversation: {}", conversation_sid);
    Ok(())
}


pub async fn create_twilio_conversation_for_participant(user: &User, proxy_address: String) -> Result<(String, String), Box<dyn Error>> {
    // Validate phone numbers
    if !user.phone_number.starts_with("+") {
        return Err("Invalid user phone number format - must start with +".into());
    }

    if !proxy_address.starts_with("+") {
        return Err("Invalid proxy address format - must start with +".into());
    }

    // Check if user has SMS discount tier and use their credentials if they do
    let (account_sid, auth_token) = if user.discount_tier.as_deref() == Some("msg") {
        (
            env::var(format!("TWILIO_ACCOUNT_SID_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_ACCOUNT_SID_{}", user.id))?,
            env::var(format!("TWILIO_AUTH_TOKEN_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_AUTH_TOKEN_{}", user.id))?,
        )
    } else {
        (
            env::var("TWILIO_ACCOUNT_SID")?,
            env::var("TWILIO_AUTH_TOKEN")?,
        )
    };
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
        tracing::error!(
            "Failed to add participant. Status: {}, Error: {}",
            status,
            error_text
        );
        
        // Handle specific error cases
        match status.as_u16() {
            409 => {
                tracing::debug!("Participant already exists in a conversation");
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
                    
                    tracing::debug!("Found existing conversation: {}", conv_sid);
                    return Ok((conv_sid, conversation.chat_service_sid));
                }
            }
            400 => {
                if error_text.contains("Invalid messaging binding address") {
                    return Err("Invalid phone number format. Please ensure both numbers are in E.164 format (+1234567890)".into());
                }
            }
            401 => return Err("Authentication failed with Twilio. Please check your credentials.".into()),
            403 => return Err("Permission denied. Please check your Twilio account permissions.".into()),
            _ => {}
        }
        
        return Err(format!("Failed to add participant: {}", error_text).into());
    }

    tracing::debug!("Successfully added participant to conversation");

    Ok((conversation.sid, conversation.chat_service_sid))
}


pub async fn validate_user_twilio_signature(
    request: Request<Body>,
    next: middleware::Next,
) -> Result<Response, StatusCode> {
    tracing::info!("\n=== Starting User-Specific Twilio Signature Validation ===");

    // Extract user_id from the request path
    let path = request.uri().path();
    let user_id = match path.split('/').last() {
        Some(id) => match id.parse::<i32>() {
            Ok(id) => {
                tracing::info!("✅ Successfully extracted user_id: {}", id);
                id
            },
            Err(e) => {
                tracing::error!("❌ Failed to parse user_id: {}", e);
                return Err(StatusCode::BAD_REQUEST);
            }
        },
        None => {
            tracing::error!("❌ No user_id found in path");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Get the Twilio signature from headers
    let signature = match request.headers().get("X-Twilio-Signature") {
        Some(header) => match header.to_str() {
            Ok(s) => {
                tracing::info!("✅ Successfully retrieved X-Twilio-Signature");
                s.to_string()
            },
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

    // Get the user-specific Twilio auth token
    let auth_token = match std::env::var(format!("TWILIO_AUTH_TOKEN_{}", user_id)) {
        Ok(token) => {
            tracing::info!("✅ Successfully retrieved user-specific TWILIO_AUTH_TOKEN");
            token
        },
        Err(e) => {
            tracing::error!("❌ Failed to get user-specific TWILIO_AUTH_TOKEN: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Get the server URL
    let url = match std::env::var("SERVER_URL") {
        Ok(url) => {
            tracing::info!("✅ Successfully retrieved SERVER_URL");
            format!("{}/api/sms/server/{}", url, user_id)
        },
        Err(e) => {
            tracing::error!("❌ Failed to get SERVER_URL: {}", e);
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
                tracing::error!("❌ Error converting X-Twilio-Signature to string: {}", e);
                return Err(StatusCode::UNAUTHORIZED);
            }
        },
        None => {
            tracing::error!("❌ No X-Twilio-Signature header found");
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
            tracing::error!("❌ Failed to get TWILIO_AUTH_TOKEN: {}", e);
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
            tracing::error!("❌ Failed to get SERVER_URL: {}", e);
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



use openai_api_rs::v1::{
    chat_completion,
    types,
    api::OpenAIClient,
    common::GPT4_O,
};

// Function to redact sensitive information from the message body
async fn redact_sensitive_info(body: &str) -> String {
    // First apply regex-based redaction for basic PII
    let re_email = regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap();
    let re_phone = regex::Regex::new(r"\b\d{3}-\d{3}-\d{4}\b").unwrap();
    
    let redacted = re_email.replace_all(body, "[EMAIL_REDACTED]");
    let redacted = re_phone.replace_all(&redacted, "[PHONE_REDACTED]");
    let initial_redacted = redacted.into_owned();

    // Use LLM for advanced content redaction
    let api_key = match std::env::var("OPENROUTER_API_KEY") {
        Ok(key) => key,
        Err(e) => {
            eprintln!("Failed to get OpenRouter API key: {}", e);
            return initial_redacted;
        }
    };

    let client = match OpenAIClient::builder()
        .with_endpoint("https://openrouter.ai/api/v1")
        .with_api_key(api_key)
        .build() {
            Ok(client) => client,
            Err(e) => {
                eprintln!("Failed to build OpenAI client for redaction: {}", e);
                return initial_redacted;
            }
        };

    let system_prompt = "You are a privacy-focused content redaction system. Your task is to identify and redact sensitive information while preserving the context and meaning of messages. 

    Redact the following types of content:
    1. Personal identifiable information (PII)
    2. Private message content from WhatsApp or other messaging platforms
    3. Specific email content or details
    4. Calendar event details (except general timing)
    5. Task/todo content details
    6. Location information (except general areas)
    7. Financial information
    8. Medical information
    9. Personal relationships and private life details
    10. Credentials or access-related information

    Guidelines:
    - Replace sensitive content with descriptive placeholders like [WHATSAPP_MESSAGE_REDACTED], [CALENDAR_EVENT_REDACTED], etc.
    - Preserve general context and conversation flow
    - Keep non-sensitive words and general topics intact
    - Maintain message structure and readability
    - When in doubt, err on the side of privacy
    
    Example:
    Input: 'WhatsApp message from John: I'll meet you at 123 Main St at 3pm tomorrow for the doctor's appointment about my knee surgery'
    Output: 'WhatsApp message from [NAME_REDACTED]: I'll meet you at [LOCATION_REDACTED] at 3pm tomorrow for [MEDICAL_APPOINTMENT_REDACTED]'";

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(system_prompt.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(initial_redacted.clone()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let req = chat_completion::ChatCompletionRequest::new(
        GPT4_O.to_string(),
        messages,
    )
    .max_tokens(150);

    match client.chat_completion(req).await {
        Ok(result) => {
            result.choices[0].message.content.clone().unwrap_or(initial_redacted)
        },
        Err(e) => {
            eprintln!("Failed to get LLM redaction: {}", e);
            initial_redacted
        }
    }
}

// Function to update (redact) a message in Twilio
pub async fn redact_message(
    conversation_sid: &str,
    message_sid: &str,
    redacted_body: &str,
    user: &User,
) -> Result<(), Box<dyn Error>> {
    // Check if user has SMS discount tier and use their credentials if they do
    let (account_sid, auth_token) = if user.discount_tier.as_deref() == Some("msg") {
        (
            env::var(format!("TWILIO_ACCOUNT_SID_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_ACCOUNT_SID_{}", user.id))?,
            env::var(format!("TWILIO_AUTH_TOKEN_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_AUTH_TOKEN_{}", user.id))?,
        )
    } else {
        (
            env::var("TWILIO_ACCOUNT_SID")?,
            env::var("TWILIO_AUTH_TOKEN")?,
        )
    };
    let client = Client::new();

    // 1. Update the conversation message body
    let form_data = vec![("Body", redacted_body)];

    let response = client
        .post(format!(
            "https://conversations.twilio.com/v1/Conversations/{}/Messages/{}",
            conversation_sid, message_sid
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .form(&form_data)
        .send()
        .await?;

    if !response.status().is_success() {
        tracing::error!("Failed to update conversation message {}: {}", message_sid, response.status());
        return Err(format!("Failed to update conversation message: {}", response.status()).into());
    }

    tracing::debug!("Message {} fully redacted (both conversation and logs)", message_sid);
    Ok(())
}


#[derive(Deserialize)]
pub struct MediaResponse {
    pub sid: String,
}

// Function to upload media to Twilio Media Content Service
pub async fn upload_media_to_twilio(
    chat_service_sid: &str,
    file_data: &[u8],
    content_type: &str,
    filename: &str,
    user: &User,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Check if user has SMS discount tier and use their credentials if they do
    let (account_sid, auth_token) = if user.discount_tier.as_deref() == Some("msg") {
        (
            env::var(format!("TWILIO_ACCOUNT_SID_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_ACCOUNT_SID_{}", user.id))?,
            env::var(format!("TWILIO_AUTH_TOKEN_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_AUTH_TOKEN_{}", user.id))?,
        )
    } else {
        (
            env::var("TWILIO_ACCOUNT_SID")?,
            env::var("TWILIO_AUTH_TOKEN")?,
        )
    };
    let client = Client::new();

    let response = client
        .post(format!(
            "https://mcs.us1.twilio.com/v1/Services/{}/Media",
            chat_service_sid
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .header("Content-Type", content_type)
        .header("Content-Length", file_data.len().to_string())
        .header("Content-Disposition", format!("attachment; filename=\"{}\"", filename))
        .body(file_data.to_vec())
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to upload media to Twilio: {} - {}", response.status(), response.text().await?).into());
    }


    let media_response: MediaResponse = response.json().await?;
    tracing::debug!("Successfully uploaded media to Twilio with SID: {}", media_response.sid);
    
    Ok(media_response.sid)
}


pub async fn delete_twilio_message_media(
    media_sid: &str,
    user: &User,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Check if user has SMS discount tier and use their credentials if they do
    let (account_sid, auth_token) = if user.discount_tier.as_deref() == Some("msg") {
        (
            env::var(format!("TWILIO_ACCOUNT_SID_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_ACCOUNT_SID_{}", user.id))?,
            env::var(format!("TWILIO_AUTH_TOKEN_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_AUTH_TOKEN_{}", user.id))?,
        )
    } else {
        (
            env::var("TWILIO_ACCOUNT_SID")?,
            env::var("TWILIO_AUTH_TOKEN")?,
        )
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
        return Err(format!("Failed to delete message media: {} - {}", 
            response.status(), response.text().await?).into());
    }

    tracing::debug!("Successfully deleted message media: {}", media_sid);
    Ok(())
}

pub async fn delete_media_from_twilio(
    chat_service_sid: &str,
    media_sid: String,
    user: &User,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Check if user has SMS discount tier and use their credentials if they do
    let (account_sid, auth_token) = if user.discount_tier.as_deref() == Some("msg") {
        (
            env::var(format!("TWILIO_ACCOUNT_SID_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_ACCOUNT_SID_{}", user.id))?,
            env::var(format!("TWILIO_AUTH_TOKEN_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_AUTH_TOKEN_{}", user.id))?,
        )
    } else {
        (
            env::var("TWILIO_ACCOUNT_SID")?,
            env::var("TWILIO_AUTH_TOKEN")?,
        )
    };
    let client = Client::new();

    let response = client
        .delete(format!(
            "https://mcs.us1.twilio.com/v1/Services/{}/Media/{}",
            chat_service_sid, media_sid
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to delete media from Twilio: {} - {}", response.status(), response.text().await?).into());
    }

    tracing::debug!("Successfully deleted media from Twilio: {}", media_sid);
    Ok(())
}


pub async fn send_conversation_message(
    conversation_sid: &str, 
    twilio_number: &String,
    body: &str,
    redact: bool,
    media_sid: Option<&String>,
    user: &User,
) -> Result<String, Box<dyn Error>> {
    // If this is a non-free reply message, we should redact previous free reply messages
    if redact && !body.contains("(free reply)") {
        // Fetch recent messages
        let messages = match crate::api::twilio_sms::fetch_conversation_messages(conversation_sid).await {
            Ok(msgs) => msgs,
            Err(e) => {
                eprintln!("Failed to fetch messages for redaction: {}", e);
                vec![] // Continue with sending the message even if fetching fails
            }
        };

        // Find and redact free reply messages until we hit a non-free reply message
        for msg in messages {
            if msg.author == "lightfriend" && msg.body.contains("(free reply)") {
                let redacted_body = redact_sensitive_info(&msg.body).await;
                if let Err(e) = redact_message(conversation_sid, &msg.sid, &redacted_body, &user).await {
                    eprintln!("Failed to redact free reply message {}: {}", msg.sid, e);
                }
            } else if msg.author == "lightfriend" {
                // Stop when we hit a non-free reply message
                break;
            }
        }
    }

    // Check if user has SMS discount tier and use their credentials if they do
    let (account_sid, auth_token) = if user.discount_tier.as_deref() == Some("msg") {
        (
            env::var(format!("TWILIO_ACCOUNT_SID_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_ACCOUNT_SID_{}", user.id))?,
            env::var(format!("TWILIO_AUTH_TOKEN_{}", user.id))
                .map_err(|_| format!("Missing TWILIO_AUTH_TOKEN_{}", user.id))?,
        )
    } else {
        (
            env::var("TWILIO_ACCOUNT_SID")?,
            env::var("TWILIO_AUTH_TOKEN")?,
        )
    };

    let client = Client::new();

    // Build form data with required fields
    let mut form_data = vec![
        ("Body", body), 
        ("Author", "lightfriend"),
        ("From", twilio_number),
    ];

    // Add media_sid if provided
    if let Some(media_sid) = media_sid {
        form_data.push(("MediaSid", media_sid));
    }

    let response: MessageResponse = client
        .post(format!(
            "https://conversations.twilio.com/v1/Conversations/{}/Messages",
            conversation_sid
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .header("X-Twilio-Webhook-Enabled", "true")
        .form(&form_data)
        .send()
        .await?
        .json()
        .await?;

    tracing::debug!("Successfully sent conversation message{} with SID: {}", 
        if media_sid.is_some() { " with media" } else { "" },
        response.sid);

    // Only redact the current message if it's not a free reply message
    if redact && !body.contains("(free reply)") {
        // Redact sensitive information using enhanced LLM-based redaction
        let redacted_body = redact_sensitive_info(&body).await;
        
        // Update the message with the redacted body
        redact_message(conversation_sid, &response.sid, &redacted_body, &user).await?;
    }

    Ok(response.sid)
}


