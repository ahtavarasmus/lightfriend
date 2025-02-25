use reqwest::Client;
use serde::Deserialize;
use std::env;
use crate::models::user_models::User;
use std::error::Error;

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


pub async fn send_conversation_message(
    conversation_sid: &str, 
    twilio_number: &String,
    body: &str
) -> Result<String, Box<dyn Error>> {
    let account_sid = env::var("TWILIO_ACCOUNT_SID")?;
    let auth_token = env::var("TWILIO_AUTH_TOKEN")?;
    let client = Client::new();

    let form_data = vec![("Body", body), ("Author", twilio_number)];

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
    println!("successfully sent the conversation message");

    Ok(response.sid)
}


