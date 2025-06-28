use axum::{
    Json,
    extract::State,
    response::Response,
    http::{StatusCode, Request, HeaderMap},
    body::Body,
};
use tracing::error;
use axum::middleware;
use std::sync::Arc;
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use chrono::TimeZone;
use crate::handlers::imap_handlers::{fetch_emails_imap, fetch_single_email_imap};
use crate::models::user_models::User;


#[derive(Debug, Deserialize)]
pub struct LocationCallPayload {
    location: String,
    units: String,
}

#[derive(Debug, Deserialize)]
pub struct GmailFetchPayload {
    user_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppFetchPayload {
    start_time: String,  // RFC3339 format: "2024-03-16T00:00:00Z"
    end_time: String,    // RFC3339 format: "2024-03-16T00:00:00Z"
}

#[derive(Debug, Deserialize)]
pub struct MessageCallPayload {
    message: String,
    email_id: Option<String>
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NotificationCallPayload {
    agent_id: String,
    agent_phone_number_id: String,
    to_number: String,
    conversation_initiation_client_data: ConversationInitiationClientData,
}

#[derive(Debug, Deserialize)]
pub struct AssistantPayload {
    agent_id: String,
    call_sid: String,
    called_number: String,
    caller_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ConversationInitiationClientData {
    r#type: String,
    conversation_config_override: ConversationConfig,
    dynamic_variables: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ConversationConfig {
    agent: AgentConfig,
    tts: VoiceId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AgentConfig {
    first_message: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VoiceId {
    voice_id: String,
}


pub async fn validate_elevenlabs_secret(
    headers: HeaderMap,
    request: Request<Body>,
    next: middleware::Next,
) -> Result<Response, StatusCode> {
    tracing::debug!("\n=== Starting Elevenlabs Secret Validation ===");
    
    let secret_key = match std::env::var("ELEVENLABS_SERVER_URL_SECRET") {
        Ok(key) => {
            tracing::debug!("✅ Successfully retrieved ELEVENLABS_SERVER_URL_SECRET");
            key
        },
        Err(e) => {
            tracing::error!("❌ Failed to get ELEVENLABS_SERVER_URL_SECRET: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    match headers.get("x-elevenlabs-secret") {
        Some(header_value) => {
            tracing::debug!("🔍 Found x-elevenlabs-secret header");
            match header_value.to_str() {
                Ok(value) => {
                    if value == secret_key {
                        tracing::debug!("✅ Secret validation successful");
                        Ok(next.run(request).await)
                    } else {
                        tracing::error!("❌ Invalid secret provided");
                        Err(StatusCode::UNAUTHORIZED)
                    }
                },
                Err(e) => {
                    tracing::error!("❌ Error converting header to string: {}", e);
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        },
        None => {
            tracing::error!("❌ No x-elevenlabs-secret header found");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

use jiff::{Timestamp, ToSpan};

pub fn get_offset_with_jiff(timezone_str: &str) -> Result<(i32, i32), jiff::Error> {
    let time = Timestamp::now();
    let zoned = time.in_tz(timezone_str)?;
    
    // Get offset information
    let offset_seconds = zoned.offset().seconds();
    let hours = offset_seconds / 3600;
    let minutes = (offset_seconds.abs() % 3600) / 60;
    
    Ok((hours, minutes))
}


pub async fn fetch_assistant(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AssistantPayload>,
) -> Result<Json<ConversationInitiationClientData>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Received assistant request:");
    let agent_id = payload.agent_id;
    let call_sid = payload.call_sid;
    let called_number = payload.called_number;
    let caller_number = payload.caller_id;


    let us_voice_id = std::env::var("US_VOICE_ID").expect("US_VOICE_ID not set");
    let fi_voice_id = std::env::var("FI_VOICE_ID").expect("FI_VOICE_ID not set");
    let de_voice_id = std::env::var("DE_VOICE_ID").expect("DE_VOICE_ID not set");

    let mut dynamic_variables = HashMap::new();
    let mut conversation_config_override = ConversationConfig {
        agent: AgentConfig {
            first_message: "Hello {{name}}!".to_string(),
        },
        tts: VoiceId {
            voice_id: us_voice_id.clone(),
        },
    };


    match state.user_core.find_by_phone_number(&caller_number) {
        Ok(Some(user)) => {
            tracing::debug!("Found user by their phone number");
            
            let user_settings = match state.user_core.get_user_settings(user.id) {
                Ok(settings) => settings,
                Err(e) => {
                    error!("Failed to get user settings: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to get conversation",
                            "message": "Internal server error"
                        }))
                    ));
                }
            };

            // If user is not verified, verify them
            if !user.verified {
                if let Err(e) = state.user_core.verify_user(user.id) {
                    tracing::error!("Error verifying user: {}", e);
                    // Continue even if verification fails
                } else {
                    if user_settings.agent_language == "fi" {
                        conversation_config_override.agent.first_message = "Tervetuloa! Numerosi on nyt vahvistettu. Miten voin auttaa?".to_string(); 
                        conversation_config_override.tts.voice_id = fi_voice_id.clone();
                    } else if user_settings.agent_language == "de" {
                        conversation_config_override.agent.first_message = "Willkommen! Ihre Nummer ist jetzt verifiziert. Wie kann ich Ihnen helfen?".to_string(); 
                        conversation_config_override.tts.voice_id = de_voice_id.clone();
                    } else {
                        conversation_config_override.agent.first_message = "Welcome! Your number is now verified. Anyways, how can I help?".to_string(); 
                        conversation_config_override.tts.voice_id = us_voice_id.clone();
                    }
                }
            } else if let Err(msg) = crate::utils::usage::check_user_credits(&state, &user, "voice").await {
                // Get conversation for the user
                let conversation = match state.user_conversations.get_conversation(
                    &user, 
                    user.preferred_number.clone().unwrap_or_else(|| std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set"))
                ).await {
                    Ok(conv) => conv,
                    Err(e) => {
                        error!("Failed to get conversation: {}", e);
                        return Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({
                                "error": "Failed to get conversation",
                                "message": "Internal server error"
                            }))
                        ));
                    }
                };

                // Send insufficient credits message
                let error_message = "Insufficient credits to make a voice call".to_string();
                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                    &conversation.conversation_sid,
                    &conversation.twilio_number,
                    &error_message,
                    false, // Don't redact since there is nothing there 
                    None,
                    &user,
                ).await {
                    error!("Failed to send insufficient credits message: {}", e);
                }

                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": "Insufficient credits balance",
                        "message": "Please add more credits to your account to continue on lightfriend website",
                    }))
                ));
            }

            if user_settings.agent_language == "fi" {
                conversation_config_override.agent.first_message = "Moi {{name}}!".to_string(); 
                conversation_config_override.tts.voice_id = fi_voice_id.clone();
            } else if user_settings.agent_language == "de" {
                conversation_config_override.agent.first_message = "Hallo {{name}}!".to_string(); 
                conversation_config_override.tts.voice_id = de_voice_id.clone();
            }

            let nickname = match user.nickname {
                Some(nickname) => nickname,
                None => "".to_string()
            };
            let user_info = match user_settings.info {
                Some(info) => info,
                None => "".to_string()
            };

            dynamic_variables.insert("name".to_string(), json!(nickname));
            dynamic_variables.insert("user_info".to_string(), json!(user_info));
            dynamic_variables.insert("user_id".to_string(), json!(user.id));
            dynamic_variables.insert("email_id".to_string(), json!("-1".to_string()));
            dynamic_variables.insert("content_type".to_string(), json!("".to_string()));
            dynamic_variables.insert("notification_message".to_string(), json!("".to_string()));

            // Get timezone from user info or default to UTC
            let timezone_str = match user_settings.timezone {
                Some(ref tz) => tz.as_str(),
                None => "UTC",
            };

