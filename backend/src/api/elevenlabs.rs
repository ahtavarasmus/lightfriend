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
use crate::handlers::imap_handlers::fetch_emails_imap;



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
pub struct MessageCallPayload {
    message: String,
}

#[derive(Debug, Deserialize)]
pub struct AssistantPayload {
    agent_id: String,
    call_sid: String,
    called_number: String,
    caller_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct ConversationInitiationClientData {
    r#type: String,
    conversation_config_override: ConversationConfig,
    dynamic_variables: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize)]
pub struct ConversationConfig {
    agent: AgentConfig,
}

#[derive(Serialize, Deserialize)]
pub struct AgentConfig {
    first_message: String,
}


pub async fn validate_elevenlabs_secret(
    headers: HeaderMap,
    request: Request<Body>,
    next: middleware::Next,
) -> Result<Response, StatusCode> {
    println!("\n=== Starting Elevenlabs Secret Validation ===");
    
    let secret_key = match std::env::var("ELEVENLABS_SERVER_URL_SECRET") {
        Ok(key) => {
            println!("✅ Successfully retrieved ELEVENLABS_SERVER_URL_SECRET");
            key
        },
        Err(e) => {
            println!("❌ Failed to get ELEVENLABS_SERVER_URL_SECRET: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    match headers.get("x-elevenlabs-secret") {
        Some(header_value) => {
            println!("🔍 Found x-elevenlabs-secret header");
            match header_value.to_str() {
                Ok(value) => {
                    if value == secret_key {
                        println!("✅ Secret validation successful");
                        Ok(next.run(request).await)
                    } else {
                        println!("❌ Invalid secret provided");
                        Err(StatusCode::UNAUTHORIZED)
                    }
                },
                Err(e) => {
                    println!("❌ Error converting header to string: {}", e);
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        },
        None => {
            println!("❌ No x-elevenlabs-secret header found");
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
    println!("Received assistant request:");
    let agent_id = payload.agent_id;
    println!("Agent ID: {}", agent_id);
    let call_sid = payload.call_sid;
    println!("Call SID: {}", call_sid);
    let called_number = payload.called_number;
    println!("Called Number: {}", called_number);
    let caller_number = payload.caller_id;
    println!("Caller Number: {}", caller_number);

    let mut dynamic_variables = HashMap::new();
    let mut conversation_config_override = ConversationConfig {
        agent: AgentConfig {
            first_message: "Hello {{name}}!".to_string(),
        },
    };


    match state.user_repository.find_by_phone_number(&caller_number) {
        Ok(Some(user)) => {
            println!("Found user: {}, {}", user.email, user.phone_number);
            
            if user.credits <= 0.00 {

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

                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": "Insufficient credits balance",
                        "message": "Please add more credits to your account to continue on lightfriend website"
                    }))
                ));
            }

            // If user is not verified, verify them
            if !user.verified {
                if let Err(e) = state.user_repository.verify_user(user.id) {
                    println!("Error verifying user: {}", e);
                    // Continue even if verification fails
                } else {
                    conversation_config_override = ConversationConfig {
                        agent: AgentConfig {
                            first_message: "Welcome! Your number is now verified. You can add some information about yourself in the profile page. Anyways, how can I help?".to_string(),
                        },
                    };
                }
            }

            let nickname = match user.nickname {
                Some(nickname) => nickname,
                None => "".to_string()
            };
            let user_info = match user.info {
                Some(info) => info,
                None => "".to_string()
            };

            dynamic_variables.insert("name".to_string(), json!(nickname));
            dynamic_variables.insert("user_info".to_string(), json!(user_info));
            dynamic_variables.insert("user_id".to_string(), json!(user.id));

            // Get timezone from user info or default to UTC
            let timezone_str = match user.timezone {
                Some(ref tz) => tz.as_str(),
                None => "UTC",
            };

            // Get timezone offset using jiff
            let (hours, minutes) = match get_offset_with_jiff(timezone_str) {
                Ok((h, m)) => (h, m),
                Err(_) => {
                    println!("Failed to get timezone offset for {}, defaulting to UTC", timezone_str);
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
            println!("SEconds_to_threshold: {}", seconds_to_threshold);
            // following just so it doesn't go negative although i don't think it matters
            let recharge_threshold_timestamp: i32 = (chrono::Utc::now().timestamp() as i32) + seconds_to_threshold;

            let seconds_to_zero_credits= (user.credits / voice_second_cost) as i32;
            println!("Seconds to zero credits: {}", seconds_to_zero_credits);
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
                eprintln!("Failed to log call usage: {}", e);
                // Continue execution even if logging fails
            }

        },
        Ok(None) => {
            println!("No user found for number: {}", caller_number);
            dynamic_variables.insert("name".to_string(), json!(""));
            dynamic_variables.insert("user_info".to_string(), json!("new user"));
        },
        Err(e) => {
            println!("Error looking up user: {}", e);
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
    println!("Received waiting check creation request for user: {}", payload.user_id);

    // Handle due_date: parse provided string or use default (2 weeks from now)
    let due_date_utc = match payload.due_date {
        Some(date_str) => {
            // Validate the provided date string
            match chrono::DateTime::parse_from_rfc3339(&date_str) {
                Ok(_) => date_str, // If valid RFC3339, use as-is
                Err(_) => {
                    println!("Invalid due_date format provided, using default");
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
    match state.user_repository.find_by_id(payload.user_id) {
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
                    println!("Successfully created waiting check for user: {} with due date: {}", 
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
            println!("User not found: {}", payload.user_id);
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
) -> Json<serde_json::Value> {

    // Extract and parse user_id from query params
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Json(json!({
                "error": "Invalid or missing user_id parameter"
            }));
        }
    };
    println!("Received Gmail fetch request for user: {}", user_id);
    
    match crate::handlers::imap_handlers::fetch_emails_imap(&state, user_id, true, Some(10), false).await {
        Ok(emails) => {
            // Format emails for voice response
            let mut response_text = String::from("Here are your recent emails. ");
            
            for (i, email) in emails.iter().enumerate() {
                let subject = email.subject.as_deref().unwrap_or("No subject");
                let from = email.from.as_deref().unwrap_or("Unknown sender");
                let status = if email.is_read { "read" } else { "unread" };
                let body = email.body.as_deref().unwrap_or("No content available");
                response_text.push_str(&format!(
                    "Email {}. From {}. Subject: {}. Status: {}. Content: {}. ",
                    i + 1,
                    from,
                    subject,
                    status,
                    body
                ));
            }

            if emails.is_empty() {
                response_text = "You have no recent emails.".to_string();
            }

            Json(json!({
                "response": response_text
            }))
        },
        Err(e) => {
            error!("Error fetching Gmail messages: {:?}", e);
            Json(json!({
                "error": "Failed to fetch Gmail messages",
                "details": format!("{:?}", e)
            }))
        }
    }
}




pub async fn handle_send_sms_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<MessageCallPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Received SMS send request with message: {}", payload.message);
    
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
    let user = match state.user_repository.find_by_id(user_id) {
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

    // Send the message using Twilio
    match crate::api::twilio_utils::send_conversation_message(
        &conversation.conversation_sid,
        &conversation.twilio_number,
        &payload.message
    ).await {
        Ok(message_sid) => {
            println!("Successfully sent SMS with SID: {}", message_sid);
            Ok(Json(json!({
                "status": "success",
                "message_sid": message_sid,
                "conversation_sid": conversation.conversation_sid
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
    Json(payload): Json<MessageCallPayload>,
) -> Json<serde_json::Value> {
    println!("Received message: {}", payload.message);
    

    let system_prompt = "You are assisting an AI voice calling service. The questions you receive are from voice conversations where users are seeking information or help. Please note: 1. Provide clear, conversational responses that can be easily read aloud 2. Avoid using any markdown, HTML, or other markup languages 3. Keep responses concise but informative 4. Use natural language sentence structure 5. When listing multiple points, use simple numbering (1, 2, 3) or natural language transitions (First... Second... Finally...) 6. Focus on the most relevant information that addresses the user's immediate needs 7. If specific numbers, dates, or proper names are important, spell them out clearly 8. Format numerical data in a way that's easy to read aloud (e.g., twenty-five percent instead of 25%) Your responses will be incorporated into a voice conversation, so clarity and natural flow are essential.";
    
    match crate::utils::tool_exec::ask_perplexity(&payload.message, system_prompt).await {
        Ok(response) => {
            println!("Perplexity response: {}", response);
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
    println!("Starting calendar tool call with params: {:?}", params);
    
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

pub async fn handle_weather_tool_call(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<LocationCallPayload>,
) -> Json<serde_json::Value> {
    println!("Received weather request for location: {}, using: {}", payload.location, payload.units);
    
    match crate::utils::tool_exec::get_weather(&payload.location, &payload.units).await {
        Ok(weather_info) => {
            println!("Weather response: {}", weather_info);
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


