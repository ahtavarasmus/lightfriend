use reqwest::Client;
use std::env;
use std::error::Error;
use std::sync::Arc;
use crate::AppState;
use crate::api::twilio_utils::send_conversation_message;
use serde::{Deserialize, Serialize};
use axum::{
    extract::Form,
    response::IntoResponse,
    extract::State,
    http::StatusCode,
};

#[derive(Deserialize)]
pub struct TwilioWebhookPayload {
    #[serde(rename = "From")]
    from: String,
    #[serde(rename = "To")]
    to: String,
    #[serde(rename = "Body")]
    body: String,
}

#[derive(Serialize)]
struct TwilioResponse {
    #[serde(rename = "Message")]
    message: String,
}

pub async fn send_sms(to: &str, body: &str) -> Result<(), Box<dyn Error>> {
    let account_sid = env::var("TWILIO_ACCOUNT_SID")?;
    let auth_token = env::var("TWILIO_AUTH_TOKEN")?;
    let from = env::var("FIN_PHONE")?;

    let client = Client::new();
    let response = client
        .post(format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json",
            account_sid
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .form(&[
            ("To", to),
            ("From", &from),
            ("Body", body),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to send SMS: {}", response.status()).into());
    }

    Ok(())
}

pub async fn handle_incoming_sms(
    State(state): State<Arc<AppState>>,
    Form(payload): Form<TwilioWebhookPayload>,
) -> impl IntoResponse {
    println!("Received SMS from: {} to: {}", payload.from, payload.to);
    println!("Message: {}", payload.body);

    // Find or create conversation
    let result = handle_conversation(&state, &payload.from, &payload.to).await;

    match result {
        Ok(_) => {
            let response = TwilioResponse {
                message: "Message received and processed!".to_string(),
            };
            (StatusCode::OK, axum::Json(response))
        }
        Err(e) => {
            eprintln!("Error processing message: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(TwilioResponse {
                    message: "Error processing message".to_string(),
                })
            )
        }
    }
}

async fn handle_conversation(
    state: &Arc<AppState>,
    from_number: &str,
    twilio_number: &str,
) -> Result<(), Box<dyn Error>> {

    // Find user by phone number
    let user = match state.user_repository.find_by_phone_number(from_number)? {
        Some(user) => user,
        None => return Err("User not found".into()),
    };

    // Try to find an active conversation
    let conversation = match state.user_conversations.find_active_conversation(user.id)? {
        Some(conversation) => conversation,
        None => {
            state.user_conversations.create_conversation_for_user(&user, Some(twilio_number.to_string())).await?
        }
    };

    // Send message and handle potential errors
    if let Err(err) = send_conversation_message(&conversation.conversation_sid, "Hello!").await {
        eprintln!("Failed to send conversation message: {}", err);
        return Err(format!("Failed to send message in conversation: {}", err).into());
    }

    Ok(())
}