            // Get timezone offset using jiff
            let (hours, minutes) = match get_offset_with_jiff(timezone_str) {
                Ok((h, m)) => (h, m),
                Err(_) => {
                    tracing::error!("Failed to get timezone offset for {}, defaulting to UTC", timezone_str);
                    (0, 0) // UTC default
                }
            };

            // Format offset string (e.g., "+02:00" or "-05:30")
            let offset = format!("{}{:02}:{:02}", 
                if hours >= 0 { "+" } else { "-" },
                hours.abs(),
                minutes.abs()
            );

            dynamic_variables.insert("timezone".to_string(), json!(timezone_str));
            dynamic_variables.insert("timezone_offset_from_utc".to_string(), json!(offset));


            let charge_back_threshold= std::env::var("CHARGE_BACK_THRESHOLD")
                .expect("CHARGE_BACK_THRESHOLD not set")
                .parse::<f32>()
                .unwrap_or(2.00);
            let voice_second_cost = std::env::var("VOICE_SECOND_COST")
                .expect("VOICE_SECOND_COST not set")
                .parse::<f32>()
                .unwrap_or(0.0033);

            let user_current_credits_to_threshold = user.credits - charge_back_threshold;
            let seconds_to_threshold = (user_current_credits_to_threshold / voice_second_cost) as i32;
            // following just so it doesn't go negative although i don't think it matters
            let recharge_threshold_timestamp: i32 = (chrono::Utc::now().timestamp() as i32) + seconds_to_threshold;

            let seconds_to_zero_credits= (user.credits / voice_second_cost) as i32;
            let zero_credits_timestamp: i32 = (chrono::Utc::now().timestamp() as i32) + seconds_to_zero_credits as i32;

            // log usage and start call
            if let Err(e) = state.user_repository.log_usage(
                user.id,
                Some(call_sid),
                "call".to_string(),
                None,
                None,
                None,
                None,
                Some("ongoing".to_string()),
                Some(recharge_threshold_timestamp),
                Some(zero_credits_timestamp),
            ) {
                tracing::error!("Failed to log call usage: {}", e);
                // Continue execution even if logging fails
            }

        },
        Ok(None) => {
            tracing::debug!("No user found for number: {}", caller_number);
            dynamic_variables.insert("name".to_string(), json!(""));
            dynamic_variables.insert("user_info".to_string(), json!("new user"));
        },
        Err(e) => {
            tracing::error!("Error looking up user: {}", e);
            dynamic_variables.insert("name".to_string(), json!("Guest"));
            dynamic_variables.insert("user_info".to_string(), json!({
                "error": "Database error"
            }));
        }
    }

    dynamic_variables.insert("now".to_string(), json!(format!("{}", chrono::Utc::now())));

    let payload = ConversationInitiationClientData {
        r#type: "conversation_initiation_client_data".to_string(),
        conversation_config_override,
        dynamic_variables,
    };

    Ok(Json(payload))
}


#[derive(Deserialize)]
pub struct WaitingCheckPayload {
    pub content: String,
    pub due_date: Option<String>,
    pub remove_when_found: Option<bool>,
    pub user_id: i32,
}

pub async fn handle_create_waiting_check_email_tool_call(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WaitingCheckPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Received waiting check creation request");

    // Handle due_date: parse provided string or use default (2 weeks from now)
    let due_date_utc = match payload.due_date {
        Some(date_str) => {
            // Validate the provided date string
            match chrono::DateTime::parse_from_rfc3339(&date_str) {
                Ok(_) => date_str, // If valid RFC3339, use as-is
                Err(_) => {
                    tracing::error!("Invalid due_date format provided, using default");
                    let two_weeks = chrono::Duration::weeks(2);
                    (chrono::Utc::now() + two_weeks)
                        .format("%Y-%m-%dT00:00:00Z")
                        .to_string()
                }
            }
        }
        None => {
            let two_weeks = chrono::Duration::weeks(2);
            (chrono::Utc::now() + two_weeks)
                .format("%Y-%m-%dT00:00:00Z")
                .to_string()
        }
    };

    // Convert UTC string to timestamp integer
    let due_date_timestamp = chrono::DateTime::parse_from_rfc3339(&due_date_utc)
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|e| {
            error!("Failed to parse due_date: {}", e);
            (chrono::Utc::now() + chrono::Duration::weeks(2)).timestamp()
        }) as i32;

    // Default remove_when_found to true if not provided
    let remove_when_found = payload.remove_when_found.unwrap_or(true);

    // Verify user exists
    match state.user_core.find_by_id(payload.user_id) {
        Ok(Some(_user)) => {
            let new_check = crate::models::user_models::NewWaitingCheck {
                user_id: payload.user_id,
                due_date: due_date_timestamp,
                content: payload.content,
                remove_when_found,
                service_type: "imap".to_string(),
            };

            match state.user_repository.create_waiting_check(&new_check) {
                Ok(_) => {
                    tracing::debug!("Successfully created waiting check for user: {} with due date: {}", 
                        payload.user_id, due_date_utc);
                    Ok(Json(json!({
                        "response": "I'll keep an eye out for that in your emails and notify you when I find it.",
                        "status": "success",
                        "user_id": payload.user_id,
                        "due_date": due_date_utc
                    })))
                },
                Err(e) => {
                    error!("Failed to create waiting check: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to create waiting check",
                            "details": e.to_string()
                        }))
                    ))
                }
            }
        },
        Ok(None) => {
            tracing::error!("User not found: {}", payload.user_id);
            Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "User not found"
                }))
            ))
        },
        Err(e) => {
            error!("Error fetching user: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user",
                    "details": e.to_string()
                }))
            ))
        }
    }
}

pub async fn handle_email_fetch_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Extract and parse user_id from query params
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid or missing user_id parameter"
                }))
            ));
        }
    };
    tracing::debug!("Received email fetch request for user: {}", user_id);
    
    match crate::handlers::imap_handlers::fetch_emails_imap(&state, user_id, true, Some(10), false).await {
        Ok(emails) => {
            if emails.is_empty() {
                return Ok(Json(json!({
                    "response": "I don't see any recent emails in your inbox.",
                    "emails": [],
                    "total_count": 0
                })));
            }

            // Format emails for voice response in a more natural way
            let mut response_text = format!(
                "I found {} recent emails in your inbox. ", 
                emails.len()
            );

            // Group emails by read status
            let unread_count = emails.iter().filter(|e| !e.is_read).count();
            if unread_count > 0 {
                response_text.push_str(&format!(
                    "{} of them {} unread. ",
                    unread_count,
                    if unread_count == 1 { "is" } else { "are" }
                ));
            }

            // Add details for each email in a conversational way
            for (i, email) in emails.iter().enumerate() {
                let from = email.from.as_deref().unwrap_or("an unknown sender");
                let subject = email.subject.as_deref().unwrap_or("no subject");
                let date = email.date_formatted.as_deref().unwrap_or("recently");
                
                // Truncate body if too long and clean it up
                let body = email.body.as_ref()
                    .map(|b| {
                        let cleaned = b.replace('\n', " ").replace('\r', " ");
                        let chars: Vec<char> = cleaned.chars().collect();
                        if chars.len() > 150 {
                            let truncated: String = chars.into_iter().take(150).collect();
                            format!("{}...", truncated)
                        } else {
                            cleaned
                        }
                    })
                    .unwrap_or_else(|| "no content".to_string());
                // Format each email in a more natural way
                let email_intro = if i == 0 {
                    "The most recent email is"
                } else if i == emails.len() - 1 {
                    "And finally"
                } else {
                    "Next"
                };

                response_text.push_str(&format!(
                    "{} from {}, sent {}. The subject is '{}'. {}. ",
                    email_intro,
                    from,
                    date,
                    subject,
                    if email.is_read {
                        format!("Here's what it says: {}", body)
                    } else {
                        format!("This unread email says: {}", body)
                    }
                ));
            }

            Ok(Json(json!({
                "response": response_text,
                "emails": emails.iter().map(|email| {
                    json!({
                        "id": email.id,
                        "subject": email.subject,
                        "from": email.from,
                        "from_email": email.from_email,
                        "date": email.date.map(|dt| dt.to_rfc3339()),
                        "date_formatted": email.date_formatted,
                        "body": email.body,
                        "is_read": email.is_read
                    })
                }).collect::<Vec<_>>(),
                "total_count": emails.len(),
                "unread_count": unread_count
            })))
        },
        Err(e) => {
            error!("Failed to fetch emails: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch emails",
                    "details": format!("{:?}", e)
                }))
            ))
        }
    }
}



