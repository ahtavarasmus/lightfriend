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
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize)]
struct GroqRequest {
    messages: Vec<ChatMessage>,
    model: String,
}

#[derive(Debug, Deserialize)]
struct GroqResponse {
    choices: Vec<GroqChoice>,
}

#[derive(Debug, Deserialize)]
struct GroqChoice {
    message: ChatMessage,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct TwilioMessageResponse {
    sid: String,
    conversation_sid: String,
    body: String,
    author: String,
}

#[derive(Debug, Deserialize)]
struct TwilioMessagesResponse {
    messages: Vec<TwilioMessageResponse>,
}

async fn fetch_conversation_messages(conversation_sid: &str) -> Result<Vec<TwilioMessageResponse>, Box<dyn Error>> {
    let account_sid = env::var("TWILIO_ACCOUNT_SID")?;
    let auth_token = env::var("TWILIO_AUTH_TOKEN")?;

    let client = Client::new();
    let url = format!(
        "https://conversations.twilio.com/v1/Conversations/{}/Messages",
        conversation_sid
    );

    let response = client
        .get(&url)
        .basic_auth(&account_sid, Some(&auth_token))
        .query(&[("Order", "desc"), ("PageSize", "20")])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to fetch messages: {}", response.status()).into());
    }

    let messages_response: TwilioMessagesResponse = response.json().await?;
    Ok(messages_response.messages)
}

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

    let result = async {
        let user = match state.user_repository.find_by_phone_number(&payload.from) {
            Ok(Some(user)) => user,
            Ok(None) => return (
                StatusCode::NOT_FOUND,
                axum::Json(TwilioResponse {
                    message: "User not found".to_string(),
                })
            ),
            Err(e) => {
                eprintln!("Database error: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(TwilioResponse {
                        message: "Database error".to_string(),
                    })
                );
            }
        };



        let conversation = match state.user_conversations.ensure_conversation_exists(&user, Some(payload.to)).await {
            Ok(conv) => conv,
            Err(e) => {
                eprintln!("Failed to ensure conversation exists: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(TwilioResponse {
                        message: "Failed to create conversation".to_string(),
                    })
                );
            }
        };

        // Fetch conversation messages
        let messages = match fetch_conversation_messages(&conversation.conversation_sid).await {
            Ok(msgs) => msgs,
            Err(e) => {
                eprintln!("Failed to fetch conversation messages: {}", e);
                Vec::new()
            }
        };

        // Start with the system message
        let mut chat_messages: Vec<ChatMessage> = vec![ChatMessage {
            role: "system".to_string(),
            content: "You are a friendly and helpful AI assistant named Lightfriend. You provide concise, accurate, and helpful responses while maintaining a warm and engaging tone. You should be direct in your answers while still being conversational and natural. Since users are interacting with you through SMS on basic phones, keep your responses clear, and avoid suggesting actions that require smartphones or internet access. Use simple language and break complex information into digestible parts. Make sure you answer the user's question throughly since they are paying for each response.".to_string(),
        }];
        
        // Add the conversation history
        let mut history: Vec<ChatMessage> = messages.into_iter().map(|msg| {
            ChatMessage {
                role: if msg.author == "lightfriend" { "assistant" } else { "user" }.to_string(),
                content: msg.body,
            }
        }).collect();
        history.reverse();
        
        // Combine system message with conversation history
        chat_messages.extend(history);

        // Print formatted messages for debugging
        for msg in &chat_messages {
            println!("Formatted message - Role: {}, Content: {}", msg.role, msg.content);
        }

        // Send messages to Groq API
        let groq_api_key = env::var("GROQ_API_KEY").expect("GROQ_API_KEY not set");

        let client = Client::new();
        let groq_request = GroqRequest {
            messages: chat_messages,
            model: "llama-3.3-70b-versatile".to_string(),
        };
        let response = match client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", groq_api_key))
            .header("Content-Type", "application/json")
            .json(&groq_request)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("Failed to send request to Groq: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(TwilioResponse {
                        message: "Failed to get AI response".to_string(),
                    })
                );
            }
        };

        if !response.status().is_success() {
            eprintln!("Groq API error: {}", response.status());
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(TwilioResponse {
                    message: "Failed to get AI response".to_string(),
                })
            );
        }

        let groq_response: GroqResponse = match response.json().await {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("Failed to parse Groq response: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(TwilioResponse {
                        message: "Failed to process AI response".to_string(),
                    })
                );
            }
        };

        
        let body = groq_response.choices[0].message.content.clone();
        println!("Sending response to user: {}", body);

        match send_conversation_message(&conversation.conversation_sid, &body).await {
            Ok(_) => (
                StatusCode::OK,
                axum::Json(TwilioResponse {
                    message: "Message received and processed!".to_string(),
                })
            ),
            Err(e) => {
                eprintln!("Failed to send conversation message: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(TwilioResponse {
                        message: "Failed to send message".to_string(),
                    })
                )
            }
        }
    }.await;

    result
}

