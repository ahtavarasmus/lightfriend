use reqwest::Client;
use crate::handlers::imap_handlers::ImapError;
use std::env;
use std::error::Error;
use std::sync::Arc;
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use axum::{
    extract::Form,
    response::IntoResponse,
    extract::State,
    http::StatusCode,
    Json,
};
use chrono::Utc;


use openai_api_rs::v1::{
    chat_completion,
    types,
    api::OpenAIClient,
    common::GPT4_O,
};


#[derive(Clone, Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: chat_completion::Content,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TwilioMessageResponse {
    pub sid: String,
    pub conversation_sid: String,
    pub body: String,
    pub author: String,
}

#[derive(Debug, Deserialize)]
struct TwilioMessagesResponse {
    messages: Vec<TwilioMessageResponse>,
}

pub async fn fetch_conversation_messages(conversation_sid: &str) -> Result<Vec<TwilioMessageResponse>, Box<dyn Error>> {
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
        .query(&[("Order", "desc"), ("PageSize", "15")])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to fetch messages: {}", response.status()).into());
    }

    let messages_response: TwilioMessagesResponse = response.json().await?;
    Ok(messages_response.messages)
}

#[derive(Deserialize, Clone)]
pub struct MediaItem {
    pub content_type: String,
    pub url: String,
    pub sid: String,
}

#[derive(Deserialize, Clone)]
pub struct TwilioWebhookPayload {
    #[serde(rename = "From")]
    pub from: String,
    #[serde(rename = "To")]
    pub to: String,
    #[serde(rename = "Body")]
    pub body: String,
    #[serde(rename = "NumMedia")]
    pub num_media: Option<String>,
    #[serde(rename = "MediaUrl0")]
    pub media_url0: Option<String>,
    #[serde(rename = "MediaContentType0")]
    pub media_content_type0: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct TwilioResponse {
    #[serde(rename = "Message")]
    pub message: String,
}

#[derive(Deserialize)]
struct WaitingCheckArgs {
    content: String,
    due_date: Option<i64>,
    remove_when_found: Option<bool>,
}

async fn handle_create_waiting_check(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> Result<String, Box<dyn Error>> {
    let args: WaitingCheckArgs = serde_json::from_str(args)?;

    // Calculate default due date (2 weeks from now) if not provided
    let due_date = args.due_date.unwrap_or_else(|| {
        let two_weeks = chrono::Duration::weeks(2);
        (chrono::Utc::now() + two_weeks).timestamp()
    }) as i32;

    // Default remove_when_found to true if not provided
    let remove_when_found = args.remove_when_found.unwrap_or(true);

    let new_check = crate::models::user_models::NewWaitingCheck {
        user_id,
        due_date,
        content: args.content,
        remove_when_found,
        service_type: "imap".to_string(), // Default to email service type
    };

    state.user_repository.create_waiting_check(&new_check)?;

    Ok("I'll keep an eye out for that in your emails and notify you when I find it.".to_string())
}

pub async fn send_shazam_answer_to_user(
    state: Arc<crate::shazam_call::ShazamState>,
    user_id: i32,
    message: &str,
    success: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Add check for user's subscription status
    tracing::info!("Starting send_shazam_answer_to_user for user_id: {}", user_id);
    tracing::info!("Message to send: {}", message);

    let user = match state.user_repository.find_by_id(user_id) {
        Ok(Some(user)) => {
            tracing::info!("Found user with phone number: {}", user.phone_number);
            user
        },
        Ok(None) => {
            tracing::info!("User not found with id: {}", user_id);
            return Err("User not found".into());
        },
        Err(e) => {
            eprintln!("Database error while finding user {}: {}", user_id, e);
            return Err(Box::new(e));
        },
    };
    let message_credits_cost = if user.phone_number.starts_with("+1") {
        std::env::var("MESSAGE_COST_US")
            .unwrap_or_else(|_| std::env::var("MESSAGE_COST").expect("MESSAGE_COST not set"))
            .parse::<f32>()
            .unwrap_or(0.10)
    } else {
        std::env::var("MESSAGE_COST")
            .expect("MESSAGE_COST not set")
            .parse::<f32>()
            .unwrap_or(0.15)
    };

    tracing::info!("Determining sender number for user {}", user_id);
    let sender_number = match user.preferred_number.clone() {
        Some(number) => {
            tracing::info!("Using user's preferred number: {}", number);
            number
        },
        None => {
            let number = std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set");
            tracing::info!("Using default SHAZAM_PHONE_NUMBER: {}", number);
            number
        },
    };

    tracing::info!("Getting conversation for user {} with sender number {}", user_id, sender_number);
    // First check if there are any existing conversations
    let conversation = state
        .user_conversations
        .get_conversation(&user, sender_number.to_string())
        .await?;

    // Verify the conversation is still active by fetching participants
    let participants = crate::api::twilio_utils::fetch_conversation_participants(&user, &conversation.conversation_sid).await?;
    
    if participants.is_empty() {
        tracing::error!("No active participants found in conversation {}", conversation.conversation_sid);
        return Err("No active participants in conversation".into());
    }

    // Check if the user's number is still active in the conversation
    let user_participant = participants.iter().find(|p| {
        p.messaging_binding.as_ref()
            .and_then(|b| b.address.as_ref())
            .map_or(false, |addr| addr == &user.phone_number)
    });

    if user_participant.is_none() {
        tracing::error!("User {} is no longer active in conversation {}", user.phone_number, conversation.conversation_sid);
        return Err("User is no longer active in conversation".into());
    }
    tracing::info!("Retrieved conversation with SID: {}", conversation.conversation_sid);

    tracing::info!("Sending message to conversation {}", conversation.conversation_sid);
    match crate::api::twilio_utils::send_conversation_message(
        &conversation.conversation_sid,
        &conversation.twilio_number,
        message,
        true,
        &user,
    )
    .await {
        Ok(message_sid) => {

            // Deduct credits for the message
            if let Err(e) = state.user_repository
                .update_user_credits(user.id, user.credits - message_credits_cost) {
                eprintln!("Failed to update user credits after Shazam message: {}", e);
                return Err("Failed to process credits points".into());
            }

            // Log the SMS usage
            if let Err(e) = state.user_repository.log_usage(
                user.id,
                Some(message_sid),
                "sms".to_string(),
                Some(message_credits_cost),
                None,
                Some(success),
                Some("shazam response".to_string()),
                None,
                None,
                None,
            ) {
                eprintln!("Failed to log Shazam SMS usage: {}", e);
                // Continue execution even if logging fails
            }
        }
        Err(e) => {
            tracing::error!("Failed to send conversation message: {}", e);
            return Err("Failed to send shazam response to the user".into());
        }
    }

    /* TODO have to make this whole charging thing in a separate function at some point
    // Check if credits are under threshold and handle automatic charging
    match state.user_repository.is_credits_under_threshold(user.id) {
        Ok(is_under) => {
            if is_under && user.charge_when_under {
                println!("User {} credits is under threshold after Shazam message, attempting automatic charge", user.id);
                use axum::extract::{State, Path};
                let state_clone = Arc::clone(&state);
                tokio::spawn(async move {
                    let _ = crate::handlers::stripe_handlers::automatic_charge(
                        State(state_clone),
                        Path(user.id),
                    ).await;
                });
                println!("Initiated automatic recharge for user after Shazam message");
            }
        },
        Err(e) => eprintln!("Failed to check if user credits is under threshold after Shazam message: {}", e),
    }
    */

    Ok(())
}



// New wrapper handler for the regular SMS endpoint
pub async fn handle_regular_sms(
    State(state): State<Arc<AppState>>,
    Form(payload): Form<TwilioWebhookPayload>,
) -> (StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<TwilioResponse>) {
    // First check if this user has a discount_tier == sms - they shouldn't be using this endpoint, but their own dedicated
    match state.user_repository.find_by_phone_number(&payload.from) {
        Ok(Some(user)) => {
            if let Some(tier) = user.discount_tier {
                if tier == "msg".to_string() {
                    tracing::warn!("User {} with discount_tier equal to msg attempted to use regular SMS endpoint", user.id);
                    return (
                        StatusCode::FORBIDDEN,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        axum::Json(TwilioResponse {
                            message: "Please use your dedicated SMS endpoint. Contact support if you need help.".to_string(),
                        })
                    );
                }
            }
        },
        Ok(None) => {
            tracing::error!("No user found for phone number: {}", payload.from);
            return (
                StatusCode::NOT_FOUND,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "User not found".to_string(),
                })
            );
        },
        Err(e) => {
            tracing::error!("Database error while finding user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Internal server error".to_string(),
                })
            );
        }
    }

    // If we get here, the user is allowed to use this endpoint
    handle_incoming_sms(State(state), Form(payload)).await
}

// Original handler becomes internal and is used by both routes
pub async fn handle_incoming_sms(
    State(state): State<Arc<AppState>>,
    Form(payload): Form<TwilioWebhookPayload>,
) -> (StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<TwilioResponse>) {
    println!("Received SMS from: {} to: {}", payload.from, payload.to);

    // If this is a media-only webhook (no body but has media), skip processing
    if payload.body.trim().is_empty() && payload.num_media.as_ref().map_or(false, |n| n != "0") {
        println!("only media, skipping..");
        /*
        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            axum::Json(TwilioResponse {
                message: "Media-only webhook acknowledged".to_string(),
            })
        );
        */
    }

    // Check for Shazam shortcut ('S' or 's')
    if payload.body.trim() == "S" || payload.body.trim() == "s" {

        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            axum::Json(TwilioResponse {
                message: "The Shazam feature has been discontinued due to insufficient usage. Thank you for your understanding.".to_string(),
            })
        );
    }

    // Check for STOP command
    if payload.body.trim().to_uppercase() == "STOP" {
        if let Ok(Some(user)) = state.user_repository.find_by_phone_number(&payload.from) {
            if let Err(e) = state.user_repository.update_notify(user.id, false) {
                eprintln!("Failed to update notify status: {}", e);
            } else {
                return (
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(TwilioResponse {
                        message: "You have been unsubscribed from notifications.".to_string(),
                    })
                );
            }
        }
    }

    // Process SMS in the background
    tokio::spawn(async move {
        let result = process_sms(&state, payload.clone(), false).await;
        
        if result.0 != StatusCode::OK {
            eprintln!("Background SMS processing failed with status: {:?}", result.0);
            eprintln!("Error response: {:?}", result.1);
        }
    });
    

    // Immediately return a success response to Twilio
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        axum::Json(TwilioResponse {
            message: "Message received, processing in progress".to_string(),
        })
    )
}