use base64::Engine as _;

pub async fn handle_send_sms_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<MessageCallPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Received SMS send request");
    
    // Get user_id from query params
    let user_id_str = match params.get("user_id") {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing user_id query parameter"
                }))
            ));
        }
    };

    // Convert String to i32
    let user_id: i32 = match user_id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid user_id format, must be an integer"
                }))
            ));
        }
    };

    // Fetch user from user_repository
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "User not found"
                }))
            ));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user"
                }))
            ));
        }
    };

    // Get conversation for the user
    let conversation = match state.user_conversations.get_conversation(&user, user.preferred_number.clone().unwrap()).await {
        Ok(conv) => conv,
        Err(e) => {
            error!("Failed to get conversation: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to get or create conversation"
                }))
            ));
        }
    };

    let mut message_sids = Vec::new();


    // Handle email attachments if email_id is provided - spawn as background task
    if let Some(email_id) = payload.email_id.clone() {
        tracing::debug!("Spawning background task for email attachments processing for email ID: {}", email_id);
        
        let state_clone = Arc::clone(&state);
        let conversation_clone = conversation.clone();
        let user_clone = user.clone();
        
        tokio::spawn(async move {
            tracing::debug!("Background task: Fetching email attachments for email ID: {}", email_id);
            
            match fetch_single_email_imap(&state_clone, user_clone.id, &email_id).await {
                Ok(email) => {

                }
                Err(e) => {
                    error!("Background task: Failed to fetch email: {:?}", e);
                }
            }
        });
    }

    // Send the main message using Twilio
    match crate::api::twilio_utils::send_conversation_message(
        &conversation.conversation_sid,
        &conversation.twilio_number,
        &payload.message,
        true,
        None,
        &user,
    ).await {
        Ok(message_sid) => {
            message_sids.push(message_sid.clone());
            tracing::debug!("Successfully sent main SMS with SID: {}", message_sid);
            
            let attachment_info = if payload.email_id.is_some() {
                "Email attachments are being processed in the background and will be sent shortly."
            } else {
                "No email attachments to process."
            };
            
            Ok(Json(json!({
                "status": "success",
                "message_sid": message_sid,
                "conversation_sid": conversation.conversation_sid,
                "attachment_processing": attachment_info,
                "total_messages_sent": message_sids.len(),
                "all_message_sids": message_sids
            })))
        }
        Err(e) => {
            error!("Failed to send SMS: {}", e);
            

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to send message",
                    "details": e.to_string()
                }))
            ))
        }
    }
}

pub async fn handle_shazam_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Received shazam request with params: {:?}", params);
    
    // Get user_id from query params
    let user_id_str = match params.get("user_id") {
        Some(id) => {
            tracing::debug!("Found user_id in params: {}", id);
            id
        },
        None => {
            tracing::error!("Missing user_id in query parameters");
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing user_id query parameter"
                }))
            ));
        }
    };

    // Convert String to i32
    let user_id: i32 = match user_id_str.parse() {
        Ok(id) => {
            tracing::debug!("Successfully parsed user_id to integer: {}", id);
            id
        },
        Err(e) => {
            tracing::error!("Failed to parse user_id '{}' to integer: {}", user_id_str, e);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid user_id format, must be an integer"
                }))
            ));
        }
    };

    // Spawn a new thread to handle the Shazam call
    let state_clone = Arc::clone(&state);
    let user_id_string = user_id.to_string();
    
    tracing::info!("Spawning new task for Shazam call for user_id: {}", user_id);
    tokio::spawn(async move {
        tracing::debug!("Starting Shazam call for user_id: {}", user_id_string);
        crate::api::shazam_call::start_call_for_user(
            axum::extract::Path(user_id_string),
            axum::extract::State(state_clone),
        ).await;
        tracing::debug!("Completed Shazam call task for user_id: {}", user_id);
    });

    tracing::info!("Successfully initiated Shazam call for user_id: {}", user_id);
    Ok(Json(json!({
        "status": "success",
        "message": "Shazam call initiated",
        "user_id": user_id
    })))
}

pub async fn handle_perplexity_tool_call(
    State(_state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<MessageCallPayload>,
) -> Json<serde_json::Value> {
    

    let system_prompt = "You are assisting an AI voice calling service. The questions you receive are from voice conversations where users are seeking information or help. Please note: 1. Provide clear, conversational responses that can be easily read aloud 2. Avoid using any markdown, HTML, or other markup languages 3. Keep responses concise but informative 4. Use natural language sentence structure 5. When listing multiple points, use simple numbering (1, 2, 3) or natural language transitions (First... Second... Finally...) 6. Focus on the most relevant information that addresses the user's immediate needs 7. If specific numbers, dates, or proper names are important, spell them out clearly 8. Format numerical data in a way that's easy to read aloud (e.g., twenty-five percent instead of 25%) Your responses will be incorporated into a voice conversation, so clarity and natural flow are essential.";
    
    match crate::utils::tool_exec::ask_perplexity(&payload.message, system_prompt).await {
        Ok(response) => {
            Json(json!({
                "response": response
            }))
        },
        Err(e) => {
            error!("Error getting response from Perplexity: {}", e);
            Json(json!({
                "error": "Failed to get response from AI"
            }))
        }
    }
}

pub async fn handle_calendar_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    
    // Extract required parameters from query
    let user_id_str = match params.get("user_id") {
        Some(id) => id,
        None => {

            return Json(json!({
                "error": "Missing user_id parameter"
            }));
        }
    };

    let start = match params.get("start") {
        Some(start) => start,
        None => {

            return Json(json!({
                "error": "Missing start parameter"
            }));
        }
    };

    let end = match params.get("end") {
        Some(end) => end,
        None => {

            return Json(json!({
                "error": "Missing end parameter"
            }));
        }
    };

    // Parse user_id from string to i32
    let user_id = match user_id_str.parse::<i32>() {
        Ok(id) => id,
        Err(_) => {

            return Json(json!({
                "error": "Invalid user ID format"
            }));
        }
    };

    // Call the handler in google_calendar.rs
    match crate::handlers::google_calendar::handle_calendar_fetching(&state, user_id, start, end).await {
        Ok(response) => response,
        Err((_, json_response)) => json_response,
    }
}

#[derive(Debug, Deserialize)]
pub struct TaskCreatePayload {
    pub title: String,
    pub description: Option<String>,
    pub due_time: Option<String>,
}

