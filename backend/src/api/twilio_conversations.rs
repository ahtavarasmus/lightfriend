use reqwest::Client;
use serde::Deserialize;
use std::env;
use crate::models::user_models::User;
use std::error::Error;

#[derive(Deserialize)]
struct ConversationResponse {
    sid: String,
    chat_service_sid: String,
}

pub async fn setup_conversation(user: &User) -> Result<(String, String), Box<dyn Error>> {
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

    // Add the user as participant
    client
        .post(format!(
            "https://conversations.twilio.com/v1/Conversations/{}/Participants",
            conversation.sid
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .form(&[
            ("MessagingBinding.Address", &user.phone_number),
            (
                "MessagingBinding.ProxyAddress",
                user.preferred_number.as_ref().unwrap(),
            ),
        ])
        .send()
        .await?;

    Ok((conversation.sid, conversation.chat_service_sid))
}


