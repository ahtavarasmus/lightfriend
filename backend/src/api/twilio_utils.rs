use reqwest::Client;
use serde::Deserialize;
use std::env;
use crate::models::user_models::User;
use std::error::Error;

#[derive(Deserialize)]
struct MessageResponse {
    sid: String,
}

#[derive(Deserialize)]
struct ConversationResponse {
    sid: String,
    chat_service_sid: String,
}

pub async fn setup_conversation(user: &User, twilio_number: Option<String>) -> Result<(String, String), Box<dyn Error>> {
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

    // Use provided Twilio number or fall back to user's preferred number
    let proxy_address = twilio_number.unwrap_or_else(|| 
        user.preferred_number.as_ref()
            .expect("User must have either a preferred number or a provided Twilio number")
            .clone()
    );

    // Add the user as participant
    client
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

    Ok((conversation.sid, conversation.chat_service_sid))
}

pub async fn send_conversation_message(
    conversation_sid: &str, 
    body: &str
) -> Result<String, Box<dyn Error>> {
    let account_sid = env::var("TWILIO_ACCOUNT_SID")?;
    let auth_token = env::var("TWILIO_AUTH_TOKEN")?;
    let client = Client::new();

    let form_data = vec![("Body", body), ("Author", "lightfriend")];

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

    Ok(response.sid)
}