pub async fn handle_tasks_creation_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(task_payload): axum::extract::Json<TaskCreatePayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    
    // Get user_id from query params
    let user_id_str = match params.get("user_id") {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing user_id query parameter"
                }))
            ));
        }
    };

    // Convert String to i32
    let user_id: i32 = match user_id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid user_id format, must be an integer"
                }))
            ));
        }
    };

    // Convert due_time string to DateTime<Utc> if provided
    let due_time = match task_payload.due_time {
        Some(time_str) => {
            match chrono::DateTime::parse_from_rfc3339(&time_str) {
                Ok(dt) => Some(dt.with_timezone(&chrono::Utc)),
                Err(_) => {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "error": "Invalid due_time format. Please use RFC3339 format."
                        }))
                    ));
                }
            }
        },
        None => None,
    };

    let task_request = crate::handlers::google_tasks::CreateTaskRequest {
        title: task_payload.title,
        description: task_payload.description,
        due_time,
    };

    match crate::handlers::google_tasks::create_task(&state, user_id, &task_request).await {
        Ok(response) => {
            tracing::debug!("Successfully created task for user: {}", user_id);
            Ok(response)
        },
        Err(e) => {
            error!("Failed to create task: {:?}", e);
            Err(e)
        }
    }
}

pub async fn handle_tasks_fetching_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Extract and parse user_id from query params
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid or missing user_id parameter"
                }))
            ));
        }
    };

    tracing::debug!("Received tasks fetch request for user: {}", user_id);

    match crate::handlers::google_tasks::get_tasks(&state, user_id).await {
        Ok(response) => {
            tracing::debug!("Successfully fetched tasks for user: {}", user_id);
            Ok(response)
        },
        Err(e) => {
            error!("Failed to fetch tasks: {:?}", e);
            Err(e)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EmailSearchPayload {
    pub search_term: String,
    pub search_type: Option<String>, // "sender", "subject", or "all"
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppSearchPayload {
    search_term: String,
}

#[derive(Debug, Deserialize)]
pub struct TelegramSearchPayload {
    search_term: String,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppConfirmPayload {
    chat_name: String,
    message: String,
}

#[derive(Debug, Deserialize)]
pub struct TelegramConfirmPayload {
    chat_name: String,
    message: String,
}

#[derive(Debug, Deserialize)]
pub struct CalendarEventConfirmPayload {
    summary: String,
    start_time: String,
    duration_minutes: i32,
    description: Option<String>,
    add_notification: Option<bool>,
}

pub async fn handle_email_search_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<EmailSearchPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                }))
            ));
        }
    };

    // First fetch recent emails with increased limit
    match fetch_emails_imap(&state, user_id, true, Some(50), false).await {
        Ok(emails) => {
            let search_term = payload.search_term.to_lowercase();
            let search_type = payload.search_type.as_deref().unwrap_or("all");

            // Create a structure to hold email with its match score
            #[derive(Debug)]
            struct ScoredEmail {
                email: crate::handlers::imap_handlers::ImapEmailPreview,
                score: f64,
                match_type: String,
                matched_field: String,
            }

            let mut scored_emails: Vec<ScoredEmail> = Vec::new();
            let now = chrono::Utc::now().timestamp() as f64;

            for email in emails {
                let mut best_score = 0.0;
                let mut best_match_type = String::new();
                let mut best_matched_field = String::new();

                // Calculate time-based score factor (higher for more recent emails)
                let time_factor = email.date
                    .map(|date| {
                        let age_in_days = (now - date.timestamp() as f64) / (24.0 * 60.0 * 60.0);
                        // Exponential decay: score drops by half every 7 days, but never below 0.1
                        (0.5f64.powf(age_in_days / 7.0)).max(0.1)
                    })
                    .unwrap_or(0.1); // Default factor for emails without dates

                // Helper closure for scoring
                let score_field = |field: &Option<String>, field_name: &str| -> Option<(f64, String)> {
                    field.as_ref().map(|content| {
                        let content_lower = content.to_lowercase();
                        
                        // Exact match
                        if content_lower == search_term {
                            return (1.0, "exact".to_string());
                        }
                        
                        // Substring match
                        if content_lower.contains(&search_term) {
                            return (0.8, "substring".to_string());
                        }
                        
                        // Similarity match using Jaro-Winkler
                        let similarity = strsim::jaro_winkler(&content_lower, &search_term);
                        if similarity >= 0.7 {
                            return (similarity * 0.6, "similar".to_string());
                        }
                        
                        (0.0, "none".to_string())
                    })
                };

                // Score based on search type
                match search_type {
                    "sender" => {
                        if let Some((score, match_type)) = score_field(&email.from, "sender") {
                            if score > best_score {
                                best_score = score;
                                best_match_type = match_type;
                                best_matched_field = "sender".to_string();
                            }
                        }
                    },
                    "subject" => {
                        if let Some((score, match_type)) = score_field(&email.subject, "subject") {
                            if score > best_score {
                                best_score = score;
                                best_match_type = match_type;
                                best_matched_field = "subject".to_string();
                            }
                        }
                    },
                    _ => { // "all" or any other value
                        // Check subject
                        if let Some((score, match_type)) = score_field(&email.subject, "subject") {
                            if score > best_score {
                                best_score = score;
                                best_match_type = match_type;
                                best_matched_field = "subject".to_string();
                            }
                        }
                        
                        // Check sender
                        if let Some((score, match_type)) = score_field(&email.from, "sender") {
                            if score > best_score {
                                best_score = score;
                                best_match_type = match_type;
                                best_matched_field = "sender".to_string();
                            }
                        }
                        
                        // Check body
                        if let Some((score, match_type)) = score_field(&email.body, "body") {
                            if score > best_score {
                                best_score = score;
                                best_match_type = match_type;
                                best_matched_field = "body".to_string();
                            }
                        }
                    }
                }

                // Add to scored emails if there's any match
                if best_score > 0.0 {
                    // Combine content match score with time factor
                    let final_score = best_score * time_factor;
                    scored_emails.push(ScoredEmail {
                        email,
                        score: final_score,
                        match_type: best_match_type,
                        matched_field: best_matched_field,
                    });
                }
            }

            // Sort by score (highest first)
            scored_emails.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

            if scored_emails.is_empty() {
                return Ok(Json(json!({
                    "response": format!("No emails found matching '{}'.", payload.search_term),
                    "found": false
                })));
            }

            // Get the best match
            let best_match = &scored_emails[0];
            
            // Fetch the full email content for the best match
            match fetch_single_email_imap(&state, user_id, &best_match.email.id).await {
                Ok(full_email) => {
                    // Format response text in a more natural, voice-friendly way
                    let match_quality = match best_match.match_type.as_str() {
                        "exact" => "an exact match",
                        "substring" => "a matching part",
                        "similar" => "a similar match",
                        _ => "a match"
                    };

                    let from = full_email.from.as_ref().map_or("an unknown sender", String::as_str);
                    let subject = full_email.subject.as_ref().map_or("no subject", String::as_str);
                    let body = full_email.body.as_ref()
                        .map(|b| {
                            let chars: Vec<char> = b.chars().collect();
                            if chars.len() > 200 {
                                let truncated: String = chars.into_iter().take(200).collect();
                                format!("{}...", truncated)
                            } else {
                                b.clone()
                            }
                        })
                        .unwrap_or_else(|| "no content".to_string());

                    let mut response_text = format!(
                        "I found {} for your search in the {} field. This email is from {}. The subject is: {}. Here's what it says: {}. ",
                        match_quality,
                        best_match.matched_field,
                        from,
                        subject,
                        body
                    );

                    // Add information about additional matches if any
                    if scored_emails.len() > 1 {
                        let additional_matches = scored_emails.len() - 1;
                        
                        if additional_matches == 1 {
                            response_text.push_str("I also found one more matching email. ");
                        } else {
                            response_text.push_str(&format!("I also found {} more matching emails. ", additional_matches));
                        }

                        // Add brief info about next few matches in a more conversational way
                        for (i, scored_email) in scored_emails.iter().skip(1).take(2).enumerate() {
                            let from = scored_email.email.from.as_ref().map_or("an unknown sender", String::as_str);
                            let match_desc = match scored_email.match_type.as_str() {
                                "exact" => "exactly matches",
                                "substring" => "contains",
                                "similar" => "is similar to",
                                _ => "matches"
                            };

                            if i == 0 {
                                response_text.push_str(&format!(
                                    "The next best match is from {}, where your search term {} the {}. ",
                                    from,
                                    match_desc,
                                    scored_email.matched_field
                                ));
                            } else {
                                response_text.push_str(&format!(
                                    "Another match is from {}, with the search term matching the {}. ",
                                    from,
                                    scored_email.matched_field
                                ));
                            }
                        }

                        if additional_matches > 2 {
                            response_text.push_str(&format!(
                                "There are {} more matching emails that I haven't described. ",
                                additional_matches - 2
                            ));
                        }
                    }

                    Ok(Json(json!({
                        "response": response_text,
                        "found": true,
                        "primary_match": {
                            "email": {
                                "id": full_email.id,
                                "subject": full_email.subject,
                                "from": full_email.from,
                                "from_email": full_email.from_email,
                                "date": full_email.date.map(|dt| dt.to_rfc3339()),
                                "date_formatted": full_email.date_formatted,
                                "body": full_email.body,
                                "is_read": full_email.is_read
                            },
                            "match_quality": {
                                "score": best_match.score,
                                "match_type": best_match.match_type,
                                "matched_field": best_match.matched_field
                            }
                        },
                        "additional_matches": scored_emails.iter().skip(1).take(4).map(|scored| {
                            json!({
                                "id": scored.email.id,
                                "subject": scored.email.subject,
                                "from": scored.email.from,
                                "date_formatted": scored.email.date_formatted,
                                "match_quality": {
                                    "score": scored.score,
                                    "match_type": scored.match_type,
                                    "matched_field": scored.matched_field
                                }
                            })
                        }).collect::<Vec<_>>(),
                        "total_matches": scored_emails.len()
                    })))
                },
                Err(e) => {
                    error!("Failed to fetch full email content: {:?}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to fetch full email content",
                            "details": format!("{:?}", e)
                        }))
                    ))
                }
            }
        },
        Err(e) => {
            error!("Failed to fetch emails: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch emails",
                    "details": format!("{:?}", e)
                }))
            ))
        }
    }
}