// Helper function to check if a tool is accessible based on user's status
fn requires_subscription(tool_name: &str, sub_tier: Option<String>, has_discount: bool) -> bool {
    println!("\n=== Subscription Check Details ===");
    println!("Tool name: {}", tool_name);
    println!("Has subscription: {:#?}", sub_tier);
    println!("Has discount: {}", has_discount);
    
    // Tier 2 subscribers get access to everything
    if Some("tier 2".to_string()) == sub_tier || has_discount {
        println!("✅ User has tier 2 subscription - granting full access");
        return false;
    } else if Some("tier 1".to_string()) == sub_tier {
        let in_allowed_tools = tool_name.contains("perplexity") ||
            tool_name.contains("weather") ||
            tool_name.contains("assistant");

        if in_allowed_tools {
            return false;
        }
    }
    return true;
}


// Helper function to get subscription error message
fn get_subscription_error(tool_name: &str) -> String {
    format!("This feature ({}) requires a subscription. Please visit our website to subscribe.", tool_name
    )
}

pub async fn process_sms(
    state: &Arc<AppState>,
    payload: TwilioWebhookPayload,
    is_test: bool,
) -> (StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<TwilioResponse>) {
    let start_time = std::time::Instant::now(); // Track processing time

    let user = match state.user_repository.find_by_phone_number(&payload.from) {
        Ok(Some(user)) => {
            tracing::info!("Found user with ID: {} for phone number: {}", user.id, payload.from);
            
            // Print image information if present (for admin only)
            if let (Some(num_media), Some(media_url), Some(content_type)) = (
                payload.num_media.as_ref(),
                payload.media_url0.as_ref(),
                payload.media_content_type0.as_ref()
            ) {
                if user.id == 1 {
                    println!("Media information:");
                    println!("  Number of media items: {}", num_media);
                    println!("  Media URL: {}", media_url);
                    println!("  Content type: {}", content_type);
                }
            }
            
            user
        },
        Ok(None) => {
            tracing::error!("No user found for phone number: {}", payload.from);
                return (
                    StatusCode::NOT_FOUND,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(TwilioResponse {
                        message: "User not found".to_string(),
                    })
                );
        },
        Err(e) => {
            tracing::error!("Database error while finding user for phone number {}: {}", payload.from, e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(TwilioResponse {
                        message: "Database error".to_string(),
                    })
                );
        }
    };

    let conversation = match state.user_conversations.get_conversation(&user, payload.to).await {
        Ok(conv) => conv,
        Err(e) => {
            eprintln!("Failed to ensure conversation exists: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(TwilioResponse {
                        message: "Failed to create conversation".to_string(),
                    })
                );
        }
    };

    // Handle WhatsApp message confirmation flow
    let messages = match fetch_conversation_messages(&conversation.conversation_sid).await {
        Ok(msgs) => msgs,
        Err(e) => {
            eprintln!("Failed to fetch conversation messages: {}", e);
            Vec::new()
        }
    };
    println!("messages: {:#?}", messages);

    let last_msg = messages.iter().find(|msg| msg.author == "lightfriend");
    println!("last msg: {:#?}", last_msg);

    // if AI decides it needs more information we should put this as false so next message knows whatsup
    let mut redact_the_body = true;
    if let Some(last_ai_message) = last_msg {
        if user.confirm_send_event {
            println!("last message was confirmation to send event since the flag was set");
            let user_response = payload.body.trim().to_lowercase();
            
            // Check for calendar event confirmation
            if let Some(captures) = regex::Regex::new(r"Confirm creating calendar event: '([^']+)' starting at '([^']+)' for (\d+) minutes(\s*with description: '([^']+)')?.*")
                .ok()
                .and_then(|re| re.captures(&last_ai_message.body)) {

                
                let summary = captures.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                let start_time = captures.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
                let duration = captures.get(3).and_then(|m| m.as_str().parse::<i32>().ok()).unwrap_or_default();
                let description = captures.get(5).map(|m| m.as_str().to_string());

                // Redact the confirmation message
                if let Err(e) = crate::api::twilio_utils::redact_message(
                    &conversation.conversation_sid,
                    &last_ai_message.sid,
                    "Calendar event confirmation message redacted",
                    &user,
                ).await {
                    eprintln!("Failed to redact calendar confirmation message: {}", e);
                }

                match user_response.as_str() {
                    "yes" => {
                        // Reset the confirmation flag
                        if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                            eprintln!("Failed to reset confirm_send_event flag: {}", e);
                        }

                        // Create the calendar event
                        let event_request = crate::handlers::google_calendar::CreateEventRequest {
                            start_time: match chrono::DateTime::parse_from_rfc3339(&start_time) {
                                Ok(dt) => dt.with_timezone(&chrono::Utc),
                                Err(e) => {
                                    eprintln!("Failed to parse start time: {}", e);
                                    if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                        &conversation.conversation_sid,
                                        &conversation.twilio_number,
                                        "Failed to create calendar event due to invalid start time.",
                                        true,
                                        &user,
                                    ).await {
                                        eprintln!("Failed to send error message: {}", e);
                                    }
                                    return (
                                        StatusCode::OK,
                                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                                        axum::Json(TwilioResponse {
                                            message: "Failed to create calendar event".to_string(),
                                        })
                                    );
                                }
                            },
                            duration_minutes: duration,
                            summary,
                            description,
                            add_notification: true,
                        };
 
                        let auth_user = crate::handlers::auth_middleware::AuthUser {
                            user_id: user.id, 
                            is_admin: false,
                        };

                        match crate::handlers::google_calendar::create_calendar_event(
                            State(state.clone()),
                            auth_user,
                            Json(event_request),
                        ).await {
                            Ok(_) => {
                                // Send confirmation via Twilio
                                let confirmation_msg = "Calendar event created successfully!";
                                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                    &conversation.conversation_sid,
                                    &conversation.twilio_number,
                                    confirmation_msg,
                                    true,
                                    &user,
                                ).await {
                                    eprintln!("Failed to send confirmation message: {}", e);
                                }

                                // Deduct credits for the confirmation response
                                if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user.id, "message", None) {
                                    eprintln!("Failed to deduct user credits for calendar confirmation: {}", e);
                                }

                                return (
                                    StatusCode::OK,
                                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                                    axum::Json(TwilioResponse {
                                        message: confirmation_msg.to_string(),
                                    })
                                );
                            }
                            Err((status, Json(error))) => {
                                let error_msg = format!("Failed to create calendar event: {} (not charged)", error["error"].as_str().unwrap_or("Unknown error"));
                                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                    &conversation.conversation_sid,
                                    &conversation.twilio_number,
                                    &error_msg,
                                    true,
                                    &user,
                                ).await {
                                    eprintln!("Failed to send error message: {}", e);
                                }
                                return (
                                    StatusCode::OK,
                                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                                    axum::Json(TwilioResponse {
                                        message: error_msg,
                                    })
                                );
                            }
                        }
                    }
                    "no" => {
                        // Reset the confirmation flag
                        if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                            eprintln!("Failed to reset confirm_send_event flag: {}", e);
                        }

                        // Send cancellation confirmation
                        let cancel_msg = "Calendar event creation cancelled.";
                        if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                            &conversation.conversation_sid,
                            &conversation.twilio_number,
                            cancel_msg,
                            true,
                            &user,
                        ).await {
                            eprintln!("Failed to send cancellation confirmation: {}", e);
                        }

                        return (
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            axum::Json(TwilioResponse {
                                message: cancel_msg.to_string(),
                            })
                        );
                    }
                    _ => {
                        // Reset the confirmation flag since we're treating this as a new message
                        if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                            eprintln!("Failed to reset confirm_send_event flag: {}", e);
                        }
                        // Continue with normal message processing
                    }
                }
            }
            
            // Check for email response confirmation
            if let Some(captures) = regex::Regex::new(r"Confirm sending email response to '([^']+)' regarding '([^']+)' with content: '([^']+)' id:\((\d+)\)")
                .ok()
                .and_then(|re| re.captures(&last_ai_message.body)) {

                let recipient = captures.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                let subject = captures.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
                let response_text = captures.get(3).map(|m| m.as_str().to_string()).unwrap_or_default();
                let email_id = captures.get(4).map(|m| m.as_str().to_string()).unwrap_or_default();

                // Redact the confirmation message after extracting the necessary information
                if let Err(e) = crate::api::twilio_utils::redact_message(
                    &conversation.conversation_sid,
                    &last_ai_message.sid,
                    &format!("Confirm sending email response to '[RECIPIENT_REDACTED]' regarding '[SUBJECT_REDACTED]' with content: '[CONTENT_REDACTED]' id:[ID_REDACTED]"),
                    &user,
                ).await {
                    eprintln!("Failed to redact email confirmation message: {}", e);
                }

                match user_response.as_str() {
                    "yes" => {
                        // Reset the confirmation flag
                        if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                            eprintln!("Failed to reset confirm_send_event flag: {}", e);
                        }

                        // Create request for email response
                        let email_request = crate::handlers::imap_handlers::EmailResponseRequest {
                            email_id: email_id.clone(),
                            response_text: response_text.clone(),
                        };

                        let auth_user = crate::handlers::auth_middleware::AuthUser {
                            user_id: user.id,
                            is_admin: false,
                        };

                        // Send the email response
                        match crate::handlers::imap_handlers::respond_to_email(
                            State(state.clone()),
                            auth_user,
                            Json(email_request),
                        ).await {
                            Ok(_) => {
                                // Send confirmation via Twilio
                                let confirmation_msg = format!("Email response sent successfully to {} regarding '{}'", recipient, subject);
                                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                    &conversation.conversation_sid,
                                    &conversation.twilio_number,
                                    &confirmation_msg,
                                    true,
                                    &user,
                                ).await {
                                    eprintln!("Failed to send confirmation message: {}", e);
                                }

                                // Deduct credits for the confirmation response
                                if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user.id, "message", None) {
                                    eprintln!("Failed to deduct user credits for email confirmation: {}", e);
                                }

                                return (
                                    StatusCode::OK,
                                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                                    axum::Json(TwilioResponse {
                                        message: confirmation_msg,
                                    })
                                );
                            }
                            Err((status, Json(error))) => {
                                let error_msg = format!("Failed to send email response: {} (not charged)", error["error"].as_str().unwrap_or("Unknown error"));
                                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                    &conversation.conversation_sid,
                                    &conversation.twilio_number,
                                    &error_msg,
                                    true,
                                    &user,
                                ).await {
                                    eprintln!("Failed to send error message: {}", e);
                                }
                                return (
                                    StatusCode::OK,
                                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                                    axum::Json(TwilioResponse {
                                        message: error_msg,
                                    })
                                );
                            }
                        }
                    }
                    "no" => {
                        // Reset the confirmation flag
                        if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                            eprintln!("Failed to reset confirm_send_event flag: {}", e);
                        }

                        // Send cancellation confirmation
                        let cancel_msg = "Email response cancelled.";
                        if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                            &conversation.conversation_sid,
                            &conversation.twilio_number,
                            cancel_msg,
                            true,
                            &user,
                        ).await {
                            eprintln!("Failed to send cancellation confirmation: {}", e);
                        }

                        return (
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            axum::Json(TwilioResponse {
                                message: cancel_msg.to_string(),
                            })
                        );
                    }
                    _ => {
                        // Reset the confirmation flag since we're treating this as a new message
                        if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                            eprintln!("Failed to reset confirm_send_event flag: {}", e);
                        }
                        // Continue with normal message processing
                    }
                }
            }

            // Extract chat name and message content from the confirmation message
            if let Some(captures) = regex::Regex::new(r"Confirm the sending of WhatsApp message to '([^']+)' with content: '([^']+)'.*?(?:\(yes->|$)")
                .ok()
                .and_then(|re| re.captures(&last_ai_message.body)) {

                let chat_name = captures.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                println!("chatname: {}",chat_name);
                let message_content = captures.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
                println!("content: {}",message_content);

                // Redact the confirmation message after extracting the necessary information
                if let Err(e) = crate::api::twilio_utils::redact_message(
                    &conversation.conversation_sid,
                    &last_ai_message.sid,
                    &format!("Confirm the sending of WhatsApp message to '[CHAT_NAME_REDACTED]' with content: '[MESSAGE_CONTENT_REDACTED]'"),
                    &user,
                ).await {
                    eprintln!("Failed to redact confirmation message: {}", e);
                }

                match user_response.as_str() {
                    "yes" => {
                        // Send the WhatsApp message
                        match crate::utils::whatsapp_utils::send_whatsapp_message(
                            &state,
                            user.id,
                            &chat_name,
                            &message_content,
                        ).await {
                            Ok(_) => {
                                // Reset the confirmation flag
                                if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                                    eprintln!("Failed to reset confirm_send_event flag: {}", e);
                                }

                                // Send confirmation via Twilio
                                println!("sending messages since user said yes");
                                let confirmation_msg = format!("Message sent successfully to {}", chat_name);
                                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                    &conversation.conversation_sid,
                                    &conversation.twilio_number,
                                    &confirmation_msg,
                                    true,
                                    &user,
                                ).await {
                                    eprintln!("Failed to send confirmation message: {}", e);
                                }

                                // Deduct credits for the confirmation response
                                if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user.id, "message", None) {
                                    eprintln!("Failed to deduct user credits for WhatsApp confirmation: {}", e);
                                }

                                return (
                                    StatusCode::OK,
                                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                                    axum::Json(TwilioResponse {
                                        message: confirmation_msg,
                                    })
                                );
                            }
                            Err(e) => {
                                // Send error message via Twilio
                                println!("sending failed to send the message to whatsapp sms");
                                let error_msg = format!("Failed to send message: {}(not charged)", e);
                                if let Err(send_err) = crate::api::twilio_utils::send_conversation_message(
                                    &conversation.conversation_sid,
                                    &conversation.twilio_number,
                                    &error_msg,
                                    true,
                                    &user,
                                ).await {
                                    eprintln!("Failed to send error message: {}", send_err);
                                }
                                return (
                                    StatusCode::OK,
                                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                                    axum::Json(TwilioResponse {
                                        message: error_msg,
                                    })
                                );
                            }
                        }
                    }
                    "no" => {
                        // Reset the confirmation flag
                        if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                            eprintln!("Failed to reset confirm_send_event flag: {}", e);
                        }

                        // Send cancellation confirmation via Twilio
                        println!("User said not so we are sending message discarded confirmation");
                        let cancel_msg = "Message sending cancelled.";
                        if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                            &conversation.conversation_sid,
                            &conversation.twilio_number,
                            cancel_msg,
                            true,
                            &user,
                        ).await {
                            eprintln!("Failed to send cancellation confirmation: {}", e);
                        }

                        return (
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            axum::Json(TwilioResponse {
                                message: cancel_msg.to_string(),
                            })
                        );
                    }
                    _ => {
                        println!("User said something else than yes or no so continuing");
                        // Reset the confirmation flag since we're treating this as a new message
                        if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                            eprintln!("Failed to reset confirm_send_event flag: {}", e);
                        }
                        // If user provided something else than yes/no, treat it as a new message
                        // and continue with normal message processing
                    }
                }
            }
        }
    }
    
    let auth_user = crate::handlers::auth_middleware::AuthUser {
        user_id: user.id, 
        is_admin: false,
    };


    let user_info = match user.clone().info {
        Some(info) => info,
        None => "".to_string()
    };

    // Get timezone from user info or default to UTC
    let timezone_str = match user.timezone {
        Some(ref tz) => tz.as_str(),
        None => "UTC",
    };

    // Get timezone offset using jiff
    let (hours, minutes) = match crate::api::elevenlabs::get_offset_with_jiff(timezone_str) {
        Ok((h, m)) => (h, m),
        Err(_) => {
            println!("Failed to get timezone offset for {}, defaulting to UTC", timezone_str);
            (0, 0) // UTC default
        }
    };

    // Calculate total offset in seconds
    let offset_seconds = hours * 3600 + minutes * 60 * if hours >= 0 { 1 } else { -1 };

    // Create FixedOffset for chrono
    let user_timezone = chrono::FixedOffset::east_opt(offset_seconds)
        .unwrap_or_else(|| chrono::FixedOffset::east(0)); // Fallback to UTC if invalid

    // Format current time in RFC3339 for the user's timezone
    let formatted_time = Utc::now().with_timezone(&user_timezone).to_rfc3339();
    println!("FORMATTED_TIME: {}", formatted_time);

    // Format offset string (e.g., "+02:00" or "-05:30")
    let offset = format!("{}{:02}:{:02}", 
        if hours >= 0 { "+" } else { "-" },
        hours.abs(),
        minutes.abs()
    );

    // Start with the system message
    let mut chat_messages: Vec<ChatMessage> = vec![ChatMessage {
        role: "system".to_string(),
        content: chat_completion::Content::Text(format!("You are a direct and efficient AI assistant named lightfriend. The current date is {}. You must provide extremely concise responses (max 400 characters) while being accurate and helpful. Since users pay per message, always provide all available information immediately without asking follow-up questions unless confirming details for actions that involve sending information or making changes. Always use all tools immidiately that you think will be needed to complete the user's query and base your response to those responses.\n\n### Tool Usage Guidelines:\n- Provide all relevant details in the response immediately. \n- Tools that involve sending or creating something(eg. send_whatsapp_message), you can call them straight away using the available information without confirming with the user. These tools will send extra confirmation message to user anyways before doing anything.\n\n### Date and Time Handling:\n- Use **RFC3339/ISO8601 format** (e.g., '2024-03-23T14:30:00Z') for all date/time inputs.\n- If no specific time is mentioned:\n  - Use the **current time** for start and **24 hours ahead** for end or the otherway around depending on the tool.\n- Always consider the user's timezone: {} with offset {}.\n- For queries about:\n  - **\"Today\"**: Use 00:00 to 23:59 of the current day.\n  - **\"Tomorrow\"**: Use 00:00 to 23:59 of the next day.\n\n### Additional Guidelines:\n- **Weather Queries**: If no location is specified, assume the user’s home location from user info.\n- **Email Queries**: For `fetch_specific_email`, provide the whole message body or a summary if too long—never just the subject.\n- **WhatsApp Fetching**: Use the room name directly from the user’s message/context without searching rooms.\n\nNever use markdown or HTML in responses. User information: {}. Always use tools to fetch the latest information before answering.", formatted_time, timezone_str, offset, user_info)),
    }];
    
    // Process the message body to remove "forget" if it exists at the start
    let processed_body = if payload.body.to_lowercase().starts_with("forget") {
        payload.body.trim_start_matches(|c: char| c.is_alphabetic()).trim().to_string()
    } else {
        payload.body.clone()
    };

    // Delete media if present after processing
    if let (Some(num_media), Some(media_url), Some(_)) = (
        payload.num_media.as_ref(),
        payload.media_url0.as_ref(),
        payload.media_content_type0.as_ref()
    ) {
        if num_media != "0" {
            // Extract media SID from URL
            if let Some(media_sid) = media_url.split("/Media/").nth(1) {
                println!("Attempting to delete media with SID: {}", media_sid);
                match crate::api::twilio_utils::delete_twilio_message_media(&media_sid, &user).await {
                    Ok(_) => println!("Successfully deleted media: {}", media_sid),
                    Err(e) => eprintln!("Failed to delete media {}: {}", media_sid, e),
                }
            }
        }
    }

    // Only include conversation history if message starts with "forget"
    if !payload.body.to_lowercase().starts_with("forget") {
    let mut history: Vec<ChatMessage> = messages.clone().into_iter().map(|msg| {
        ChatMessage {
            role: if msg.author == "lightfriend" { "assistant" } else { "user" }.to_string(),
            content: chat_completion::Content::Text(msg.body.clone()),
        }
    }).collect();
        history.reverse();
        
        // Combine system message with conversation history
        chat_messages.extend(history);
    }

    // Handle image if present
    let mut has_image = false;
    let mut image_url = None;
    
    if let (Some(num_media), Some(media_url), Some(content_type)) = (
        payload.num_media.as_ref(),
        payload.media_url0.as_ref(),
        payload.media_content_type0.as_ref()
    ) {
        if num_media != "0" && content_type.starts_with("image/") {
            has_image = true;
            image_url = Some(media_url.clone());
            
            println!("setting image_url var to: {:#?}", image_url);
            // Add the image URL message with the text
            chat_messages.push(ChatMessage {
                role: "user".to_string(),
                content: chat_completion::Content::ImageUrl(vec![
                    chat_completion::ImageUrl {
                        r#type: chat_completion::ContentType::image_url,
                        text: Some(processed_body.clone()),
                        image_url: Some(chat_completion::ImageUrlType {
                            url: media_url.clone(),
                        }),
                    },
                ]),

            });

            // Also add the text as a separate message if it's not empty 
            if !processed_body.trim().is_empty() {
                chat_messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: chat_completion::Content::Text(format!("Text accompanying the image: {}", processed_body)),
                });
            }
        } else {
            // Add regular text message if no image
            chat_messages.push(ChatMessage {
                role: "user".to_string(),
                content: chat_completion::Content::Text(processed_body),
            });
        }
    } else {
        // Add regular text message if no media
        chat_messages.push(ChatMessage {
            role: "user".to_string(),
            content: chat_completion::Content::Text(processed_body),
        });
    }

    println!("chat_messages: {:#?}",chat_messages);


    let mut waiting_check_properties = HashMap::new();
    waiting_check_properties.insert(
        "content".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The content to look for in emails".to_string()),
            ..Default::default()
        }),
    );
    waiting_check_properties.insert(
        "due_date".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some("Unix timestamp for when this check should be completed by, default to two weeks into the future.".to_string()),
            ..Default::default()
        }),
    );
    waiting_check_properties.insert(
        "remove_when_found".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("Whether to remove the check once the content is found, default to true.".to_string()),
            ..Default::default()
        }),
    );

    let mut plex_properties = HashMap::new();
    plex_properties.insert(
        "query".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The question or topic to get information about".to_string()),
            ..Default::default()
        }),
    );

    let mut weather_properties = HashMap::new();
    weather_properties.insert(
        "location".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Location of the place where we want to search the weather.".to_string()),
            ..Default::default()
        }),
    );
    weather_properties.insert(
        "units".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Units that the weather should be returned as. Should be either 'metric' or 'imperial'".to_string()),
            ..Default::default()
        }),
    );




    let mut calendar_properties = HashMap::new();
    calendar_properties.insert(
        "start".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Start time in RFC3339 format in UTC (e.g., '2024-03-16T00:00:00Z')".to_string()),
            ..Default::default()
        }),
    );
    calendar_properties.insert(
        "end".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("End time in RFC3339 format in UTC (e.g., '2024-03-16T00:00:00Z')".to_string()),
            ..Default::default()
        }),
    );
    // Add calendar event properties
    let mut calendar_event_properties = HashMap::new();
    calendar_event_properties.insert(
        "summary".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The title/summary of the calendar event".to_string()),
            ..Default::default()
        }),
    );
    calendar_event_properties.insert(
        "start_time".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Start time in RFC3339 format in UTC (e.g., '2024-03-23T14:30:00Z')".to_string()),
            ..Default::default()
        }),
    );
    calendar_event_properties.insert(
        "duration_minutes".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some("Duration of the event in minutes".to_string()),
            ..Default::default()
        }),
    );
    calendar_event_properties.insert(
        "description".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Optional description of the event. Do not add unless user asks specifically.".to_string()),
            ..Default::default()
        }),
    );
    calendar_event_properties.insert(
        "add_notification".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("Whether to add a notification reminder (defaults to true unless specified)".to_string()),
            ..Default::default()
        }),
    );

    let mut tasks_properties = HashMap::new();
    tasks_properties.insert(
        "param".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Can be anything, will fetch all tasks regardless".to_string()),
            ..Default::default()
        }),
    );

    let mut whatsapp_messages_properties = HashMap::new();
    whatsapp_messages_properties.insert(
        "start".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Start time in RFC3339 format in UTC (e.g., '2024-03-16T00:00:00Z')".to_string()),
            ..Default::default()
        }),
    );
    whatsapp_messages_properties.insert(
        "end".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("End time in RFC3339 format in UTC (e.g., '2024-03-16T00:00:00Z')".to_string()),
            ..Default::default()
        }),
    );

    let mut whatsapp_room_messages_properties = HashMap::new();
    whatsapp_room_messages_properties.insert(
        "chat_name".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The name of the WhatsApp chat/room to fetch messages from".to_string()),
            ..Default::default()
        }),
    );
    whatsapp_room_messages_properties.insert(
        "limit".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some("Optional: Maximum number of messages to fetch (default: 20)".to_string()),
            ..Default::default()
        }),
    );

    let mut whatsapp_search_properties = HashMap::new();
    whatsapp_search_properties.insert(
        "search_term".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Search term to find WhatsApp rooms/contacts".to_string()),
            ..Default::default()
        }),
    );

    let mut whatsapp_send_properties = HashMap::new();
    whatsapp_send_properties.insert(
        "chat_name".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The chat name or room name to send the message to. Doesn't have to be exact since fuzzy search is used.".to_string()),
            ..Default::default()
        }),
    );
    whatsapp_send_properties.insert(
        "message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The message content to send".to_string()),
            ..Default::default()
        }),
    );

    let mut create_task_properties = HashMap::new();
    create_task_properties.insert(
        "title".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The title of the task".to_string()),
            ..Default::default()
        }),
    );
    create_task_properties.insert(
        "description".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Optional description of the task".to_string()),
            ..Default::default()
        }),
    );
    create_task_properties.insert(
        "due_time".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Optional due time in RFC3339 format in UTC (e.g., '2024-03-23T14:30:00Z')".to_string()),
            ..Default::default()
        }),
    );

    let mut imap_properties = HashMap::new();
    imap_properties.insert(
        "param".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Can be anything, will fetch last 5 emails regardless".to_string()),
            ..Default::default()
        }),
    );

    let mut specific_email_properties = HashMap::new();
    specific_email_properties.insert(
        "query".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The search query to find a specific email".to_string()),
            ..Default::default()
        }),
    );

    let mut placeholder_properties = HashMap::new();
    placeholder_properties.insert(
        "param".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("put nothing here".to_string()),
            ..Default::default()
        }),
    );

    // Define tools
    let tools = vec![
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("scan_qr_code"),
                description: Some(String::from("Scans and extracts data from a QR code in an image. Use this when the user sends an image that appears to contain a QR code.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(placeholder_properties),
                    required: None,
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("delete_sms_conversation_history"),
                description: Some(String::from("Deletes all sms conversation history for a specific user. Use this when user asks to delete their chat history or conversations. It won't delete the history from their phone obviously.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(placeholder_properties),
                    required: None,
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("send_whatsapp_message"),
                description: Some(String::from("Sends a WhatsApp message to a specific chat. This tool will first make a confirmation message for the user, which they can then confirm or not. The chat_name will be used to fuzzy search for the actual chat name and then confirmed with the user.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(whatsapp_send_properties),
                    required: Some(vec![String::from("chat_name"), String::from("message")]),
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("fetch_whatsapp_messages"),
                description: Some(String::from("Fetches recent WhatsApp messages. Use this when user asks about their WhatsApp messages or conversations.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(whatsapp_messages_properties),
                    required: Some(vec![String::from("start"), String::from("end")]),
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("fetch_whatsapp_room_messages"),
                description: Some(String::from("Fetches messages from a specific WhatsApp chat/room. Use this when user asks about messages from a specific WhatsApp contact or group. You should return the latest messages that person or chat has sent to the user.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(whatsapp_room_messages_properties),
                    required: Some(vec![String::from("chat_name")]),
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("search_whatsapp_rooms"),
                description: Some(String::from("Searches for WhatsApp rooms/contacts by name.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(whatsapp_search_properties),
                    required: Some(vec![String::from("search_term")]),
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("create_waiting_check"),
                description: Some(String::from("Creates a waiting check for monitoring emails. Use this when user wants to be notified about specific emails or content in their emails.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(waiting_check_properties),
                    required: Some(vec![String::from("content")]),
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("fetch_imap_emails"),
                description: Some(String::from("Fetches the last 5 emails using IMAP. Use this when user asks about their recent emails or wants to check their inbox.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(imap_properties),
                    required: None,
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("fetch_specific_email"),
                description: Some(String::from("Search and fetch a specific email based on a query. Use this when user asks about a specific email or wants to find an email about a particular topic. You must ALWAYS respond with the whole message body or summary of the body if too long. Never reply with just the subject line!")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(specific_email_properties),
                    required: Some(vec![String::from("query")]),
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("ask_perplexity"),
                description: Some(String::from("Get factual or timely information about any topic")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(plex_properties),
                    required: Some(vec![String::from("query")]),
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("get_weather"),
                description: Some(String::from("Fetches the current weather for the given location. The AI should use the user's home location from user info if none is specified in the query.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(weather_properties),
                    required: Some(vec![String::from("location"), String::from("units")]),
                },
            },
        },

        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("calendar"),
                description: Some(String::from("Fetches the user's calendar events for the specified time frame.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(calendar_properties),
                    required: Some(vec![String::from("start"), String::from("end")]),
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("create_calendar_event"),
                description: Some(String::from("Creates a new Google Calendar event. Use this when user wants to schedule or add an event to their calendar. This tool will first make a confirmation message for the user, which they can then confirm or not.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(calendar_event_properties),
                    required: Some(vec![String::from("summary"), String::from("start_time"), String::from("duration_minutes")]),
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("fetch_tasks"),
                description: Some(String::from("Fetches the user's Google Tasks. Use this when user asks about their tasks, reminders, or ideas.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(tasks_properties),
                    required: None,
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("create_task"),
                description: Some(String::from("Creates a new Google Task. Use this when user wants to add a task, reminder, or idea.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(create_task_properties),
                    required: Some(vec![String::from("title")]),
                },
            },
        },
        
    ];

    let api_key = match env::var("OPENROUTER_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("OPENROUTER_API_KEY not set");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Server configuration error".to_string(),
                })
            );
        }
    };

    let client = match OpenAIClient::builder()
        .with_endpoint("https://openrouter.ai/api/v1")
        .with_api_key(api_key)
        .build() {
            Ok(client) => client,
            Err(e) => {
                eprintln!("Failed to build OpenAI client: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(TwilioResponse {
                        message: "Failed to initialize AI service".to_string(),
                    })
                );
            }
        };

    println!("built client");

    // Convert ChatMessage vec into ChatCompletionMessage vec
    let completion_messages: Vec<chat_completion::ChatCompletionMessage> = chat_messages
        .into_iter()
        .map(|msg| chat_completion::ChatCompletionMessage {
            role: match msg.role.as_str() {
                "user" => chat_completion::MessageRole::user,
                "assistant" => chat_completion::MessageRole::assistant,
                "system" => chat_completion::MessageRole::system,
                _ => chat_completion::MessageRole::user, // default to user if unknown
            },
            content: msg.content.clone(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        })
        .collect();

    let req = chat_completion::ChatCompletionRequest::new(
        GPT4_O.to_string(),
        completion_messages.clone(),
    )
    .tools(tools)
    .tool_choice(chat_completion::ToolChoiceType::Auto)
    .max_tokens(250); // This will result in responses around 400-450 characters

    println!("built request");

    let result = match client.chat_completion(req).await {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Failed to get chat completion: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        axum::Json(TwilioResponse {
                            message: "Failed to process your request".to_string(),
                        })
                    );
        }
    };

    println!("built completion");

    println!("Processing model response with finish reason: {:?}", result.choices[0].finish_reason);
    let mut fail = false;
    let mut tool_answers: HashMap<String, String> = HashMap::new(); // tool_call id and answer
    let final_response = match result.choices[0].finish_reason {
        None | Some(chat_completion::FinishReason::stop) => {
            println!("Model provided direct response (no tool calls needed)");
            // Direct response from the model
            let resp = result.choices[0].message.content.clone().unwrap_or_default();
            resp
        }
        Some(chat_completion::FinishReason::tool_calls) => {
            println!("Model requested tool calls - beginning tool execution phase");
            #[derive(Deserialize, Serialize)]
            struct PerplexityQuestion {
                query: String,
            }
            #[derive(Deserialize, Serialize)]
            struct WeatherQuestion {
                location: String,
                units: String,
            }
            #[derive(Deserialize, Serialize)]
            struct CalendarTimeFrame {
                start: String,
                end: String,
            }
            
            let tool_calls = match result.choices[0].message.tool_calls.as_ref() {
                Some(calls) => {
                    println!("Found {} tool call(s) in response", calls.len());
                    calls
                },
                None => {
                    eprintln!("No tool calls found in response despite tool_calls finish reason");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        axum::Json(TwilioResponse {
                            message: "Failed to process your request".to_string(),
                        })
                    );
                }
            };

            for tool_call in tool_calls {
                let tool_call_id = tool_call.id.clone();
                println!("Processing tool call: {:?} with id: {:?}", tool_call, tool_call_id);
                let name = match &tool_call.function.name {
                    Some(n) => {
                        println!("Tool call function name: {}", n);
                        n
                    },
                    None => {
                        println!("Tool call missing function name, skipping");
                        continue;
                    },
                };

                // Check if user has access to this tool
                if requires_subscription(name, user.sub_tier.clone(), user.discount) {
                    println!("Attempted to use subscription-only tool {} without proper subscription", name);
                    tool_answers.insert(tool_call_id, format!("This feature ({}) requires a subscription. Please visit our website to subscribe.", name));
                    continue;
                }
                let arguments = match &tool_call.function.arguments {
                    Some(args) => args,
                    None => continue,
                };
                if name == "ask_perplexity" {
                    println!("Executing ask_perplexity tool call");
                    let c: PerplexityQuestion = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            eprintln!("Failed to parse perplexity question: {}", e);
                            continue;
                        }
                    };
                    let query = format!("User info: {}. Query: {}", user_info, c.query);

                    let sys_prompt = format!("You are assisting an AI text messaging service. The questions you receive are from text messaging conversations where users are seeking information or help. Please note: 1. Provide clear, conversational responses that can be easily read from a small screen 2. Avoid using any markdown, HTML, or other markup languages 3. Keep responses concise but informative 4. When listing multiple points, use simple numbering (1, 2, 3) 5. Focus on the most relevant information that addresses the user's immediate needs. This is what you should know about the user who this information is going to in their own words: {}", user_info);
                    match crate::utils::tool_exec::ask_perplexity(&query, &sys_prompt).await {
                        Ok(answer) => {
                            println!("Successfully received Perplexity answer");
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            eprintln!("Failed to get perplexity answer: {}", e);
                            continue;
                        }
                    };
                } else if name == "get_weather" {
                    println!("Executing get_weather tool call");
                    let c: WeatherQuestion = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            eprintln!("Failed to parse calendar question: {}", e);
                            continue;
                        }
                    };
                    let location= c.location;
                    let units= c.units;

                    match crate::utils::tool_exec::get_weather(&location, &units).await {
                        Ok(answer) => {
                            println!("Successfully received weather answer");
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            eprintln!("Failed to get weather answer: {}", e);
                            continue;
                        }
                    };
                } else if name == "use_shazam" {
                    tool_answers.insert(tool_call_id, "The Shazam feature has been discontinued due to insufficient usage. Thank you for your understanding.".to_string());
                } else if name == "fetch_imap_emails" {
                    println!("Executing fetch_imap_emails tool call");
                    let queryObj = crate::handlers::imap_handlers::FetchEmailsQuery { limit: None };
                    match crate::handlers::imap_handlers::fetch_full_imap_emails(
                        axum::extract::State(state.clone()),
                        auth_user,
                        axum::extract::Query(queryObj),
                    ).await {
                        Ok(Json(response)) => {
                            if let Some(emails) = response.get("emails") {
                                if let Some(emails_array) = emails.as_array() {
                                    let mut response = String::new();
                                    for (i, email) in emails_array.iter().rev().take(5).enumerate() {
                                        let subject = email.get("subject").and_then(|s| s.as_str()).unwrap_or("No subject");
                                        let from = email.get("from").and_then(|f| f.as_str()).unwrap_or("Unknown sender");

                                        let date_formatted = email.get("date_formatted")
                                            .and_then(|d| d.as_str())
                                            .unwrap_or("Unknown date");
                                        
                                        if i == 0 {
                                            response.push_str(&format!("{}. {} from {} ({}):\n", i + 1, subject, from, date_formatted));
                                        } else {
                                            response.push_str(&format!("\n\n{}. {} from {} ({}):\n", i + 1, subject, from, date_formatted));
                                        }
                                    }
                                    
                                    if emails_array.len() > 5 {
                                        response.push_str(&format!("\n\n(+ {} more emails)", emails_array.len() - 5));
                                    }
                                    
                                    if emails_array.is_empty() {
                                        response = "No recent emails found.".to_string();
                                    }
                                    
                                    tool_answers.insert(tool_call_id, response);
                                } else {
                                    tool_answers.insert(tool_call_id, "Failed to parse emails.".to_string());
                                }
                            } else {
                                tool_answers.insert(tool_call_id, "No emails found.".to_string());
                            }

                        }
                        Err((status, Json(error))) => {
                            let error_message = match status {
                                StatusCode::BAD_REQUEST => "No IMAP connection found. Please check your email settings.",
                                StatusCode::UNAUTHORIZED => "Your email credentials need to be updated.",
                                _ => "Failed to fetch emails. Please try again later.",
                            };
                            tool_answers.insert(tool_call_id, error_message.to_string());
                            eprintln!("Failed to fetch IMAP emails: {:?}", error);
                        }
                    }
                } else if name == "fetch_specific_email" {
                    println!("Executing fetch_specific_email tool call");
                    #[derive(Deserialize)]
                    struct EmailQuery {
                        query: String,
                    }
                    
                    let query: EmailQuery = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            eprintln!("Failed to parse email query: {}", e);
                            continue;
                        }
                    };

                    // Fetch the latest 20 emails with full content
                    match crate::handlers::imap_handlers::fetch_emails_imap(&state, user.id, true, Some(20), false).await {
                        Ok(emails) => {
                            if emails.is_empty() {
                                tool_answers.insert(tool_call_id, "No emails found.".to_string());
                                continue;
                            }

                            // Format all emails into a searchable response
                            let mut response = format!("Search query: '{}'\n\nLatest emails (newest first):\n\n", query.query);
                            for (i, email) in emails.iter().enumerate() {
                                let formatted_email = format!(
                                    "Email {}:\nFrom: {}\nSubject: {}\nDate: {}\n\n{}\n",
                                    i + 1,
                                    email.from.as_deref().unwrap_or("Unknown"),
                                    email.subject.as_deref().unwrap_or("No subject"),
                                    email.date_formatted.as_deref().unwrap_or("No date"),
                                    email.body.as_deref().unwrap_or("No content"),
                                );
                                response.push_str(&formatted_email);
                            }

                            tool_answers.insert(tool_call_id, response);
                        }
                        Err(e) => {
                            let error_message = match e {
                                ImapError::NoConnection => "No IMAP connection found. Please check your email settings.",
                                ImapError::CredentialsError(_) => "Your email credentials need to be updated.",
                                ImapError::ConnectionError(msg) | ImapError::FetchError(msg) | ImapError::ParseError(msg) => {
                                    eprintln!("Failed to fetch emails: {}", msg);
                                    "Failed to fetch emails. Please try again later."
                                }
                            };
                            tool_answers.insert(tool_call_id, error_message.to_string());
                        }
                    }
                } else if name == "create_waiting_check" {
                    println!("Executing create_waiting_check tool call");
                    match handle_create_waiting_check(&state, user.id, arguments).await {
                        Ok(answer) => {
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            eprintln!("Failed to create waiting check: {}", e);
                            tool_answers.insert(tool_call_id, "Sorry, I couldn't set up the email monitoring. Please try again.".to_string());
                        }
                    }
                } else if name == "create_calendar_event" {
                    println!("Executing create_calendar_event tool call");
                    #[derive(Deserialize)]
                    struct CalendarEventArgs {
                        summary: String,
                        start_time: String,
                        duration_minutes: i32,
                        description: Option<String>,
                        add_notification: Option<bool>,
                    }

                    let args: CalendarEventArgs = match serde_json::from_str(arguments) {
                        Ok(args) => args,
                        Err(e) => {
                            eprintln!("Failed to parse calendar event arguments: {}", e);

                            continue;
                        }
                    };

                    // Set the confirmation flag
                    if let Err(e) = state.user_repository.set_confirm_send_event(user.id, true) {
                        eprintln!("Failed to set confirm_send_event flag: {}", e);
                        if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                            &conversation.conversation_sid,
                            &conversation.twilio_number,
                            "Failed to prepare calendar event creation. (not charged, contact rasmus@ahtava.com)",
                            true,
                            &user,
                        ).await {
                            eprintln!("Failed to send error message: {}", e);
                        }
                        return (
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            axum::Json(TwilioResponse {
                                message: "Failed to prepare calendar event creation".to_string(),
                            })
                        );
                    }

                    // Format the confirmation message
                    let confirmation_msg = if let Some(desc) = args.description {
                        format!(
                            "Confirm creating calendar event: '{}' starting at '{}' for {} minutes with description: '{}' (yes-> send, no -> discard) (free reply)",
                            args.summary, args.start_time, args.duration_minutes, desc
                        )
                    } else {
                        format!(
                            "Confirm creating calendar event: '{}' starting at '{}' for {} minutes (yes-> send, no -> discard) (free reply)",
                            args.summary, args.start_time, args.duration_minutes
                        )
                    };

                    // Send the confirmation message
                    match crate::api::twilio_utils::send_conversation_message(
                        &conversation.conversation_sid,
                        &conversation.twilio_number,
                        &confirmation_msg,
                        false, // Don't redact since we need to extract info from this message later
                        &user,
                    ).await {
                        Ok(_) => {
                            // Deduct credits for the confirmation message
                            if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user.id, "message", None) {
                                eprintln!("Failed to deduct user credits: {}", e);
                            }
                            return (
                                StatusCode::OK,
                                [(axum::http::header::CONTENT_TYPE, "application/json")],
                                axum::Json(TwilioResponse {
                                    message: "Calendar event confirmation sent".to_string(),
                                })
                            );
                        }
                        Err(e) => {
                            eprintln!("Failed to send confirmation message: {}", e);
                            return (
                                StatusCode::OK,
                                [(axum::http::header::CONTENT_TYPE, "application/json")],
                                axum::Json(TwilioResponse {
                                    message: "Failed to send calendar event confirmation".to_string(),
                                })
                            );
                        }
                    }
                } else if name == "create_task" {
                    println!("Executing create_task tool call");
                    #[derive(Deserialize)]
                    struct CreateTaskArgs {
                        title: String,
                        description: Option<String>,
                        due_time: Option<String>,
                    }

                    let args: CreateTaskArgs = match serde_json::from_str(arguments) {
                        Ok(args) => args,
                        Err(e) => {
                            eprintln!("Failed to parse create task arguments: {}", e);
                            tool_answers.insert(tool_call_id, "Failed to create task due to invalid arguments.".to_string());
                            continue;
                        }
                    };

                    // Convert due_time string to DateTime<Utc> if provided
                    let due_time = if let Some(dt_str) = args.due_time {
                        match chrono::DateTime::parse_from_rfc3339(&dt_str) {
                            Ok(dt) => Some(dt.with_timezone(&chrono::Utc)),
                            Err(e) => {
                                eprintln!("Failed to parse due time: {}", e);
                                None
                            }
                        }
                    } else {
                        None
                    };

                    let task_request = crate::handlers::google_tasks::CreateTaskRequest {
                        title: args.title,
                        description: args.description,
                        due_time,
                    };

                    match crate::handlers::google_tasks::create_task(&state, user.id, &task_request).await {
                        Ok(Json(response)) => {
                            tool_answers.insert(tool_call_id, "Task created successfully.".to_string());
                        }
                        Err((status, Json(error))) => {
                            let error_message = match status {
                                StatusCode::UNAUTHORIZED => "You need to connect your Google Tasks first. Visit the website to set it up.",
                                _ => "Failed to create task. Please try again later.",
                            };
                            tool_answers.insert(tool_call_id, error_message.to_string());
                            eprintln!("Failed to create task: {:?}", error);
                        }
                    }
                } else if name == "fetch_tasks" {
                    println!("Executing fetch_tasks tool call");
                    match crate::handlers::google_tasks::get_tasks(&state, user.id).await {
                        Ok(Json(response)) => {
                            if let Some(tasks) = response.get("tasks") {
                                if let Some(tasks_array) = tasks.as_array() {
                                    if tasks_array.is_empty() {
                                        tool_answers.insert(tool_call_id, "You don't have any tasks in your list.".to_string());
                                    } else {
                                        let mut response = String::new();
                                        for (i, task) in tasks_array.iter().enumerate() {
                                            let title = task.get("title").and_then(|t| t.as_str()).unwrap_or("Untitled");
                                            let status = task.get("status").and_then(|s| s.as_str()).unwrap_or("unknown");
                                            let due = task.get("due").and_then(|d| d.as_str()).unwrap_or("");
                                            let notes = task.get("notes").and_then(|n| n.as_str()).unwrap_or("");
                                            
                                            let status_emoji = if status == "completed" { "✅" } else { "📝" };
                                            let due_text = if !due.is_empty() {
                                                format!(" (due: {})", due)
                                            } else {
                                                String::new()
                                            };
                                            
                                            if i == 0 {
                                                response.push_str(&format!("{}. {} {}{}", i + 1, status_emoji, title, due_text));
                                            } else {
                                                response.push_str(&format!("\n{}. {} {}{}", i + 1, status_emoji, title, due_text));
                                            }
                                            
                                            if !notes.is_empty() {
                                                response.push_str(&format!("\n   Note: {}", notes));
                                            }
                                        }
                                        tool_answers.insert(tool_call_id, response);
                                    }
                                } else {
                                    tool_answers.insert(tool_call_id, "Failed to parse tasks list.".to_string());
                                }
                            } else {
                                tool_answers.insert(tool_call_id, "No tasks found.".to_string());
                            }
                        }
                        Err((status, Json(error))) => {
                            let error_message = match status {
                                StatusCode::UNAUTHORIZED => "You need to connect your Google Tasks first. Visit the website to set it up.",
                                _ => "Failed to fetch tasks. Please try again later.",
                            };
                            tool_answers.insert(tool_call_id, error_message.to_string());
                            eprintln!("Failed to fetch tasks: {:?}", error);
                        }
                    }
                } else if name == "send_whatsapp_message" {
                    println!("Executing send_whatsapp_message tool call");
                    #[derive(Deserialize)]
                    struct WhatsAppSendArgs {
                        chat_name: String,
                        message: String,
                    }

                    let args: WhatsAppSendArgs = match serde_json::from_str(arguments) {
                        Ok(args) => args,
                        Err(e) => {
                            eprintln!("Failed to parse WhatsApp send arguments: {}", e);
                            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                &conversation.conversation_sid,
                                &conversation.twilio_number,
                                "Failed to parse message sending request. (not charged, contact rasmus@ahtava.com)",
                                true,
                                &user,
                            ).await {
                                eprintln!("Failed to send error message: {}", e);
                            }
                            return (
                                StatusCode::OK,
                                [(axum::http::header::CONTENT_TYPE, "application/json")],
                                axum::Json(TwilioResponse {
                                    message: "Failed to parse WhatsApp message request".to_string(),
                                })
                            );
                        }
                    };

                    // First search for the chat room
                    match crate::utils::whatsapp_utils::search_whatsapp_rooms(
                        &state,
                        user.id,
                        &args.chat_name,
                    ).await {
                        Ok(rooms) => {
                            if rooms.is_empty() {
                                let error_msg = format!("No WhatsApp contacts found matching '{}'.", args.chat_name);
                                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                    &conversation.conversation_sid,
                                    &conversation.twilio_number,
                                    &error_msg,
                                    true,
                                    &user,
                                ).await {
                                    eprintln!("Failed to send error message: {}", e);
                                }
                                return (
                                    StatusCode::OK,
                                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                                    axum::Json(TwilioResponse {
                                        message: error_msg,
                                    })
                                );
                            }

                            // Get the best match (first result)
                            let best_match = &rooms[0];
                            let exact_name = best_match.display_name.trim_end_matches(" (WA)").to_string();

                            // Set the confirmation flag
                            if let Err(e) = state.user_repository.set_confirm_send_event(user.id, true) {
                                eprintln!("Failed to set confirm_send_event flag: {}", e);
                                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                    &conversation.conversation_sid,
                                    &conversation.twilio_number,
                                    "Failed to prepare WhatsApp message sending. (not charged, contact rasmus@ahtava.com)",
                                    true,
                                    &user,
                                ).await {
                                    eprintln!("Failed to send error message: {}", e);
                                }
                                return (
                                    StatusCode::OK,
                                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                                    axum::Json(TwilioResponse {
                                        message: "Failed to prepare WhatsApp message sending".to_string(),
                                    })
                                );
                            }

                            // Format the confirmation message with the found contact name
                            let confirmation_msg = format!(
                                "Confirm the sending of WhatsApp message to '{}' with content: '{}' (yes-> send, no -> discard) (free reply)",
                                exact_name, args.message
                            );

                            // Send the confirmation message
                            match crate::api::twilio_utils::send_conversation_message(
                                &conversation.conversation_sid,
                                &conversation.twilio_number,
                                &confirmation_msg,
                                false, // Don't redact since we need to extract info from this message later
                                &user,
                            ).await {
                                Ok(_) => {
                                    // Deduct credits for the confirmation message
                                    if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user.id, "message", None) {
                                        eprintln!("Failed to deduct user credits: {}", e);
                                    }
                                    return (
                                        StatusCode::OK,
                                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                                        axum::Json(TwilioResponse {
                                            message: "WhatsApp message confirmation sent".to_string(),
                                        })
                                    );
                                }
                                Err(e) => {
                                    eprintln!("Failed to send confirmation message: {}", e);
                                    return (
                                        StatusCode::OK,
                                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                                        axum::Json(TwilioResponse {
                                            message: "Failed to send WhatsApp confirmation".to_string(),
                                        })
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to search WhatsApp rooms: {}", e);
                            let error_msg = "Failed to find WhatsApp contact. Please make sure you're connected to WhatsApp bridge.";
                            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                &conversation.conversation_sid,
                                &conversation.twilio_number,
                                error_msg,
                                true,
                                &user,
                            ).await {
                                eprintln!("Failed to send error message: {}", e);
                            }
                            return (
                                StatusCode::OK,
                                [(axum::http::header::CONTENT_TYPE, "application/json")],
                                axum::Json(TwilioResponse {
                                    message: error_msg.to_string(),
                                })
                            );
                        }
                    }
                } else if name == "search_whatsapp_rooms" {
                    println!("Executing search_whatsapp_rooms tool call");
                    #[derive(Deserialize)]
                    struct WhatsAppSearchArgs {
                        search_term: String,
                    }

                    let args: WhatsAppSearchArgs = match serde_json::from_str(arguments) {
                        Ok(args) => args,
                        Err(e) => {
                            eprintln!("Failed to parse WhatsApp search arguments: {}", e);
                            tool_answers.insert(tool_call_id, "Failed to parse search request.".to_string());
                            continue;
                        }
                    };

                    match crate::utils::whatsapp_utils::search_whatsapp_rooms(
                        &state,
                        user.id,
                        &args.search_term,
                    ).await {
                        Ok(rooms) => {
                            if rooms.is_empty() {
                                tool_answers.insert(tool_call_id, format!("No WhatsApp contacts found matching '{}'.", args.search_term));
                            } else {
                                let mut response = String::new();
                                for (i, room) in rooms.iter().take(5).enumerate() {
                                    if i == 0 {
                                        response.push_str(&format!("{}. {} (last active: {})", 
                                            i + 1,
                                            room.display_name.trim_end_matches(" (WA)"),
                                            room.last_activity_formatted
                                        ));
                                    } else {
                                        response.push_str(&format!("\n{}. {} (last active: {})", 
                                            i + 1,
                                            room.display_name.trim_end_matches(" (WA)"),
                                            room.last_activity_formatted
                                        ));
                                    }
                                }
                                
                                if rooms.len() > 5 {
                                    response.push_str(&format!("\n\n(+ {} more contacts)", rooms.len() - 5));
                                }
                                
                                tool_answers.insert(tool_call_id, response);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to search WhatsApp rooms: {}", e);
                            tool_answers.insert(tool_call_id, 
                                "Failed to search WhatsApp contacts. Please make sure you're connected to WhatsApp bridge.".to_string()
                            );
                        }
                    }
                } else if name == "fetch_whatsapp_room_messages" {
                    println!("Executing fetch_whatsapp_room_messages tool call");
                    #[derive(Deserialize)]
                    struct WhatsAppRoomArgs {
                        chat_name: String,
                        limit: Option<u64>,
                    }

                    let args: WhatsAppRoomArgs = match serde_json::from_str(arguments) {
                        Ok(args) => args,
                        Err(e) => {
                            eprintln!("Failed to parse WhatsApp room arguments: {}", e);
                            tool_answers.insert(tool_call_id, "Failed to parse room message request.".to_string());
                            continue;
                        }
                    };

                    match crate::utils::whatsapp_utils::fetch_whatsapp_room_messages(
                        &state,
                        user.id,
                        &args.chat_name,
                        args.limit,
                    ).await {
                        Ok((messages, room_name)) => {
                            if messages.is_empty() {
                                tool_answers.insert(tool_call_id, format!("No messages found in chat '{}'.", room_name.trim_end_matches(" (WA)")));
                            } else {
                                let mut response = format!("Messages from '{}':\n\n", room_name.trim_end_matches(" (WA)"));
                                for (i, msg) in messages.iter().take(10).enumerate() {
                                    let content = if msg.content.chars().count() > 100 {
                                        let truncated: String = msg.content.chars().take(97).collect();
                                        format!("{}...", truncated)
                                    } else {
                                        msg.content.clone()
                                    };
                                    
                                    if i == 0 {
                                        response.push_str(&format!("{}. {} at {}:\n{}", 
                                            i + 1, 
                                            msg.sender_display_name,
                                            msg.formatted_timestamp,
                                            content
                                        ));
                                    } else {
                                        response.push_str(&format!("\n\n{}. {} at {}:\n{}", 
                                            i + 1, 
                                            msg.sender_display_name,
                                            msg.formatted_timestamp,
                                            content
                                        ));
                                    }
                                }
                                
                                if messages.len() > 10 {
                                    response.push_str(&format!("\n\n(+ {} more messages)", messages.len() - 10));
                                }
                                
                                tool_answers.insert(tool_call_id, response);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to fetch WhatsApp room messages: {}", e);
                            tool_answers.insert(tool_call_id, 
                                format!("Failed to fetch messages from '{}'. Please make sure you're connected to WhatsApp bridge and the chat exists.", args.chat_name)
                            );
                        }
                    }
                } else if name == "fetch_whatsapp_messages" {
                    println!("Executing fetch_whatsapp_messages tool call");
                    #[derive(Deserialize)]
                    struct WhatsAppTimeFrame {
                        start: String,
                        end: String,
                    }

                    let time_frame: WhatsAppTimeFrame = match serde_json::from_str(arguments) {
                        Ok(tf) => tf,
                        Err(e) => {
                            eprintln!("Failed to parse WhatsApp time frame: {}", e);
                            tool_answers.insert(tool_call_id, "Failed to parse time frame for WhatsApp messages.".to_string());
                            continue;
                        }
                    };

                    // Parse the RFC3339 timestamps into Unix timestamps
                    let start_time = match chrono::DateTime::parse_from_rfc3339(&time_frame.start) {
                        Ok(dt) => dt.timestamp(),
                        Err(e) => {
                            eprintln!("Failed to parse start time: {}", e);
                            tool_answers.insert(tool_call_id, "Invalid start time format. Please use RFC3339 format.".to_string());
                            continue;
                        }
                    };

                    let end_time = match chrono::DateTime::parse_from_rfc3339(&time_frame.end) {
                        Ok(dt) => dt.timestamp(),
                        Err(e) => {
                            eprintln!("Failed to parse end time: {}", e);
                            tool_answers.insert(tool_call_id, "Invalid end time format. Please use RFC3339 format.".to_string());
                            continue;
                        }
                    };

                    match crate::utils::whatsapp_utils::fetch_whatsapp_messages(
                        &state,
                        user.id,
                        start_time,
                        end_time,
                    ).await {
                        Ok(messages) => {
                            if messages.is_empty() {
                                tool_answers.insert(tool_call_id, "No WhatsApp messages found for this time period.".to_string());
                            } else {
                                let mut response = String::new();
                                for (i, msg) in messages.iter().take(15).enumerate() {
                                    let content = if msg.content.len() > 100 {
                                        format!("{}...", &msg.content[..97])
                                    } else {
                                        msg.content.clone()
                                    };
                                    
                                    if i == 0 {
                                        response.push_str(&format!("{}. {} in {} at {}:\n{}", 
                                            i + 1, 
                                            msg.sender_display_name,
                                            msg.room_name,
                                            msg.formatted_timestamp,
                                            content
                                        ));
                                    } else {
                                        response.push_str(&format!("\n\n{}. {} in {} at {}:\n{}", 
                                            i + 1, 
                                            msg.sender_display_name,
                                            msg.room_name,
                                            msg.formatted_timestamp,
                                            content
                                        ));
                                    }
                                }
                                
                                if messages.len() > 15 {
                                    response.push_str(&format!("\n\n(+ {} more messages)", messages.len() - 15));
                                }
                                
                                tool_answers.insert(tool_call_id, response);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to fetch WhatsApp messages: {}", e);
                            tool_answers.insert(tool_call_id, 
                                "Failed to fetch WhatsApp messages. Please make sure you're connected to WhatsApp bridge.".to_string()
                            );
                        }
                    }
                } else if name == "scan_qr_code" {
                    println!("Executing scan_qr_code tool call with url: {:#?}", image_url);

                    // Only proceed if we have an image URL from the message
                    if let Some(url) = image_url.as_ref() {
                        match crate::utils::qr_utils::scan_qr_code(url).await {
                            Ok(data) => {
                                if data.is_empty() {
                                    tool_answers.insert(tool_call_id, "No QR code found in the image.".to_string());
                                } else {
                                    tool_answers.insert(tool_call_id, format!("QR code content: {}", data));
                                }
                            },
                            Err(e) => {
                                eprintln!("Failed to scan QR code: {}", e);
                                tool_answers.insert(tool_call_id, 
                                    "Failed to scan QR code from the image. Please make sure the QR code is clearly visible.".to_string()
                                );
                            }
                        }
                    } else {
                        tool_answers.insert(tool_call_id, 
                            "No image was provided in the message. Please send an image containing a QR code.".to_string()
                        );
              }
                } else if name == "delete_sms_conversation_history" {
                    println!("Executing delete_sms_conversation_history tool call");

                    match crate::api::twilio_utils::delete_bot_conversations(&user.phone_number, &user).await {
                        Ok(_) => {
                            tool_answers.insert(tool_call_id, "Successfully deleted all bot conversations.".to_string());
                        }
                        Err(e) => {
                            eprintln!("Failed to delete bot conversations: {}", e);
                            tool_answers.insert(tool_call_id, 
                                format!("Failed to delete conversations: {}", e)
                            );
                        }
                    }
                } else if name == "calendar" {
                    println!("Executing calendar tool call");
                    let c: CalendarTimeFrame = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            eprintln!("Failed to parse calendar question: {}", e);
                            continue;
                        }
                    };

                    match crate::handlers::google_calendar::handle_calendar_fetching(&state, user.id, &c.start, &c.end).await {
                        Ok(Json(response)) => {
                            if let Some(events) = response.get("events") {
                                tool_answers.insert(tool_call_id, format!("Here are your calendar events: {}", events.to_string()));
                            } else {
                                tool_answers.insert(tool_call_id, "No events found for this time period.".to_string());
                            }

                        }
                        Err((status, json_error)) => {
                            let error_message = match status {
                                StatusCode::BAD_REQUEST => "No active Google Calendar connection found. Visit the website to connect.",
                                StatusCode::UNAUTHORIZED => "Your calendar connection needs to be renewed. Please reconnect on the website.",
                                _ => "Failed to fetch calendar events. Please try again later.",
                            };
                            tool_answers.insert(tool_call_id, error_message.to_string());
                            eprintln!("Failed to fetch calendar events: {:?}", json_error);
                        }
                    }
                }
            }


            let mut follow_up_messages = completion_messages.clone();
            // Add the assistant's message with tool calls
            follow_up_messages.push(chat_completion::ChatCompletionMessage {
                role: chat_completion::MessageRole::assistant,
                content: chat_completion::Content::Text(result.choices[0].message.content.clone().unwrap_or_default()),
                name: None,
                tool_calls: result.choices[0].message.tool_calls.clone(),
                tool_call_id: None,
            });
            // Add the tool response
            if let Some(tool_calls) = &result.choices[0].message.tool_calls {
                for tool_call in tool_calls {
                    let tool_answer = match tool_answers.get(&tool_call.id) {
                        Some(ans) => ans.clone(),
                        None => "".to_string(),
                    };
                    follow_up_messages.push(chat_completion::ChatCompletionMessage {
                        role: chat_completion::MessageRole::tool,
                        content: chat_completion::Content::Text(tool_answer),
                        name: None,
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                    });
                }
            }

            println!("Making follow-up request to model with tool call answers");
            let follow_up_req = chat_completion::ChatCompletionRequest::new(
                GPT4_O.to_string(),
                follow_up_messages,
            )
            .max_tokens(100); // Consistent token limit for follow-up messages
            println!("Follow-up request created");

            match client.chat_completion(follow_up_req).await {
                Ok(follow_up_result) => {
                    println!("Received follow-up response from model");
                    let response = follow_up_result.choices[0].message.content.clone().unwrap_or_default();
                    response
                }
                Err(e) => {
                    eprintln!("Failed to get follow-up completion: {}", e);
                    fail = true;
                    tool_answers.values().next()
                        .map(|ans| format!("Based on my research: {} (you were not charged for this message)", ans.chars().take(370).collect::<String>()))
                        .unwrap_or_else(|| "I apologize, but I encountered an error processing your request. (you were not charged for this message)".to_string())
                }
            }
        }
        Some(chat_completion::FinishReason::length) => {
            fail = true;
            "I apologize, but my response was too long. Could you please ask your question in a more specific way? (you were not charged for this message)".to_string()
        }
        Some(chat_completion::FinishReason::content_filter) => {
            fail = true;
            "I apologize, but I cannot provide an answer to that question due to content restrictions. (you were not charged for this message)".to_string()
        }
        Some(chat_completion::FinishReason::null) => {
            fail = true;
            "I apologize, but something went wrong while processing your request. (you were not charged for this message)".to_string()
        }
    };

    let processing_time_secs = start_time.elapsed().as_secs(); // Calculate processing time

    // Check if the latest AI message in history had a free reply string
    let should_charge = if payload.body.to_lowercase().starts_with("forget") {
        true
    } else {
        !messages.iter()
            .rev() // Reverse to get latest messages first
            .find(|msg| msg.author == "lightfriend") // Find the latest AI message
            .map(|msg| msg.body.contains("(free reply)")) // Check if it contains "(free reply)"
            .unwrap_or(false) // Default to charging if no AI message found
    };

    // Create clarification check function properties
    let mut clarify_properties = HashMap::new();
    clarify_properties.insert(
        "is_clarifying".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("Whether the AI's response is asking a clarifying question instead of providing an answer".to_string()),
            ..Default::default()
        }),
    );
    clarify_properties.insert(
        "explanation".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Brief explanation of why this is or isn't a clarifying question without revealing conversation content.".to_string()),
            ..Default::default()
        }),
    );

    // Define clarification check tool
    let clarify_tools = vec![
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("check_clarification"),
                description: Some(String::from(
                    "Determines if the AI's response is asking a clarifying question instead of providing an answer"
                )),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(clarify_properties),
                    required: Some(vec![String::from("is_clarifying")]),
                },
            },
        },
    ];

    // Create clarification check messages with more context
    let mut clarify_messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(
                "You are an evaluator that determines if an AI response is asking for REQUIRED information OR is seeking for CONFIRMATION to complete the UNFINISHED task. Unfinished task is an answer where the AI did not provide ANY useful information yet to the user. Default to FALSE otherwise.\n\n\
                Examples of TRUE clarifying questions:\n\
                - User: 'Send a message to mom' -> AI: 'I see multiple contacts named mom. Which one should I send the message to?'\n\
                - User: 'Check my calendar' -> AI: 'For which date range would you like me to check your calendar?'\n\
                - User: 'What's the weather?' -> AI: 'Which location would you like the weather for?'\n\n\
                - User: 'Can you send message hey hows it going to mom?' -> AI: 'I found the contact \'Mom\' on WhatsApp. Do you want me to send \'hey hows it going\' to this contact?'\n\n\
                Examples that should be FALSE (complete answers with optional follow-ups):\n\
                - User: 'Show contacts named mom' -> AI: 'You have 2 contacts: 1. Mom (mobile) 2. Mom (work).'\n\
                - User: 'Get my recent emails' -> AI: 'Here are your latest emails: [email list]. Would you like to see more?'\n\
                - User: 'Check weather in London' -> AI: 'It's sunny and 20°C in London. Would you like to check another city?'\n\n\
                Key rules:\n\
                2. Follow-up questions after answering the original question are NOT clarifying questions\n\
                3. Only mark TRUE if the AI is asking to confirm the data it got from the user or it CANNOT provide an answer without more information\n\
                4. When in doubt, return FALSE".to_string(),
            ),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    // Add up to 3 previous message pairs for context
    let context_messages: Vec<_> = messages.iter()
        .rev()
        .take(6) // Take 6 messages (3 pairs of user-assistant exchanges)
        .collect();

    if !context_messages.is_empty() {
        clarify_messages.push(chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(
                format!(
                    "Previous conversation:\n{}",
                    context_messages.iter()
                        .rev() // Reverse back to chronological order
                        .map(|msg| format!("[{}]: {}", 
                            if msg.author == "lightfriend" { "AI" } else { "User" },
                            msg.body
                        ))
                        .collect::<Vec<String>>()
                        .join("\n")
                )
            ),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });
    }

    // Add the current exchange
    clarify_messages.push(chat_completion::ChatCompletionMessage {
        role: chat_completion::MessageRole::user,
        content: chat_completion::Content::Text(format!(
            "Current exchange:\nUser: {}\nAI: {}",
            payload.body,
            final_response
        )),
        name: None,
        tool_calls: None,
        tool_call_id: None,
    });

    let clarify_req = chat_completion::ChatCompletionRequest::new(
        "openai/gpt-4o-mini".to_string(),
        clarify_messages,
    )
    .tools(clarify_tools)
    .tool_choice(chat_completion::ToolChoiceType::Required)
    .max_tokens(100);

    #[derive(Deserialize)]
    struct ClarifyResponse {
        #[serde(deserialize_with = "deserialize_bool")]
        is_clarifying: bool,
        explanation: Option<String>,
    }

    // If message starts with "forget", skip clarification check
    let (is_clarifying, clarify_explanation) = match client.chat_completion(clarify_req).await {
            Ok(result) => {
                println!("Got clarification check response from model");
                if let Some(tool_calls) = result.choices[0].message.tool_calls.as_ref() {
                    if let Some(first_call) = tool_calls.first() {
                        if let Some(args) = &first_call.function.arguments {
                            match serde_json::from_str::<ClarifyResponse>(args) {
                                Ok(clarify) => {
                                    println!("Clarification check result: is_clarifying={}, explanation={:?}", 
                                        clarify.is_clarifying, clarify.explanation);
                                    (clarify.is_clarifying, clarify.explanation)
                                },
                                Err(e) => {
                                    println!("Failed to parse clarification response: {}", e);
                                    (false, Some("Failed to parse clarification check".to_string()))
                                },
                            }
                        } else {
                            println!("No arguments found in clarification tool call");
                            (false, Some("Missing clarification check arguments".to_string()))
                        }
                    } else {
                        println!("No clarification tool calls found");
                        (false, Some("No clarification check tool calls found".to_string()))
                    }
                } else {
                    println!("No tool calls section in clarification response");
                    (false, Some("No clarification check tool calls received".to_string()))
                }
            }
            Err(e) => {
                println!("Failed to get clarification check response: {}", e);
                (false, Some("Failed to get clarification check response".to_string()))
            }
        };

    

    let mut eval_properties = HashMap::new();
    eval_properties.insert(
        "success".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("Whether the response was successful and provided the information user asked for. Note that the information might not look like success(whatsapp message fetch returns missed call notice), but should still be considered successful.".to_string()),
            ..Default::default()
        }),
    );
    eval_properties.insert(
        "reason".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Reason for failure if success is false, explaining the issue without revealing conversation content".to_string()),
            ..Default::default()
        }),
    );
    // Define evaluation tool
    let eval_tools = vec![
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("evaluate_response"),
                description: Some(String::from(
                    "Evaluates the AI response based on success."
                )),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(eval_properties),
                    required: Some(vec![String::from("success")]),
                },
            },
        },
    ];

    // Create evaluation messages
    let eval_messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(
                "You are a conversation evaluator. Assess the latest user's query in the context of the conversation history and the AI's response to it. Use the evaluate_response function to provide feedback.\n\n\
                ### Guidelines:\n\
                - **Success**: True if the AI successfully answered the user's request; false otherwise.".to_string(),
            ),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(format!(
                "Conversation history: {}\nLatest user message: {}\nAI response: {}",
                messages.iter()
                    .map(|msg| format!("[{}]: {}", 
                        if msg.author == "lightfriend" { "AI" } else { "User" },
                        if msg.body.chars().count() > 50 {
                            format!("{}...", msg.body.chars().take(50).collect::<String>())
                        } else {
                            msg.body.clone()
                        }
                    ))
                    .collect::<Vec<String>>()
                    .join("\n"),
                payload.body,
                final_response
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let eval_req = chat_completion::ChatCompletionRequest::new(
        "openai/gpt-4o-mini".to_string(),
        eval_messages,
    )
    .tools(eval_tools)
    .tool_choice(chat_completion::ToolChoiceType::Required)
    .max_tokens(200);

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolValue {
        Bool(bool),
        String(String),
    }

    impl From<BoolValue> for bool {
        fn from(value: BoolValue) -> Self {
            match value {
                BoolValue::Bool(b) => b,
                BoolValue::String(s) => s.to_lowercase() == "true",
            }
        }
    }

    #[derive(Deserialize)]
    struct EvalResponse {
        #[serde(deserialize_with = "deserialize_bool")]
        success: bool,
        reason: Option<String>,
    }

    fn deserialize_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(BoolValue::deserialize(deserializer)?.into())
    }

    let (eval_result, eval_reason) = match client.chat_completion(eval_req).await {
        Ok(result) => {
            println!("Got evaluation response from model");
            if let Some(tool_calls) = result.choices[0].message.tool_calls.as_ref() {
                println!("Found tool calls in evaluation response");
                if let Some(first_call) = tool_calls.first() {
                    println!("Processing first tool call");
                    if let Some(args) = &first_call.function.arguments {
                        println!("Tool call arguments: {}", args);
                        match serde_json::from_str::<EvalResponse>(args) {
                            Ok(eval) => {
                                println!("Successfully parsed evaluation response: success={}, reason={:?}", 
                                    eval.success, eval.reason);
                                (eval.success, eval.reason)
                            },
                            Err(e) => {
                                println!("Failed to parse evaluation response: {}, falling back to default", e);
                                (!fail, Some("Failed to parse evaluation response".to_string()))
                            },
                        }
                    } else {
                        println!("No arguments found in tool call");
                        (!fail, Some("Missing evaluation arguments".to_string()))
                    }
                } else {
                    println!("No tool calls found in response");
                    (!fail, Some("No evaluation tool calls found".to_string()))
                }
            } else {
                println!("No tool calls section in response");
                (!fail, Some("No evaluation tool calls received".to_string()))
            }
        }
        Err(e) => {
            println!("Failed to get evaluation response: {}", e);
            (!fail, Some("Failed to get evaluation response".to_string()))
        }
    };

    

    let final_response_with_notice = if is_clarifying {
        format!("{} (free reply)", final_response)
    } else {
        final_response
    };
    println!("is_clarifying message: {}", is_clarifying);
    if is_clarifying {
        redact_the_body = false;
    }

    let status = if should_charge {"charging".to_string()} else {"this was free reply".to_string()};
    println!("STATUS: {}", status);
    let mut final_eval: String = "".to_string();
    if let Some(eval) = eval_reason {
        if let Some(clarify_expl) = clarify_explanation {
            final_eval = format!("success reason: {}; clarifying explanation: {}",eval,clarify_expl);
        } else {
            final_eval = format!("success reason: {}", eval);
        }
    } else if let Some(clarify_expl) = clarify_explanation {
        final_eval = format!("clarifying reason: {}", clarify_expl);
    }
    println!("FINAL_EVAL: {}", final_eval);

    // If in test mode, skip sending the actual message and return the response directly
    if is_test {
        // Log the test usage without actually sending the message
        if let Err(e) = state.user_repository.log_usage(
            user.id,
            None,  // No message SID in test mode
            "sms_test".to_string(),
            None,
            Some(processing_time_secs as i32),
            Some(eval_result),
            Some(final_eval),
            Some(status),
            None,
            None
        ) {
            eprintln!("Failed to log test SMS usage: {}", e);
        }

        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            axum::Json(TwilioResponse {
                message: final_response_with_notice,
            })
        );
    }

    // Send the actual message if not in test mode
    match crate::api::twilio_utils::send_conversation_message(&conversation.conversation_sid, &conversation.twilio_number, &final_response_with_notice, redact_the_body, &user).await {
        Ok(message_sid) => {
            // Always log the SMS usage metadata and eval(no content!)
            println!("status of the message: {}", status);
            if let Err(e) = state.user_repository.log_usage(
                user.id,
                Some(message_sid),
                "sms".to_string(),
                None,
                Some(processing_time_secs as i32),
                Some(eval_result),
                Some(final_eval),
                Some(status),
                None,
                None,
            ) {
                eprintln!("Failed to log SMS usage: {}", e);
                // Continue execution even if logging fails
            }

            if should_charge {
                // Only deduct credits if we should charge
                if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user.id, "message", None) {
                    eprintln!("Failed to deduct user credits: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        axum::Json(TwilioResponse {
                            message: "Failed to process credits points".to_string(),
                        })
                    );
                }
                        
                match state.user_repository.is_credits_under_threshold(user.id) {
                    Ok(is_under) => {
                        if is_under {
                            println!("User {} credits is under threshold, attempting automatic charge", user.id);
                            // Get user information
                            if user.charge_when_under {
                                use axum::extract::{State, Path};
                                let state_clone = Arc::clone(&state);
                                tokio::spawn(async move {
                                    let _ = crate::handlers::stripe_handlers::automatic_charge(
                                        State(state_clone),
                                        Path(user.id),
                                    ).await;
                                    println!("Recharged the user successfully back up!");
                                });
                                println!("recharged the user successfully back up!");
                            }
                        }
                    },
                    Err(e) => eprintln!("Failed to check if user credits is under threshold: {}", e),
                }
            }

            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Message sent successfully".to_string(),
                })
            )
        }
        Err(e) => {
            eprintln!("Failed to send conversation message: {}", e);
            // Log the failed attempt with error message in status
            let error_status = format!("failed to send: {}", e);
            if let Err(log_err) = state.user_repository.log_usage(
                user.id,
                None,
                "sms".to_string(),
                None,
                Some(processing_time_secs as i32),
                Some(false),  // Mark as unsuccessful
                Some(final_eval),
                Some(error_status),
                None,
                None,
            ) {
                eprintln!("Failed to log SMS usage after send error: {}", log_err);
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Failed to send message".to_string(),
                })
            )
        }
    }
}