pub async fn handle_whatsapp_confirm_send(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<WhatsAppConfirmPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                }))
            ));
        }
    };

    // Get user from database
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "User not found"
                }))
            ));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user"
                }))
            ));
        }
    };

    // First, try to find the exact room using fetch_whatsapp_room_messages
    match crate::utils::whatsapp_utils::fetch_whatsapp_room_messages(
        &state,
        user_id,
        &payload.chat_name,
        Some(1), // Limit to 1 message since we only need the room name
    ).await {
        Ok((_, room_name)) => {
            // Get the actual room name without the (WA) suffix
            let clean_room_name = room_name.trim_end_matches(" (WA)").to_string();
            
            // Set the temporary variable for WhatsApp message
            if let Err(e) = state.user_core.set_temp_variable(
                user_id,
                Some("whatsapp"),
                Some(&clean_room_name),
                None,
                Some(&payload.message),
                None,
                None,
                None,
                None,
            ) {
                tracing::error!("Failed to set temporary variable: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Failed to prepare WhatsApp message",
                        "details": e.to_string()
                    }))
                ));
            }

            // Create confirmation message
            let confirmation_message = format!(
                "Send WhatsApp to '{}' with content: '{}' (yes-> send, no -> discard) (free reply)",
                clean_room_name,
                payload.message
            );

            // Get conversation for the user
            let conversation = match state.user_conversations.get_conversation(
                &user, 
                user.preferred_number.clone().unwrap_or_else(|| std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set"))
            ).await {
                Ok(conv) => conv,
                Err(e) => {
                    error!("Failed to get conversation: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to get or create conversation"
                        }))
                    ));
                }
            };

            // Send the confirmation SMS
            match crate::api::twilio_utils::send_conversation_message(
                &conversation.conversation_sid,
                &conversation.twilio_number,
                &confirmation_message,
                false, // Don't redact since we need to extract info from this message later
                None,
                &user,
            ).await {
                Ok(message_sid) => {
                    // Deduct credits for the confirmation message
                    if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user_id, "message", None) {
                        error!("Failed to deduct user credits: {}", e);
                        // Continue execution even if credit deduction fails
                    }
                    
                    Ok(Json(json!({
                        "status": "success",
                        "message": "WhatsApp confirmation message sent",
                        "room_name": clean_room_name,
                        "message_sid": message_sid
                    })))
                }
                Err(e) => {
                    error!("Failed to send confirmation SMS: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to send confirmation message",
                            "details": e.to_string()
                        }))
                    ))
                }
            }
        },
        Err(e) => {
            error!("Failed to find WhatsApp room: {}", e);
            Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Failed to find WhatsApp room",
                    "details": e.to_string()
                }))
            ))
        }
    }
}

pub async fn handle_telegram_confirm_send(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<TelegramConfirmPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                }))
            ));
        }
    };

    // Get user from database
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "User not found"
                }))
            ));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user"
                }))
            ));
        }
    };

    // First, try to find the exact room using fetch_telegram_room_messages
    match crate::utils::telegram_utils::fetch_telegram_room_messages(
        &state,
        user_id,
        &payload.chat_name,
        Some(1), // Limit to 1 message since we only need the room name
    ).await {
        Ok((_, room_name)) => {
            // Get the actual room name without the (WA) suffix
            let clean_room_name = room_name.trim_end_matches(" (WA)").to_string();
            
            // Set the temporary variable for Telegram message
            if let Err(e) = state.user_core.set_temp_variable(
                user_id,
                Some("telegram"),
                Some(&clean_room_name),
                None,
                Some(&payload.message),
                None,
                None,
                None,
                None,
            ) {
                tracing::error!("Failed to set temporary variable: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Failed to prepare Telegram message",
                        "details": e.to_string()
                    }))
                ));
            }

            // Create confirmation message
            let confirmation_message = format!(
                "Send Telegram to '{}' with content: '{}' (yes-> send, no -> discard) (free reply)",
                clean_room_name,
                payload.message
            );

            // Get conversation for the user
            let conversation = match state.user_conversations.get_conversation(
                &user, 
                user.preferred_number.clone().unwrap_or_else(|| std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set"))
            ).await {
                Ok(conv) => conv,
                Err(e) => {
                    error!("Failed to get conversation: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to get or create conversation"
                        }))
                    ));
                }
            };

            // Send the confirmation SMS
            match crate::api::twilio_utils::send_conversation_message(
                &conversation.conversation_sid,
                &conversation.twilio_number,
                &confirmation_message,
                false, // Don't redact since we need to extract info from this message later
                None,
                &user,
            ).await {
                Ok(message_sid) => {
                    // Deduct credits for the confirmation message
                    if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user_id, "message", None) {
                        error!("Failed to deduct user credits: {}", e);
                        // Continue execution even if credit deduction fails
                    }
                    
                    Ok(Json(json!({
                        "status": "success",
                        "message": "Telegram confirmation message sent",
                        "room_name": clean_room_name,
                        "message_sid": message_sid
                    })))
                }
                Err(e) => {
                    error!("Failed to send confirmation SMS: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to send confirmation message",
                            "details": e.to_string()
                        }))
                    ))
                }
            }
        },
        Err(e) => {
            error!("Failed to find Telegram room: {}", e);
            Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "Failed to find Telegram room",
                    "details": e.to_string()
                }))
            ))
        }
    }
}

pub async fn handle_calendar_event_confirm(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<CalendarEventConfirmPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                }))
            ));
        }
    };

    // Get user from database
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "User not found"
                }))
            ));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user"
                }))
            ));
        }
    };


    // Get conversation for the user
    let conversation = match state.user_conversations.get_conversation(
        &user, 
        user.preferred_number.clone().unwrap_or_else(|| std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set"))
    ).await {
        Ok(conv) => conv,
        Err(e) => {
            error!("Failed to get conversation: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to get or create conversation"
                }))
            ));
        }
    };

    // Format the confirmation message
    let confirmation_message = if let Some(ref desc) = payload.description {
        format!(
            "Create calendar event: '{}' starting at '{}' for {} minutes with description: '{}' (yes-> send, no -> discard) (free reply)",
            payload.summary, payload.start_time, payload.duration_minutes, desc
        )
    } else {
        format!(
            "Create calendar event: '{}' starting at '{}' for {} minutes (yes-> send, no -> discard) (free reply)",
            payload.summary, payload.start_time, payload.duration_minutes
        )
    };

    // Set the temporary variable for calendar event
    if let Err(e) = state.user_core.set_temp_variable(
        user_id,
        Some("calendar"),
        None,
        Some(&payload.summary),
        Some(&payload.description.unwrap_or_default()),
        Some(&payload.start_time),
        Some(&payload.duration_minutes.to_string()),
        None,
        None,
    ) {
        error!("Failed to set temporary variable: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Failed to prepare calendar event",
                "details": e.to_string()
            }))
        ));
    }

    // Send the confirmation SMS
    match crate::api::twilio_utils::send_conversation_message(
        &conversation.conversation_sid,
        &conversation.twilio_number,
        &confirmation_message,
        false, // Don't redact since we need to extract info from this message later
        None,
        &user,
    ).await {
        Ok(message_sid) => {
            // Deduct credits for the confirmation message
            if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user_id, "message", None) {
                error!("Failed to deduct user credits: {}", e);
                // Continue execution even if credit deduction fails
            }
            
            Ok(Json(json!({
                "status": "success",
                "message": "Calendar event confirmation sent",
                "event_summary": payload.summary,
                "message_sid": message_sid
            })))
        }
        Err(e) => {
            error!("Failed to send confirmation SMS: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to send confirmation message",
                    "details": e.to_string()
                }))
            ))
        }
    }
}

pub async fn handle_whatsapp_search_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<WhatsAppSearchPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                }))
            ));
        }
    };

    // Search for rooms using the existing utility function
    match crate::utils::whatsapp_utils::search_whatsapp_rooms(&state, user_id, &payload.search_term).await {
        Ok(rooms) => {
            if rooms.is_empty() {
                return Ok(Json(json!({
                    "response": format!("No WhatsApp contacts found matching '{}'.", payload.search_term),
                    "rooms": []
                })));
            }

            // Format rooms for voice response
            let mut response_text = format!(
                "Found {} matching WhatsApp contacts. ",
                rooms.len()
            );

            // Add up to 5 most relevant rooms to the voice response
            for (i, room) in rooms.iter().take(5).enumerate() {
                response_text.push_str(&format!(
                    "Contact {} is {}, last active {}. ",
                    i + 1,
                    room.display_name.trim_end_matches(" (WA)"),
                    room.last_activity_formatted
                ));
            }

            if rooms.len() > 5 {
                response_text.push_str(&format!(
                    "And {} more contacts found. ",
                    rooms.len() - 5
                ));
            }

            Ok(Json(json!({
                "response": response_text,
                "rooms": rooms,
                "total_count": rooms.len()
            })))
        },
        Err(e) => {
            error!("Failed to search WhatsApp rooms: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to search WhatsApp contacts",
                    "details": e.to_string()
                }))
            ))
        }
    }
}

pub async fn handle_telegram_search_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<TelegramSearchPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                }))
            ));
        }
    };

    // Search for rooms using the existing utility function
    match crate::utils::telegram_utils::search_telegram_rooms(&state, user_id, &payload.search_term).await {
        Ok(rooms) => {
            if rooms.is_empty() {
                return Ok(Json(json!({
                    "response": format!("No Telegram contacts found matching '{}'.", payload.search_term),
                    "rooms": []
                })));
            }

            // Format rooms for voice response
            let mut response_text = format!(
                "Found {} matching Telegram contacts. ",
                rooms.len()
            );

            // Add up to 5 most relevant rooms to the voice response
            for (i, room) in rooms.iter().take(5).enumerate() {
                response_text.push_str(&format!(
                    "Contact {} is {}, last active {}. ",
                    i + 1,
                    room.display_name.trim_end_matches(" (WA)"),
                    room.last_activity_formatted
                ));
            }

            if rooms.len() > 5 {
                response_text.push_str(&format!(
                    "And {} more contacts found. ",
                    rooms.len() - 5
                ));
            }

            Ok(Json(json!({
                "response": response_text,
                "rooms": rooms,
                "total_count": rooms.len()
            })))
        },
        Err(e) => {
            error!("Failed to search Telegram rooms: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to search Telegram contacts",
                    "details": e.to_string()
                }))
            ))
        }
    }
}

pub async fn handle_whatsapp_fetch_specific_room_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Starting WhatsApp specific room message fetch");

    // Extract user_id and chat_room from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                }))
            ));
        }
    };

    let chat_room = match params.get("chat_room") {
        Some(room) => room,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing chat_room parameter"
                }))
            ));
        }
    };

    // Fetch messages using the existing utility function
    match crate::utils::whatsapp_utils::fetch_whatsapp_room_messages(&state, user_id, chat_room, Some(20)).await {
        Ok((messages, room_name)) => {
            if messages.is_empty() {
                return Ok(Json(json!({
                    "response": format!("No WhatsApp messages found in chat room '{}'.", chat_room),
                    "messages": []
                })));
            }

            // Format messages for voice response
            let mut response_text = format!(
                "Here are the recent messages from {}: ",
                room_name.trim_end_matches(" (WA)")
            );

            // Add messages to the voice response
            for (i, msg) in messages.iter().take(20).enumerate() {
                response_text.push_str(&format!(
                    "Message {} from {}, sent on {}: {}. ",
                    i + 1,
                    msg.sender_display_name,
                    msg.formatted_timestamp,
                    msg.content
                ));
            }

            Ok(Json(json!({
                "response": response_text,
                "messages": messages,
                "room_name": room_name,
                "total_count": messages.len()
            })))
        },
        Err(e) => {
            error!("Failed to fetch WhatsApp messages: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch WhatsApp messages",
                    "details": e.to_string()
                }))
            ))
        }
    }
}

pub async fn handle_telegram_fetch_specific_room_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Starting Telegram specific room message fetch");

    // Extract user_id and chat_room from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                }))
            ));
        }
    };

    let chat_room = match params.get("chat_room") {
        Some(room) => room,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing chat_room parameter"
                }))
            ));
        }
    };

    // Fetch messages using the existing utility function
    match crate::utils::telegram_utils::fetch_telegram_room_messages(&state, user_id, chat_room, Some(20)).await {
        Ok((messages, room_name)) => {
            if messages.is_empty() {
                return Ok(Json(json!({
                    "response": format!("No Telegram messages found in chat room '{}'.", chat_room),
                    "messages": []
                })));
            }

            // Format messages for voice response
            let mut response_text = format!(
                "Here are the recent messages from {}: ",
                room_name.trim_end_matches(" (WA)")
            );

            // Add messages to the voice response
            for (i, msg) in messages.iter().take(20).enumerate() {
                response_text.push_str(&format!(
                    "Message {} from {}, sent on {}: {}. ",
                    i + 1,
                    msg.sender_display_name,
                    msg.formatted_timestamp,
                    msg.content
                ));
            }

            Ok(Json(json!({
                "response": response_text,
                "messages": messages,
                "room_name": room_name,
                "total_count": messages.len()
            })))
        },
        Err(e) => {
            error!("Failed to fetch Telegram messages: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch Telegram messages",
                    "details": e.to_string()
                }))
            ))
        }
    }
}

pub async fn handle_whatsapp_fetch_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,

) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Starting WhatsApp message fetch with time range");

    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                }))
            ));
        }
    };

    // Extract and parse start_time from query parameters
    let start_timestamp = match params.get("start_time").and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()) {
        Some(dt) => dt.timestamp(),
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid start_time. Expected RFC3339 format (e.g., '2024-03-16T00:00:00Z')"
                }))
            ));
        }
    };

    // Extract and parse end_time from query parameters
    let end_timestamp = match params.get("end_time").and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()) {
        Some(dt) => dt.timestamp(),
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid end_time. Expected RFC3339 format (e.g., '2024-03-16T00:00:00Z')"
                }))
            ));
        }
    };

    // Fetch messages using the existing utility function
    match crate::utils::whatsapp_utils::fetch_whatsapp_messages(&state, user_id, start_timestamp, end_timestamp).await {
        Ok(messages) => {
            if messages.is_empty() {
                return Ok(Json(json!({
                    "response": "No WhatsApp messages found for the specified time range.",
                    "messages": []
                })));
            }

            // Format messages for voice response
            let mut response_text = format!(
                "Found {} WhatsApp messages. Here are the highlights: ",
                messages.len()
            );

            // Add up to 5 most recent messages to the voice response
            for (i, msg) in messages.iter().take(20).enumerate() {

                response_text.push_str(&format!(
                    "Message {} from {} in chat {}, sent on {}: {}. ",
                    i + 1,
                    msg.sender_display_name,
                    msg.room_name,
                    msg.formatted_timestamp,
                    msg.content
                ));
            }

            if messages.len() > 20 {
                response_text.push_str(&format!(
                    "And {} more messages. ",
                    messages.len() - 20 
                ));
            }

            Ok(Json(json!({
                "response": response_text,
                "messages": messages,
                "total_count": messages.len()
            })))
        },
        Err(e) => {
            error!("Failed to fetch WhatsApp messages: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch WhatsApp messages",
                    "details": e.to_string()
                }))
            ))
        }
    }
}

pub async fn handle_telegram_fetch_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,

) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Starting Telegram message fetch with time range");

    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                }))
            ));
        }
    };

    // Extract and parse start_time from query parameters
    let start_timestamp = match params.get("start_time").and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()) {
        Some(dt) => dt.timestamp(),
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid start_time. Expected RFC3339 format (e.g., '2024-03-16T00:00:00Z')"
                }))
            ));
        }
    };

    // Extract and parse end_time from query parameters
    let end_timestamp = match params.get("end_time").and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()) {
        Some(dt) => dt.timestamp(),
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid end_time. Expected RFC3339 format (e.g., '2024-03-16T00:00:00Z')"
                }))
            ));
        }
    };

    // Fetch messages using the existing utility function
    match crate::utils::telegram_utils::fetch_telegram_messages(&state, user_id, start_timestamp, end_timestamp).await {
        Ok(messages) => {
            if messages.is_empty() {
                return Ok(Json(json!({
                    "response": "No Telegram messages found for the specified time range.",
                    "messages": []
                })));
            }

            // Format messages for voice response
            let mut response_text = format!(
                "Found {} Telegram messages. Here are the highlights: ",
                messages.len()
            );

            // Add up to 5 most recent messages to the voice response
            for (i, msg) in messages.iter().take(20).enumerate() {

                response_text.push_str(&format!(
                    "Message {} from {} in chat {}, sent on {}: {}. ",
                    i + 1,
                    msg.sender_display_name,
                    msg.room_name,
                    msg.formatted_timestamp,
                    msg.content
                ));
            }

            if messages.len() > 20 {
                response_text.push_str(&format!(
                    "And {} more messages. ",
                    messages.len() - 20 
                ));
            }

            Ok(Json(json!({
                "response": response_text,
                "messages": messages,
                "total_count": messages.len()
            })))
        },
        Err(e) => {
            error!("Failed to fetch Telegram messages: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch Telegram messages",
                    "details": e.to_string()
                }))
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EmailResponsePayload {
    pub email_id: String,
    pub response_text: String,
}
// this is not used at the moment since it didn't work and not a priority rn
pub async fn handle_email_response_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<EmailResponsePayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Starting email response confirmation for email ID: {}", payload.email_id);

    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                }))
            ));
        }
    };

    // Get user from database
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "User not found"
                }))
            ));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user"
                }))
            ));
        }
    };

    // Verify user has sufficient credits for email response
    if let Err(msg) = crate::utils::usage::check_user_credits(&state, &user, "email").await {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "Insufficient credits",
                "message": msg
            }))
        ));
    }

    // Fetch the email details to include in confirmation message
    match crate::handlers::imap_handlers::fetch_single_email_imap(&state, user_id, &payload.email_id).await {
        Ok(email) => {
            let subject = email.subject.unwrap_or_else(|| "No subject".to_string());

            // Set the temporary variable for email response
            if let Err(e) = state.user_core.set_temp_variable(
                user_id,
                Some("email"),
                None,
                Some(&subject),
                Some(&payload.response_text),
                None,
                None,
                Some(&payload.email_id),
                None,
            ) {
                tracing::error!("Failed to set temporary variable: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Failed to prepare email response",
                        "details": e.to_string()
                    }))
                ));
            }

            // Create confirmation message
            let confirmation_message = format!(
                "Send email response '{}' regarding '{}' (yes-> send, no -> discard) (free reply)",
                payload.response_text,
                subject
            );

            // Get conversation for the user
            let conversation = match state.user_conversations.get_conversation(
                &user, 
                user.preferred_number.clone().unwrap_or_else(|| std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set"))
            ).await {
                Ok(conv) => conv,
                Err(e) => {
                    error!("Failed to get conversation: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to get or create conversation"
                        }))
                    ));
                }
            };

            // Send the confirmation SMS
            match crate::api::twilio_utils::send_conversation_message(
                &conversation.conversation_sid,
                &conversation.twilio_number,
                &confirmation_message,
                false, // Don't redact since we need to extract info from this message later
                None,
                &user,
            ).await {
                Ok(message_sid) => {
                    // Deduct credits for the confirmation message
                    if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user_id, "message", None) {
                        tracing::error!("Failed to deduct user credits: {}", e);
                        // Continue execution even if credit deduction fails
                    }
                    
                    Ok(Json(json!({
                        "status": "success",
                        "message": "Email response confirmation sent",
                        "email_id": payload.email_id,
                        "message_sid": message_sid
                    })))
                }
                Err(e) => {
                    error!("Failed to send confirmation SMS: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to send confirmation message",
                            "details": e.to_string()
                        }))
                    ))
                }
            }
        },
        Err(e) => {
            error!("Failed to fetch email details: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch email details",
                    "details": format!("{:?}", e)
                }))
            ))
        }
    }
}

pub async fn make_notification_call(
    state: &Arc<AppState>,
    to_phone_number: String,
    preferred_from_number: String,
    content_type: String,
    notification_first_message: String,
    notification_message: String,
    user_id: String,
    user_timezone: Option<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

// Get user information to check for discount tier
    let user = match state.user_core.find_by_id(user_id.parse::<i32>().unwrap_or_default()) {
        Ok(Some(user)) => user,
        Ok(None) => {
            error!("User not found for ID: {}", user_id);
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "User not found",
                    "details": "Could not find user with provided ID"
                }))
            ));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Database error",
                    "details": e.to_string()
                }))
            ));
        }
    };

    // Check if user has a custom phone number ID
    let phone_number_id = if user.discount_tier.is_some() {
        let custom_env_key = format!("TWILIO_USER_PHONE_NUMBER_ID_{}", user.id);
        match std::env::var(&custom_env_key) {
            Ok(id) => {
                tracing::debug!("Using custom phone number ID for user {}: {}", user.id, id);
                id
            },
            Err(_) => {
                // Fall back to regular phone number selection if custom ID not found
                if preferred_from_number.contains("+358") {
                    std::env::var("FIN_PHONE_NUMBER_ID")
                } else if preferred_from_number.contains("+61") {
                    std::env::var("AUS_PHONE_NUMBER_ID")
                } else if preferred_from_number.contains("+44") {
                    std::env::var("GB_PHONE_NUMBER_ID")
                } else {
                    std::env::var("USA_PHONE_NUMBER_ID")
                }.map_err(|e| {
                    error!("Failed to get phone number ID: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to get phone number ID",
                            "details": e.to_string()
                        }))
                    )
                })?
            }
        }
    } else {
        // Regular phone number selection for non-discount users
        if preferred_from_number.contains("+358") {
            std::env::var("FIN_PHONE_NUMBER_ID")
        } else if preferred_from_number.contains("+61") {
            std::env::var("AUS_PHONE_NUMBER_ID")
        } else if preferred_from_number.contains("+44") {
            std::env::var("GB_PHONE_NUMBER_ID")
        } else {
            std::env::var("USA_PHONE_NUMBER_ID")
        }.map_err(|e| {
            error!("Failed to get phone number ID: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to get phone number ID",
                    "details": e.to_string()
                }))
            )
        })?
    };


    // Get voice ID based on the preferred number
    let voice_id = if preferred_from_number.contains("+358") {
        std::env::var("FI_VOICE_ID").expect("FI_VOICE_ID not set")
    } else if preferred_from_number.contains("+49") {
        std::env::var("DE_VOICE_ID").expect("DE_VOICE_ID not set")
    } else {
        std::env::var("US_VOICE_ID").expect("US_VOICE_ID not set")
    };

    // Create dynamic variables map with notification message
    let mut dynamic_variables = HashMap::new();
    dynamic_variables.insert("notification_message".to_string(), json!(notification_message));
    dynamic_variables.insert("now".to_string(), json!(format!("{}", chrono::Utc::now())));
    

    // set the ids to -1 to prevent agent from making mistake
    dynamic_variables.insert("content_type".to_string(), json!(content_type));
    dynamic_variables.insert("user_id".to_string(), json!(user_id));
    // Get timezone from user info or default to UTC
    let timezone_str = match user_timezone {
        Some(ref tz) => tz.as_str(),
        None => "UTC",
    };

    // Get timezone offset using jiff
    let (hours, minutes) = match get_offset_with_jiff(timezone_str) {
        Ok((h, m)) => (h, m),
        Err(_) => {
            tracing::error!("Failed to get timezone offset for {}, defaulting to UTC", timezone_str);
            (0, 0) // UTC default
        }
    };

    // Format offset string (e.g., "+02:00" or "-05:30")
    let offset = format!("{}{:02}:{:02}", 
        if hours >= 0 { "+" } else { "-" },
        hours.abs(),
        minutes.abs()
    );

    dynamic_variables.insert("timezone".to_string(), json!(timezone_str));
    dynamic_variables.insert("timezone_offset_from_utc".to_string(), json!(offset));



    // Create the payload for the call
    let payload = NotificationCallPayload {
        agent_id: std::env::var("AGENT_ID").expect("AGENT_ID not set"),
        agent_phone_number_id: phone_number_id.clone(),
        to_number: to_phone_number.clone(),
        conversation_initiation_client_data: ConversationInitiationClientData {
            r#type: "conversation_initiation_client_data".to_string(),
            conversation_config_override: ConversationConfig {
                agent: AgentConfig {
                    first_message: notification_first_message,
                },
                tts: VoiceId {
                    voice_id,
                },
            },
            dynamic_variables,
        },
    };

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.elevenlabs.io/v1/convai/twilio/outbound-call".to_string())
        .header("xi-api-key", std::env::var("ELEVENLABS_API_KEY").expect("ELEVENLABS_API_KEY not set"))
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to make ElevenLabs API call: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to initiate notification call",
                    "details": e.to_string()
                }))
            )
        })?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        error!("ElevenLabs API returned error: {}", error_text);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "ElevenLabs API returned error",
                "details": error_text
            }))
        ));
    }

    // Parse the response to get the call SID
    let response_json = response.json::<serde_json::Value>().await.map_err(|e| {
        error!("Failed to parse ElevenLabs API response: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Failed to parse API response",
                "details": e.to_string()
            }))
        )
    })?;

    // Get user information before spawning the thread
    let user = match state.user_core.find_by_id(user_id.parse::<i32>().unwrap_or_default()) {
        Ok(Some(user)) => user,
        Ok(None) => {
            error!("User not found for ID: {}", user_id);
            return Ok(Json(json!({
                "status": "success",
                "message": "Notification call initiated successfully, but user not found for status updates",
                "to_number": to_phone_number,
                "from_number_id": phone_number_id,
            })));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Ok(Json(json!({
                "status": "success",
                "message": "Notification call initiated successfully, but failed to fetch user for status updates",
                "to_number": to_phone_number,
                "from_number_id": phone_number_id,
            })));
        }
    };


    // Clone necessary data before spawning the task
    let user_preferred_number = user.preferred_number.clone()
        .unwrap_or_else(|| std::env::var("SHAZAM_PHONE_NUMBER")
            .expect("SHAZAM_PHONE_NUMBER not set"));

    // Get conversation before spawning the task
    let conversation = match state.user_conversations.get_conversation(&user, user_preferred_number.clone()).await {
        Ok(conv) => conv,
        Err(e) => {
            error!("Failed to get conversation before spawning status check task: {}", e);
            return Ok(Json(json!({
                "status": "success",
                "message": "Notification call initiated successfully, but failed to set up status updates",
                "to_number": to_phone_number,
                "from_number_id": phone_number_id,
            })));
        }
    };

    let conversation_sid = conversation.conversation_sid.clone();
    let twilio_number = conversation.twilio_number.clone();

    // Send an immediate SMS notification
    // Truncate and clean notification message to ensure SMS compatibility
    let cleaned_message = notification_message
        .chars()
        .filter(|c| c.is_ascii())
        .collect::<String>();
    let truncated_message = if cleaned_message.len() > 50 {
        format!("{}...", &cleaned_message[..47])
    } else {
        cleaned_message
    };
    let notification_sms = format!(
        "Notification: {}",
        truncated_message
    );

    // Send the SMS notification
    if let Err(e) = crate::api::twilio_utils::send_conversation_message(
        &conversation_sid,
        &twilio_number,
        &notification_sms,
        true,
        None,
        &user,
    ).await {
        error!("Failed to send notification SMS: {}", e);
        // Continue with the call even if SMS fails
    }

    Ok(Json(json!({
        "status": "success",
        "message": "Notification call initiated successfully",
        "to_number": to_phone_number,
        "from_number_id": phone_number_id,
    })))
}

pub async fn handle_weather_tool_call(
    State(_state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<LocationCallPayload>,
) -> Json<serde_json::Value> {
    
    match crate::utils::tool_exec::get_weather(&payload.location, &payload.units).await {
        Ok(weather_info) => {
            Json(json!({
                "response": weather_info
            }))
        },
        Err(e) => {
            error!("Error getting weather information: {}", e);
            Json(json!({
                "error": "Failed to get weather information",
                "details": e.to_string()
            }))
        }
    }
}


