use crate::handlers::imap_handlers::{fetch_emails_imap, fetch_single_email_imap};
use crate::repositories::user_repository::LogUsageParams;
use crate::AppState;
use axum::middleware;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    response::Response,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::error;

#[derive(Debug, Deserialize)]
pub struct LocationCallPayload {
    location: String,
    units: String,
    forecast_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageCallPayload {
    message: String,
    email_id: Option<String>,
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
    call_sid: String,
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
        }
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
                }
                Err(e) => {
                    tracing::error!("❌ Error converting header to string: {}", e);
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        }
        None => {
            tracing::error!("❌ No x-elevenlabs-secret header found");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

use jiff::Timestamp;

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
    let call_sid = payload.call_sid;
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

            // Check if phone service is deactivated (e.g., stolen phone scenario)
            if let Ok(false) = state.user_core.get_phone_service_active(user.id) {
                tracing::warn!(
                    "User {} has phone service deactivated - blocking incoming call",
                    user.id
                );
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": "Service deactivated",
                        "message": "Phone service is currently deactivated for this number."
                    })),
                ));
            }

            let user_settings = match state.user_core.get_user_settings(user.id) {
                Ok(settings) => settings,
                Err(e) => {
                    error!("Failed to get user settings: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to get conversation",
                            "message": "Internal server error"
                        })),
                    ));
                }
            };
            let user_info = match state.user_core.get_user_info(user.id) {
                Ok(settings) => settings,
                Err(e) => {
                    error!("Failed to get user settings: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to get conversation",
                            "message": "Internal server error"
                        })),
                    ));
                }
            };
            // If user is not verified, verify them
            if !user.verified {
                if let Err(e) = state.user_core.verify_user(user.id) {
                    tracing::error!("Error verifying user: {}", e);
                    // Continue even if verification fails
                } else if user_settings.agent_language == "fi" {
                    conversation_config_override.agent.first_message =
                        "Tervetuloa! Numerosi on nyt vahvistettu. Miten voin auttaa?".to_string();
                    conversation_config_override.tts.voice_id = fi_voice_id.clone();
                } else if user_settings.agent_language == "de" {
                    conversation_config_override.agent.first_message =
                        "Willkommen! Ihre Nummer ist jetzt verifiziert. Wie kann ich Ihnen helfen?"
                            .to_string();
                    conversation_config_override.tts.voice_id = de_voice_id.clone();
                } else {
                    conversation_config_override.agent.first_message =
                        "Welcome! Your number is now verified. Anyways, how can I help?"
                            .to_string();
                    conversation_config_override.tts.voice_id = us_voice_id.clone();
                }
            } else if crate::utils::usage::check_user_credits(&state, &user, "voice", None)
                .await
                .is_err()
            {
                // Send insufficient credits message
                let error_message = "Insufficient credits to make a voice call".to_string();
                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                    &state,
                    &error_message,
                    None,
                    &user,
                )
                .await
                {
                    error!("Failed to send insufficient credits message: {}", e);
                }
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": "Insufficient credits balance",
                        "message": "Please add more credits to your account to continue on lightfriend website",
                    })),
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
                None => "".to_string(),
            };
            let user_own_info = match user_info.info.clone() {
                Some(info) => info,
                None => "".to_string(),
            };
            let user_location = user_info.location.clone().unwrap_or("".to_string());
            let nearby_places_str = user_info.nearby_places.clone().unwrap_or("".to_string());
            dynamic_variables.insert("name".to_string(), json!(nickname));
            dynamic_variables.insert("user_info".to_string(), json!(user_own_info));
            dynamic_variables.insert("nearby_places".to_string(), json!(nearby_places_str));
            dynamic_variables.insert("location".to_string(), json!(user_location));
            dynamic_variables.insert("user_id".to_string(), json!(user.id));
            dynamic_variables.insert("email_id".to_string(), json!("-1".to_string()));
            dynamic_variables.insert("content_type".to_string(), json!("".to_string()));
            dynamic_variables.insert("notification_message".to_string(), json!("".to_string()));
            // Get timezone from user info or default to UTC
            let timezone_str = match user_info.timezone {
                Some(ref tz) => tz.as_str(),
                None => "UTC",
            };
            // Get timezone offset using jiff
            let (hours, minutes) = match get_offset_with_jiff(timezone_str) {
                Ok((h, m)) => (h, m),
                Err(_) => {
                    tracing::error!(
                        "Failed to get timezone offset for {}, defaulting to UTC",
                        timezone_str
                    );
                    (0, 0) // UTC default
                }
            };
            // Format offset string (e.g., "+02:00" or "-05:30")
            let offset = format!(
                "{}{:02}:{:02}",
                if hours >= 0 { "+" } else { "-" },
                hours.abs(),
                minutes.abs()
            );
            dynamic_variables.insert("timezone".to_string(), json!(timezone_str));
            dynamic_variables.insert("timezone_offset_from_utc".to_string(), json!(offset));
            let history_limit = 1;
            let history: Vec<crate::models::user_models::MessageHistory> = match state
                .user_repository
                .get_conversation_history(user.id, history_limit, /*include_tools=*/ false)
            {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!("Failed to fetch history: {:?}", e);
                    Vec::new()
                }
            };
            let history_string = history
                .iter()
                .rev() // oldest → newest
                .map(|m| format!("{}: {}", m.role, m.encrypted_content))
                .collect::<Vec<_>>()
                .join("\n");
            dynamic_variables.insert("recent_conversation".to_string(), json!(history_string));
            //dynamic_variables.insert("conversation_history".to_string(), json!(history_string));
            let charge_back_threshold = std::env::var("CHARGE_BACK_THRESHOLD")
                .unwrap_or_else(|_| "2.0".to_string())
                .parse::<f32>()
                .unwrap_or(2.00);
            let voice_second_cost = std::env::var("VOICE_SECOND_COST")
                .unwrap_or_else(|_| "0.0033".to_string())
                .parse::<f32>()
                .unwrap_or(0.0033);
            let user_current_credits_to_threshold = user.credits - charge_back_threshold;
            let seconds_to_threshold =
                (user_current_credits_to_threshold / voice_second_cost) as i32;
            // following just so it doesn't go negative although i don't think it matters
            let recharge_threshold_timestamp: i32 =
                (chrono::Utc::now().timestamp() as i32) + seconds_to_threshold;
            let seconds_to_zero_credits = (user.credits / voice_second_cost) as i32;
            let zero_credits_timestamp: i32 =
                (chrono::Utc::now().timestamp() as i32) + seconds_to_zero_credits;
            // log usage and start call
            if let Err(e) = state.user_repository.log_usage(LogUsageParams {
                user_id: user.id,
                sid: Some(call_sid),
                activity_type: "call".to_string(),
                credits: None,
                time_consumed: None,
                success: None,
                reason: None,
                status: Some("ongoing".to_string()),
                recharge_threshold_timestamp: Some(recharge_threshold_timestamp),
                zero_credits_timestamp: Some(zero_credits_timestamp),
            }) {
                tracing::error!("Failed to log call usage: {}", e);
                // Continue execution even if logging fails
            }
            // Get contact profile nicknames for voice pronunciation
            // This is faster than fetching from Matrix and contains user's important contacts
            let contact_nicknames: String = state
                .user_repository
                .get_contact_profiles(user.id)
                .unwrap_or_default()
                .iter()
                .map(|p| p.nickname.clone())
                .collect::<Vec<_>>()
                .join(", ");

            dynamic_variables.insert("recent_contacts".to_string(), json!(contact_nicknames));
        }
        Ok(None) => {
            tracing::debug!("No user found for number: {}", caller_number);
            dynamic_variables.insert("name".to_string(), json!(""));
            dynamic_variables.insert("user_info".to_string(), json!("new user"));
        }
        Err(e) => {
            tracing::error!("Error looking up user: {}", e);
            dynamic_variables.insert("name".to_string(), json!("Guest"));
            dynamic_variables.insert(
                "user_info".to_string(),
                json!({
                    "error": "Database error"
                }),
            );
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

/// Payload for creating a task via ElevenLabs voice call
#[derive(Deserialize)]
pub struct CreateTaskPayload {
    pub trigger_type: String, // "once", "recurring_email", "recurring_messaging"
    pub trigger_time: Option<String>, // ISO datetime for "once" triggers
    pub condition: Option<String>,
    pub action_spec: String, // Detailed step-by-step instructions for runtime AI
    pub notification_type: Option<String>,
}

pub async fn handle_create_task_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<CreateTaskPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Received task creation request");

    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid or missing user_id parameter"
                })),
            ));
        }
    };

    // Build trigger field
    let trigger = match payload.trigger_type.as_str() {
        "once" => {
            let time_str = match &payload.trigger_time {
                Some(t) => t,
                None => {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "error": "trigger_time is required for 'once' trigger type"
                        })),
                    ));
                }
            };

            // Get user's timezone
            let user_info = state.user_core.get_user_info(user_id).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to get user info: {:?}", e)})),
                )
            })?;
            let tz_str = user_info.timezone.unwrap_or_else(|| "UTC".to_string());
            let tz: chrono_tz::Tz = tz_str.parse().unwrap_or(chrono_tz::UTC);

            // Parse datetime and convert to timestamp
            let timestamp = parse_datetime_to_timestamp_elevenlabs(time_str, &tz).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": format!("Invalid trigger_time: {}", e)})),
                )
            })?;
            format!("once_{}", timestamp)
        }
        "recurring_email" => "recurring_email".to_string(),
        "recurring_messaging" => "recurring_messaging".to_string(),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("Invalid trigger_type: {}", payload.trigger_type)
                })),
            ));
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let new_task = crate::models::user_models::NewTask {
        user_id,
        trigger: trigger.clone(),
        condition: payload.condition.clone(),
        action: payload.action_spec.clone(),
        notification_type: payload.notification_type.or(Some("sms".to_string())),
        status: "active".to_string(),
        created_at: now,
    };

    // Build confirmation message (use a short summary, not full action_spec)
    let action_summary = if payload.action_spec.len() > 50 {
        format!("{}...", &payload.action_spec[..50])
    } else {
        payload.action_spec.clone()
    };

    match state.user_repository.create_task(&new_task) {
        Ok(_) => {
            tracing::debug!("Successfully created task for user: {}", user_id);

            let confirmation = match payload.trigger_type.as_str() {
                "once" => format!(
                    "Got it! I'll handle '{}' at the scheduled time.",
                    action_summary
                ),
                "recurring_email" => {
                    if let Some(cond) = &payload.condition {
                        format!(
                            "Got it! I'll watch for emails matching '{}' and execute the task.",
                            cond
                        )
                    } else {
                        format!(
                            "Got it! I'll monitor your emails and execute: {}",
                            action_summary
                        )
                    }
                }
                "recurring_messaging" => {
                    if let Some(cond) = &payload.condition {
                        format!(
                            "Got it! I'll watch for messages matching '{}' and execute the task.",
                            cond
                        )
                    } else {
                        format!(
                            "Got it! I'll monitor your messages and execute: {}",
                            action_summary
                        )
                    }
                }
                _ => format!("Task created: {}", action_summary),
            };

            Ok(Json(json!({
                "response": confirmation,
                "status": "success",
                "user_id": user_id,
            })))
        }
        Err(e) => {
            error!("Failed to create task: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to create task",
                    "details": e.to_string()
                })),
            ))
        }
    }
}

/// Parse ISO datetime string (in user's timezone) to UTC unix timestamp
fn parse_datetime_to_timestamp_elevenlabs(
    time_str: &str,
    tz: &chrono_tz::Tz,
) -> Result<i32, String> {
    use chrono::{NaiveDateTime, TimeZone};

    // Try parsing as ISO datetime (YYYY-MM-DDTHH:MM or YYYY-MM-DDTHH:MM:SS)
    let naive = if time_str.len() == 16 {
        NaiveDateTime::parse_from_str(&format!("{}:00", time_str), "%Y-%m-%dT%H:%M:%S")
            .map_err(|e| format!("Failed to parse datetime: {}", e))?
    } else {
        NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%S")
            .map_err(|e| format!("Failed to parse datetime: {}", e))?
    };

    // Interpret naive datetime in user's timezone, then get UTC timestamp
    let local_dt = tz
        .from_local_datetime(&naive)
        .single()
        .ok_or("Ambiguous or invalid local time")?;

    Ok(local_dt.timestamp() as i32)
}

#[derive(Deserialize)]
pub struct SetProactiveAgentPayload {
    pub user_id: i32,
    pub enabled: bool,
}

pub async fn handle_update_monitoring_status_tool_call(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SetProactiveAgentPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Received monitoring status update request");

    // Verify user exists
    match state.user_core.find_by_id(payload.user_id) {
        Ok(Some(_user)) => {
            match state
                .user_core
                .update_proactive_agent_on(payload.user_id, payload.enabled)
            {
                Ok(_) => {
                    tracing::debug!(
                        "Successfully updated monitoring status for user: {}",
                        payload.user_id
                    );
                    let status = if payload.enabled { "on" } else { "off" };
                    Ok(Json(json!({
                        "response": format!("Monitoring turned {}.", status),
                        "status": "success",
                        "user_id": payload.user_id,
                    })))
                }
                Err(e) => {
                    error!("Failed to update monitoring status: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to update monitoring status",
                            "details": e.to_string()
                        })),
                    ))
                }
            }
        }
        Ok(None) => {
            tracing::error!("User not found: {}", payload.user_id);
            Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "User not found"
                })),
            ))
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user",
                    "details": e.to_string()
                })),
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
                })),
            ));
        }
    };
    tracing::debug!("Received email fetch request for user: {}", user_id);

    match crate::handlers::imap_handlers::fetch_emails_imap(
        &state,
        user_id,
        true,
        Some(10),
        false,
        false,
    )
    .await
    {
        Ok(emails) => {
            if emails.is_empty() {
                return Ok(Json(json!({
                    "response": "I don't see any recent emails in your inbox.",
                    "emails": [],
                    "total_count": 0
                })));
            }

            // Format emails for voice response in a more natural way
            let mut response_text =
                format!("I found {} recent emails in your inbox. ", emails.len());

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
                let body = email
                    .body
                    .as_ref()
                    .map(|b| {
                        let cleaned = b.replace(['\n', '\r'], " ");
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
        }
        Err(e) => {
            error!("Failed to fetch emails for user {}: {:?}", user_id, e);

            // Provide user-friendly error message based on error type
            let user_message = match e {
                crate::handlers::imap_handlers::ImapError::NoConnection => {
                    "It looks like you haven't connected your email yet. You can set it up in the Lightfriend app settings."
                }
                crate::handlers::imap_handlers::ImapError::CredentialsError(_) => {
                    "I couldn't access your email because your credentials have expired or are invalid. Please reconnect your email in the Lightfriend app. If you're using Gmail, you may need to generate a new app password."
                }
                crate::handlers::imap_handlers::ImapError::ConnectionError(_) => {
                    "I'm having trouble connecting to your email server right now. This might be a temporary issue. Please try again in a moment."
                }
                _ => {
                    "I ran into a problem checking your email. Please check your email connection in the app settings and try again."
                }
            };

            Ok(Json(json!({
                "response": user_message,
                "emails": [],
                "total_count": 0
            })))
        }
    }
}

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
                })),
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
                })),
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
                })),
            ));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user"
                })),
            ));
        }
    };

    let mut message_sids = Vec::new();

    // Handle email attachments if email_id is provided - spawn as background task
    if let Some(email_id) = payload.email_id.clone() {
        tracing::debug!(
            "Spawning background task for email attachments processing for email ID: {}",
            email_id
        );

        let state_clone = Arc::clone(&state);
        let user_clone = user.clone();
        tokio::spawn(async move {
            tracing::debug!(
                "Background task: Fetching email attachments for email ID: {}",
                email_id
            );

            match fetch_single_email_imap(&state_clone, user_clone.id, &email_id).await {
                Ok(_) => {}
                Err(e) => {
                    error!("Background task: Failed to fetch email: {:?}", e);
                }
            }
        });
    }

    // Send the main message using Twilio
    match crate::api::twilio_utils::send_conversation_message(&state, &payload.message, None, &user)
        .await
    {
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
                })),
            ))
        }
    }
}

pub async fn handle_perplexity_tool_call(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MessageCallPayload>,
) -> Json<serde_json::Value> {
    let system_prompt = "You are assisting an AI voice calling service. The questions you receive are from voice conversations where users are seeking information or help. Please note: 1. Provide clear, conversational responses that can be easily read aloud 2. Avoid using any markdown, HTML, or other markup languages 3. Keep responses concise but informative 4. Use natural language sentence structure 5. When listing multiple points, use simple numbering (1, 2, 3) or natural language transitions (First... Second... Finally...) 6. Focus on the most relevant information that addresses the user's immediate needs 7. If specific numbers, dates, or proper names are important, spell them out clearly 8. Format numerical data in a way that's easy to read aloud (e.g., twenty-five percent instead of 25%) Your responses will be incorporated into a voice conversation, so clarity and natural flow are essential.";

    match crate::utils::tool_exec::ask_perplexity(&state, &payload.message, system_prompt).await {
        Ok(response) => Json(json!({
            "response": response
        })),
        Err(e) => {
            error!("Error getting response from Perplexity: {}", e);
            Json(json!({
                "error": "Failed to get response from AI"
            }))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FireCrawlCallPayload {
    query: String,
}

pub async fn handle_firecrawl_tool_call(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<FireCrawlCallPayload>,
) -> Json<serde_json::Value> {
    match crate::utils::tool_exec::handle_firecrawl_search(payload.query, 5).await {
        Ok(response) => Json(json!({
            "response": response
        })),
        Err(e) => {
            error!("Error getting response from Firecrawl: {}", e);
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
    match crate::handlers::google_calendar::handle_calendar_fetching(&state, user_id, start, end)
        .await
    {
        Ok(response) => response,
        Err((_, json_response)) => json_response,
    }
}

#[derive(Debug, Deserialize)]
pub struct EmailSearchPayload {
    pub search_term: String,
    pub search_type: Option<String>, // "sender", "subject", or "all"
}

#[derive(Debug, Deserialize)]
pub struct ChatSearchPayload {
    search_term: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatConfirmPayload {
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
                })),
            ));
        }
    };

    // First fetch recent emails with increased limit
    match fetch_emails_imap(&state, user_id, true, Some(50), false, false).await {
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
                let time_factor = email
                    .date
                    .map(|date| {
                        let age_in_days = (now - date.timestamp() as f64) / (24.0 * 60.0 * 60.0);
                        // Exponential decay: score drops by half every 7 days, but never below 0.1
                        (0.5f64.powf(age_in_days / 7.0)).max(0.1)
                    })
                    .unwrap_or(0.1); // Default factor for emails without dates

                // Helper closure for scoring
                let score_field =
                    |field: &Option<String>, _field_name: &str| -> Option<(f64, String)> {
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
                    }
                    "subject" => {
                        if let Some((score, match_type)) = score_field(&email.subject, "subject") {
                            if score > best_score {
                                best_score = score;
                                best_match_type = match_type;
                                best_matched_field = "subject".to_string();
                            }
                        }
                    }
                    _ => {
                        // "all" or any other value
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
            scored_emails.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

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
                        _ => "a match",
                    };

                    let from = full_email
                        .from
                        .as_ref()
                        .map_or("an unknown sender", String::as_str);
                    let subject = full_email
                        .subject
                        .as_ref()
                        .map_or("no subject", String::as_str);
                    let body = full_email
                        .body
                        .as_ref()
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
                            response_text.push_str(&format!(
                                "I also found {} more matching emails. ",
                                additional_matches
                            ));
                        }

                        // Add brief info about next few matches in a more conversational way
                        for (i, scored_email) in scored_emails.iter().skip(1).take(2).enumerate() {
                            let from = scored_email
                                .email
                                .from
                                .as_ref()
                                .map_or("an unknown sender", String::as_str);
                            let match_desc = match scored_email.match_type.as_str() {
                                "exact" => "exactly matches",
                                "substring" => "contains",
                                "similar" => "is similar to",
                                _ => "matches",
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
                }
                Err(e) => {
                    error!("Failed to fetch full email content: {:?}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to fetch full email content",
                            "details": format!("{:?}", e)
                        })),
                    ))
                }
            }
        }
        Err(e) => {
            error!(
                "Failed to fetch emails for search (user {}): {:?}",
                user_id, e
            );

            // Provide user-friendly error message based on error type
            let user_message = match e {
                crate::handlers::imap_handlers::ImapError::NoConnection => {
                    "It looks like you haven't connected your email yet. You can set it up in the Lightfriend app settings."
                }
                crate::handlers::imap_handlers::ImapError::CredentialsError(_) => {
                    "I couldn't access your email because your credentials have expired or are invalid. Please reconnect your email in the Lightfriend app. If you're using Gmail, you may need to generate a new app password."
                }
                crate::handlers::imap_handlers::ImapError::ConnectionError(_) => {
                    "I'm having trouble connecting to your email server right now. This might be a temporary issue. Please try again in a moment."
                }
                _ => {
                    "I ran into a problem searching your email. Please check your email connection in the app settings and try again."
                }
            };

            Ok(Json(json!({
                "response": user_message,
                "found": false
            })))
        }
    }
}

pub async fn handle_send_chat_message(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<ChatConfirmPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                })),
            ));
        }
    };
    // Extract platform from query parameters
    let platform = match params.get("platform") {
        Some(p) if p == "telegram" || p == "whatsapp" || p == "signal" => p.clone(),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid platform. Must be 'telegram' or 'whatsapp' or 'signal'."
                })),
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
                })),
            ));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user"
                })),
            ));
        }
    };
    let capitalized_platform = platform
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_default()
        + &platform[1..];
    // Check bridge connection
    let bridge = match state.user_repository.get_bridge(user_id, &platform) {
        Ok(bridge) => bridge,
        Err(e) => {
            error!("Failed to get bridge: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to get bridge"
                })),
            ));
        }
    };
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        let error_msg = format!(
            "Failed to find contact. Please make sure you're connected to {} bridge.",
            capitalized_platform
        );
        if let Err(e) =
            crate::api::twilio_utils::send_conversation_message(&state, &error_msg, None, &user)
                .await
        {
            error!("Failed to send error message: {}", e);
        }
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": error_msg
            })),
        ));
    }
    let client = match crate::utils::matrix_auth::get_cached_client(user_id, &state).await {
        Ok(client) => client,
        Err(e) => {
            let error_msg = format!("Failed to get client: {}", e);
            if let Err(e) =
                crate::api::twilio_utils::send_conversation_message(&state, &error_msg, None, &user)
                    .await
            {
                error!("Failed to send error message: {}", e);
            }
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": error_msg
                })),
            ));
        }
    };
    // Fetch rooms
    let rooms = match crate::utils::bridge::get_service_rooms(&client, &platform).await {
        Ok(rooms) => rooms,
        Err(e) => {
            let error_msg = format!("Failed to fetch {} rooms: {}", capitalized_platform, e);
            if let Err(e) =
                crate::api::twilio_utils::send_conversation_message(&state, &error_msg, None, &user)
                    .await
            {
                error!("Failed to send error message: {}", e);
            }
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": error_msg
                })),
            ));
        }
    };
    let best_match = crate::utils::bridge::search_best_match(&rooms, &payload.chat_name);
    let best_match = match best_match {
        Some(room) => room,
        None => {
            let error_msg = format!(
                "No {} contacts found matching '{}'.",
                capitalized_platform, payload.chat_name
            );
            if let Err(e) =
                crate::api::twilio_utils::send_conversation_message(&state, &error_msg, None, &user)
                    .await
            {
                error!("Failed to send error message: {}", e);
            }
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": error_msg
                })),
            ));
        }
    };
    // Get the exact name
    let exact_name = crate::utils::bridge::remove_bridge_suffix(&best_match.display_name);
    // Format the queued message
    let queued_msg = format!(
        "Will send {} to '{}' with '{}' in 60s. Use cancel_message tool to discard.",
        capitalized_platform, exact_name, payload.message
    );
    // Create cancellation channel
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    // Spawn the delayed send task
    let cloned_state = state.clone();
    let cloned_user_id = user_id;
    let cloned_user = user.clone();
    let cloned_platform = platform.clone();
    let cloned_capitalized_platform = capitalized_platform.clone();
    let cloned_exact_name = exact_name.clone();
    let cloned_message = payload.message.clone();
    tokio::spawn(async move {
        let reason = tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => "timeout",
            _ = cancel_rx => "cancel",
        };
        if reason == "timeout" {
            match crate::utils::bridge::send_bridge_message(
                &cloned_platform,
                &cloned_state,
                cloned_user_id,
                &cloned_exact_name,
                &cloned_message,
                None, // No image URL
            )
            .await
            {
                Ok(_) => {
                    // No additional success message
                }
                Err(e) => {
                    let error_msg = format!(
                        "Failed to send {} message: {}",
                        cloned_capitalized_platform, e
                    );
                    if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                        &cloned_state,
                        &error_msg,
                        None,
                        &cloned_user,
                    )
                    .await
                    {
                        error!("Failed to send error message: {}", e);
                    }
                }
            }
        }
        // Remove from map
        let mut senders = cloned_state.pending_message_senders.lock().await;
        senders.remove(&cloned_user_id);
    });
    // Store the cancel sender in the map
    {
        let mut senders = state.pending_message_senders.lock().await;
        senders.insert(user_id, cancel_tx);
    }
    Ok(Json(json!({
        "status": "success",
        "message": format!("{} message queued", capitalized_platform),
        "room_name": exact_name,
        "notification": queued_msg
    })))
}

#[derive(Debug, Deserialize)]
pub struct SendEmailArgs {
    pub to: String,
    pub subject: String,
    pub body: String,
}

pub async fn handle_email_send(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<SendEmailArgs>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                })),
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
                })),
            ));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user"
                })),
            ));
        }
    };
    // Format the queued message
    let queued_msg = format!(
        "Will send email to {} with subject '{}' and body '{}' in 60s. Use cancel_message tool to discard.",
        payload.to, payload.subject, payload.body
    );
    // Create cancellation channel
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    // Spawn the delayed send task
    let cloned_state = state.clone();
    let cloned_user_id = user_id;
    let cloned_user = user.clone();
    let cloned_to = payload.to.clone();
    let cloned_subject = payload.subject.clone();
    let cloned_body = payload.body.clone();
    tokio::spawn(async move {
        let reason = tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => "timeout",
            _ = cancel_rx => "cancel",
        };
        if reason == "timeout" {
            let email_request = crate::handlers::imap_handlers::SendEmailRequest {
                to: cloned_to,
                subject: cloned_subject,
                body: cloned_body,
            };
            match crate::handlers::imap_handlers::send_email(
                State(cloned_state.clone()),
                crate::handlers::auth_middleware::AuthUser {
                    user_id: cloned_user_id,
                    is_admin: false,
                },
                Json(email_request),
            )
            .await
            {
                Ok(_) => {
                    // No need to send success message
                }
                Err((_, error_json)) => {
                    let error_msg = format!(
                        "Failed to send email: {}",
                        error_json
                            .0
                            .get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown error")
                    );
                    if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                        &cloned_state,
                        &error_msg,
                        None,
                        &cloned_user,
                    )
                    .await
                    {
                        eprintln!("Failed to send error message: {}", e);
                    }
                }
            }
        }
        // Remove from map
        let mut senders = cloned_state.pending_message_senders.lock().await;
        senders.remove(&cloned_user_id);
    });
    // Store the cancel sender in the map
    {
        let mut senders = state.pending_message_senders.lock().await;
        senders.insert(user_id, cancel_tx);
    }
    Ok(Json(json!({
        "status": "success",
        "message": "Email queued",
        "notification": queued_msg
    })))
}

#[derive(Debug, Deserialize)]
pub struct RespondToEmailArgs {
    pub email_id: String,
    pub response_text: String,
}
pub async fn handle_respond_to_email(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<RespondToEmailArgs>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                })),
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
                })),
            ));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user"
                })),
            ));
        }
    };
    // Fetch the email details to get the subject
    let email_details = match crate::handlers::imap_handlers::fetch_single_imap_email(
        State(state.clone()),
        crate::handlers::auth_middleware::AuthUser {
            user_id,
            is_admin: false,
        },
        axum::extract::Path(payload.email_id.clone()),
    )
    .await
    {
        Ok(details) => details,
        Err((_, error_json)) => {
            let error_msg = format!(
                "Failed to fetch email details: {}",
                error_json
                    .0
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
            );
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": error_msg
                })),
            ));
        }
    };
    let subject = email_details
        .0
        .get("email")
        .and_then(|e| e.get("subject"))
        .and_then(|s| s.as_str())
        .unwrap_or("Unknown subject")
        .to_string();
    // Format the queued message using the subject
    let queued_msg = format!(
        "Will respond to email '{}' with '{}' in 60s. Use cancel_message to discard.",
        subject, payload.response_text
    );
    // Create cancellation channel
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    // Spawn the delayed send task
    let cloned_state = state.clone();
    let cloned_user_id = user_id;
    let cloned_user = user.clone();
    let cloned_email_id = payload.email_id.clone();
    let cloned_response_text = payload.response_text.clone();
    tokio::spawn(async move {
        let reason = tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => "timeout",
            _ = cancel_rx => "cancel",
        };
        if reason == "timeout" {
            let request = crate::handlers::imap_handlers::EmailResponseRequest {
                email_id: cloned_email_id,
                response_text: cloned_response_text,
            };
            match crate::handlers::imap_handlers::respond_to_email(
                State(cloned_state.clone()),
                crate::handlers::auth_middleware::AuthUser {
                    user_id: cloned_user_id,
                    is_admin: false,
                },
                Json(request),
            )
            .await
            {
                Ok(_) => {
                    // No need to send success message
                }
                Err((_, error_json)) => {
                    let error_msg = format!(
                        "Failed to respond to email: {}",
                        error_json
                            .0
                            .get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown error")
                    );
                    if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                        &cloned_state,
                        &error_msg,
                        None,
                        &cloned_user,
                    )
                    .await
                    {
                        eprintln!("Failed to send error message: {}", e);
                    }
                }
            }
        }
        // Remove from map
        let mut senders = cloned_state.pending_message_senders.lock().await;
        senders.remove(&cloned_user_id);
    });
    // Store the cancel sender in the map
    {
        let mut senders = state.pending_message_senders.lock().await;
        senders.insert(user_id, cancel_tx);
    }
    Ok(Json(json!({
        "status": "success",
        "message": "Email response queued",
        "notification": queued_msg
    })))
}

pub async fn handle_calendar_event_creation(
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
                })),
            ));
        }
    };
    // Parse the start time
    let start_time = match chrono::DateTime::parse_from_rfc3339(&payload.start_time) {
        Ok(dt) => dt.with_timezone(&chrono::Utc),
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid start_time format. Please use RFC3339 format."
                })),
            ));
        }
    };
    // Create the event request
    let event_request = crate::handlers::google_calendar::CreateEventRequest {
        summary: payload.summary.clone(),
        description: payload.description.clone(),
        start_time,
        duration_minutes: payload.duration_minutes,
        add_notification: payload.add_notification.unwrap_or(false),
    };
    // Create the event directly
    match crate::handlers::google_calendar::create_calendar_event(
        State(state.clone()),
        crate::handlers::auth_middleware::AuthUser {
            user_id,
            is_admin: false,
        },
        Json(event_request),
    )
    .await
    {
        Ok(response) => Ok(Json(json!({
            "status": "success",
            "message": "Calendar event created successfully",
            "event": response.0
        }))),
        Err(e) => {
            error!("Failed to create calendar event: {:?}", e);
            Err(e)
        }
    }
}

pub async fn handle_search_chat_contacts_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<ChatSearchPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                })),
            ));
        }
    };
    // Extract platform from query parameters
    let platform = match params.get("platform") {
        Some(p) if p == "signal" || p == "telegram" || p == "whatsapp" => p.clone(),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid platform. Must be 'telegram' or 'whatsapp' or 'signal'."
                })),
            ));
        }
    };
    // Search for rooms using the existing utility function
    match crate::utils::bridge::search_bridge_rooms(
        &platform,
        &state,
        user_id,
        &payload.search_term,
    )
    .await
    {
        Ok(rooms) => {
            let capitalized_platform = platform
                .chars()
                .next()
                .map(|c| c.to_uppercase().collect::<String>())
                .unwrap_or_default()
                + &platform[1..];
            if rooms.is_empty() {
                return Ok(Json(json!({
                    "response": format!("No {} contacts found matching '{}'.", capitalized_platform, payload.search_term),
                    "rooms": []
                })));
            }
            // Format rooms for voice response
            let mut response_text = format!(
                "Found {} matching {} contacts. ",
                rooms.len(),
                capitalized_platform
            );
            // Add up to 5 most relevant rooms to the voice response
            for (i, room) in rooms.iter().take(5).enumerate() {
                response_text.push_str(&format!(
                    "Contact {} is {}, last active {}. ",
                    i + 1,
                    room.display_name
                        .trim_end_matches(" (WA)")
                        .trim_end_matches(" (Telegram)"),
                    room.last_activity_formatted
                ));
            }
            if rooms.len() > 5 {
                response_text.push_str(&format!("And {} more contacts found. ", rooms.len() - 5));
            }
            Ok(Json(json!({
                "response": response_text,
                "rooms": rooms,
                "total_count": rooms.len()
            })))
        }
        Err(e) => {
            error!("Failed to search {} rooms: {}", platform, e);
            let capitalized_platform = platform
                .chars()
                .next()
                .map(|c| c.to_uppercase().collect::<String>())
                .unwrap_or_default()
                + &platform[1..];
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to search {} contacts", capitalized_platform),
                    "details": e.to_string()
                })),
            ))
        }
    }
}

pub async fn handle_fetch_specific_chat_messages_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Starting specific chat message fetch");
    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                })),
            ));
        }
    };
    // Extract platform from query parameters
    let platform = match params.get("platform") {
        Some(p) if p == "signal" || p == "telegram" || p == "whatsapp" => p.clone(),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid platform. Must be 'telegram' or 'whatsapp' or 'signal'."
                })),
            ));
        }
    };
    let chat_room = match params.get("chat_room") {
        Some(room) => room.clone(),
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing chat_room parameter"
                })),
            ));
        }
    };
    // Fetch messages using the existing utility function
    match crate::utils::bridge::fetch_bridge_room_messages(
        &platform,
        &state,
        user_id,
        &chat_room,
        Some(20),
    )
    .await
    {
        Ok((messages, room_name)) => {
            let capitalized_platform = platform
                .chars()
                .next()
                .map(|c| c.to_uppercase().collect::<String>())
                .unwrap_or_default()
                + &platform[1..];
            if messages.is_empty() {
                return Ok(Json(json!({
                    "response": format!("No {} messages found in chat room '{}'.", capitalized_platform, chat_room),
                    "messages": []
                })));
            }
            // Format messages for voice response
            let mut response_text = format!(
                "Here are the recent messages from {}: ",
                room_name
                    .trim_end_matches(" (WA)")
                    .trim_end_matches(" (Telegram)")
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
        }
        Err(e) => {
            error!("Failed to fetch {} messages: {}", platform, e);
            let capitalized_platform = platform
                .chars()
                .next()
                .map(|c| c.to_uppercase().collect::<String>())
                .unwrap_or_default()
                + &platform[1..];
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to fetch {} messages", capitalized_platform),
                    "details": e.to_string()
                })),
            ))
        }
    }
}

pub async fn handle_fetch_recent_messages_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Starting chat message fetch with time range");
    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid user_id"
                })),
            ));
        }
    };
    // Extract platform from query parameters
    let platform = match params.get("platform") {
        Some(p) if p == "telegram" || p == "signal" || p == "whatsapp" => p.clone(),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing or invalid platform. Must be 'telegram' or 'whatsapp' or 'signal'."
                })),
            ));
        }
    };
    // Set start_time to 2 days ago
    let start_timestamp = (chrono::Utc::now() - chrono::Duration::days(1)).timestamp();
    // Fetch messages using the existing utility function
    match crate::utils::bridge::fetch_bridge_messages(
        &platform,
        &state,
        user_id,
        start_timestamp,
        false,
    )
    .await
    {
        Ok(messages) => {
            if messages.is_empty() {
                let capitalized_platform = platform
                    .chars()
                    .next()
                    .map(|c| c.to_uppercase().collect::<String>())
                    .unwrap_or_default()
                    + &platform[1..];
                return Ok(Json(json!({
                    "response": format!("No {} messages found for the specified time range.", capitalized_platform),
                    "messages": []
                })));
            }
            // Format messages for voice response
            let capitalized_platform = platform
                .chars()
                .next()
                .map(|c| c.to_uppercase().collect::<String>())
                .unwrap_or_default()
                + &platform[1..];
            let mut response_text = format!(
                "Found {} {} messages. Here are the highlights: ",
                messages.len(),
                capitalized_platform
            );
            // Add up to 5 most recent messages to the voice response
            for (i, msg) in messages.iter().take(20).enumerate() {
                response_text.push_str(&format!(
                    "Message {} in chat {}, sent on {}: {}. ",
                    i + 1,
                    msg.room_name,
                    msg.formatted_timestamp,
                    msg.content
                ));
            }
            if messages.len() > 20 {
                response_text.push_str(&format!("And {} more messages. ", messages.len() - 20));
            }
            Ok(Json(json!({
                "response": response_text,
                "messages": messages,
                "total_count": messages.len()
            })))
        }
        Err(e) => {
            error!("Failed to fetch {} messages: {}", platform, e);
            let capitalized_platform = platform
                .chars()
                .next()
                .map(|c| c.to_uppercase().collect::<String>())
                .unwrap_or_default()
                + &platform[1..];
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to fetch {} messages", capitalized_platform),
                    "details": e.to_string()
                })),
            ))
        }
    }
}

pub async fn handle_cancel_pending_message_tool_call(
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
                })),
            ));
        }
    };
    tracing::debug!(
        "Received cancel pending message request for user: {}",
        user_id
    );
    // Verify user exists
    match state.user_core.find_by_id(user_id) {
        Ok(Some(_user)) => {
            match crate::tool_call_utils::utils::cancel_pending_message(&state, user_id).await {
                Ok(true) => {
                    tracing::debug!(
                        "Successfully cancelled pending message for user: {}",
                        user_id
                    );
                    Ok(Json(json!({
                        "response": "Pending message cancelled.",
                        "status": "success",
                        "user_id": user_id,
                    })))
                }
                Ok(false) => {
                    tracing::debug!("No pending message to cancel for user: {}", user_id);
                    Ok(Json(json!({
                        "response": "No pending message to cancel.",
                        "status": "success",
                        "user_id": user_id,
                    })))
                }
                Err(e) => {
                    error!("Failed to cancel pending message: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to cancel pending message",
                            "details": e.to_string()
                        })),
                    ))
                }
            }
        }
        Ok(None) => {
            tracing::error!("User not found: {}", user_id);
            Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "User not found"
                })),
            ))
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user",
                    "details": e.to_string()
                })),
            ))
        }
    }
}

pub async fn make_notification_call(
    state: &Arc<AppState>,
    content_type: String,
    notification_first_message: String,
    notification_message: String,
    user_id: String,
    user_timezone: Option<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Get user information to check for discount tier
    let user = match state
        .user_core
        .find_by_id(user_id.parse::<i32>().unwrap_or_default())
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            error!("User not found for ID: {}", user_id);
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "User not found",
                    "details": "Could not find user with provided ID"
                })),
            ));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Database error",
                    "details": e.to_string()
                })),
            ));
        }
    };

    let to_phone_number = user.phone_number.clone();
    let user_settings = match state.user_core.get_user_settings(user.id) {
        Ok(settings) => settings,
        Err(e) => {
            error!("Failed to get user settings: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to get user settings",
                    "message": "Internal server error"
                })),
            ));
        }
    };
    // Get or set phone_number_country
    let country = match user.phone_number_country {
        Some(c) => c,
        None => {
            match crate::handlers::profile_handlers::set_user_phone_country(
                state,
                user.id,
                &user.phone_number,
            )
            .await
            {
                Ok(Some(c)) => c,
                Ok(None) => {
                    error!(
                        "Failed to determine country for user {} after lookup",
                        user.id
                    );
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Missing user country",
                            "details": "Could not determine phone number country"
                        })),
                    ));
                }
                Err(e) => {
                    error!(
                        "Failed to set phone_number_country for user {}: {}",
                        user.id, e
                    );
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to set user country",
                            "details": e.to_string()
                        })),
                    ));
                }
            }
        }
    };
    // Check if the user's country is supported (US, FI, GB, AU, DE, CA)
    let is_supported_country = matches!(
        country.as_str(),
        "FI" | "NL" | "AU" | "GB" | "US" | "DE" | "CA"
    );
    let phone_number_id = if is_supported_country {
        // Regular phone number selection based on country
        match country.as_str() {
            "FI" => std::env::var("FIN_PHONE_NUMBER_ID"),
            "NL" => std::env::var("NL_PHONE_NUMBER_ID"),
            "AU" => std::env::var("AUS_PHONE_NUMBER_ID"),
            "GB" => std::env::var("GB_PHONE_NUMBER_ID"),
            "CA" => std::env::var("CAN_PHONE_NUMBER_ID"),
            "US" => std::env::var("USA_PHONE_NUMBER_ID"),
            _ => std::env::var("USA_PHONE_NUMBER_ID"), // Default to USA number for unsupported countries
        }
        .map_err(|_| {
            error!("Failed to get phone number ID for country: {}", country);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to get phone number ID",
                    "details": "Environment variable not set"
                })),
            )
        })?
    } else {
        // Unsupported country: Use user's ElevenLabs phone number ID if available
        match user_settings.elevenlabs_phone_number_id {
            Some(id) => id,
            None => {
                tracing::info!(
                    "No ElevenLabs phone number ID found for user {} in unsupported country",
                    user.id
                );
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "Unsupported country",
                        "message": "No phone number ID available for this country. Call cannot be sent."
                    })),
                ));
            }
        }
    };
    // Get voice ID based on country
    let voice_id = match user_settings.agent_language.to_lowercase().as_str() {
        "fi" => std::env::var("FI_VOICE_ID").expect("FI_VOICE_ID not set"),
        "de" => std::env::var("DE_VOICE_ID").expect("DE_VOICE_ID not set"),
        _ => std::env::var("US_VOICE_ID").expect("US_VOICE_ID not set"), // Default for US/CA/GB/AU/Other
    };
    // Create dynamic variables map with notification message
    let mut dynamic_variables = HashMap::new();
    dynamic_variables.insert(
        "notification_message".to_string(),
        json!(notification_message),
    );
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
            tracing::error!(
                "Failed to get timezone offset for {}, defaulting to UTC",
                timezone_str
            );
            (0, 0) // UTC default
        }
    };
    // Format offset string (e.g., "+02:00" or "-05:30")
    let offset = format!(
        "{}{:02}:{:02}",
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
                tts: VoiceId { voice_id },
            },
            dynamic_variables,
        },
    };
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.elevenlabs.io/v1/convai/twilio/outbound-call".to_string())
        .header(
            "xi-api-key",
            std::env::var("ELEVENLABS_API_KEY").expect("ELEVENLABS_API_KEY not set"),
        )
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
                })),
            )
        })?;
    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        error!("ElevenLabs API returned error: {}", error_text);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "ElevenLabs API returned error",
                "details": error_text
            })),
        ));
    }

    Ok(Json(json!({
        "status": "success",
        "message": "Notification call initiated successfully",
        "to_number": to_phone_number,
        "from_number_id": phone_number_id,
    })))
}

pub async fn handle_weather_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<LocationCallPayload>,
) -> Json<serde_json::Value> {
    // Extract user_id from query parameters
    let user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Json(json!({
                "error": "Missing or invalid user_id",
            }));
        }
    };

    let forecast_type = payload.forecast_type.as_deref().unwrap_or("current");
    match crate::utils::tool_exec::get_weather(
        &state,
        &payload.location,
        &payload.units,
        forecast_type,
        user_id,
    )
    .await
    {
        Ok(weather_info) => Json(json!({
            "response": weather_info
        })),
        Err(e) => {
            error!("Error getting weather information: {}", e);
            Json(json!({
                "error": "Failed to get weather information",
                "details": e.to_string()
            }))
        }
    }
}

#[derive(Deserialize)]
pub struct DirectionsCallPayload {
    pub start_address: String,
    pub end_address: String,
    pub mode: Option<String>,
}

pub async fn handle_directions_tool_call(
    State(_state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<DirectionsCallPayload>,
) -> Json<serde_json::Value> {
    // Extract user_id from query parameters
    let _user_id = match params.get("user_id").and_then(|id| id.parse::<i32>().ok()) {
        Some(id) => id,
        None => {
            return Json(json!({
                "error": "Missing or invalid user_id",
            }));
        }
    };

    match crate::tool_call_utils::internet::handle_directions_tool(
        payload.start_address,
        payload.end_address,
        payload.mode,
    )
    .await
    {
        Ok(directions_info) => Json(json!({
            "response": directions_info
        })),
        Err(e) => {
            error!("Error getting directions information: {}", e);
            Json(json!({
                "error": "Failed to get directions information",
                "details": e.to_string()
            }))
        }
    }
}

// =====================================================
// Web-based voice call endpoints (browser to ElevenLabs)
// =====================================================

/// Response from ElevenLabs signed URL API
#[derive(Deserialize)]
struct ElevenLabsSignedUrlResponse {
    signed_url: String,
}

/// Response for web call initiation
#[derive(Serialize)]
pub struct WebCallInitResponse {
    signed_url: String,
    user_id: i32,
    max_duration_secs: i32,
    cost_per_minute: f32,
    // Conversation overrides for the agent
    agent_overrides: serde_json::Value,
}

/// Get signed URL for web-based voice call
/// This endpoint requires JWT authentication and checks user credits
pub async fn get_web_signed_url(
    State(state): State<Arc<AppState>>,
    auth_user: crate::handlers::auth_middleware::AuthUser,
) -> Result<Json<WebCallInitResponse>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;
    // Get user
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            ));
        }
        Err(e) => {
            error!("Database error finding user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            ));
        }
    };

    // Check if user has subscription
    if user.sub_tier.is_none() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Subscription required for web calls"})),
        ));
    }

    // Determine cost per minute based on region
    let is_us_ca = user.phone_number.starts_with("+1");
    let cost_per_minute = if is_us_ca {
        1.0 // 1 message credit per minute for US/CA
    } else {
        0.15 // 0.15 EUR per minute for Euro countries
    };

    // Check if user has at least 1 minute worth of credits
    if let Err(e) =
        crate::utils::usage::check_user_credits(&state, &user, "web_call", Some(60)).await
    {
        return Err((StatusCode::PAYMENT_REQUIRED, Json(json!({"error": e}))));
    }

    // Get user settings and info for dynamic variables
    let user_settings = state.user_core.get_user_settings(user_id).ok();
    let user_info = state.user_core.get_user_info(user_id).ok();

    // Build dynamic variables like fetch_assistant does
    let nickname = user.nickname.clone().unwrap_or_default();
    let user_own_info = user_info
        .as_ref()
        .and_then(|i| i.info.clone())
        .unwrap_or_default();
    let user_location = user_info
        .as_ref()
        .and_then(|i| i.location.clone())
        .unwrap_or_default();
    let nearby_places = user_info
        .as_ref()
        .and_then(|i| i.nearby_places.clone())
        .unwrap_or_default();
    let timezone_str = user_info
        .as_ref()
        .and_then(|i| i.timezone.clone())
        .unwrap_or_else(|| "UTC".to_string());

    // Get timezone offset
    let (hours, minutes) = match get_offset_with_jiff(&timezone_str) {
        Ok((h, m)) => (h, m),
        Err(_) => (0, 0),
    };
    let offset = format!(
        "{}{:02}:{:02}",
        if hours >= 0 { "+" } else { "-" },
        hours.abs(),
        minutes.abs()
    );

    // Get voice ID based on language
    let agent_language = user_settings
        .as_ref()
        .map(|s| s.agent_language.as_str())
        .unwrap_or("en");
    let voice_id = match agent_language {
        "fi" => std::env::var("FI_VOICE_ID")
            .unwrap_or_else(|_| std::env::var("US_VOICE_ID").unwrap_or_default()),
        "de" => std::env::var("DE_VOICE_ID")
            .unwrap_or_else(|_| std::env::var("US_VOICE_ID").unwrap_or_default()),
        _ => std::env::var("US_VOICE_ID").unwrap_or_default(),
    };

    // Build first message based on language
    let first_message = match agent_language {
        "fi" => format!("Moi {}!", nickname),
        "de" => format!("Hallo {}!", nickname),
        _ => format!("Hello {}!", nickname),
    };

    // Build agent overrides (using camelCase for JS SDK)
    // Note: Only include fields we want to override - omit empty/default fields per ElevenLabs docs
    let agent_overrides = json!({
        "agent": {
            "firstMessage": first_message,
            "language": agent_language
        },
        "tts": {
            "voiceId": voice_id
        }
    });

    // Get contact profile nicknames for voice pronunciation
    let contact_nicknames: String = state
        .user_repository
        .get_contact_profiles(user_id)
        .unwrap_or_default()
        .iter()
        .map(|p| p.nickname.clone())
        .collect::<Vec<_>>()
        .join(", ");

    // Build dynamic variables (these get substituted into the prompt)
    let dynamic_variables = json!({
        "user_id": user_id.to_string(),
        "name": nickname,
        "user_info": user_own_info,
        "location": user_location,
        "nearby_places": nearby_places,
        "timezone": timezone_str,
        "timezone_offset_from_utc": offset,
        "now": format!("{}", chrono::Utc::now()),
        "email_id": "-1",
        "content_type": "",
        "notification_message": "",
        "recent_contacts": contact_nicknames
    });

    // Get agent ID from environment
    let agent_id = std::env::var("AGENT_ID")
        .or_else(|_| std::env::var("ASSISTANT_ID"))
        .unwrap_or_else(|_| "202a9873-5f36-42bb-8162-a2010ad97e62".to_string());

    // Get ElevenLabs API key
    let api_key = match std::env::var("ELEVENLABS_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            error!("ELEVENLABS_API_KEY not set");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Server configuration error"})),
            ));
        }
    };

    // Request signed URL from ElevenLabs
    let client = reqwest::Client::new();
    let response = client
        .get(format!(
            "https://api.elevenlabs.io/v1/convai/conversation/get_signed_url?agent_id={}",
            agent_id
        ))
        .header("xi-api-key", &api_key)
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<ElevenLabsSignedUrlResponse>().await {
                    Ok(data) => {
                        // Calculate max duration based on user's credits
                        // For US/CA: max minutes = credits_left (message units)
                        // For Euro: max minutes = credits_left / 0.15
                        let max_minutes = if is_us_ca {
                            (user.credits_left + user.credits).floor() as i32
                        } else {
                            ((user.credits_left + user.credits) / 0.15).floor() as i32
                        };
                        // Cap at 30 minutes max per call
                        let max_duration_secs = (max_minutes.min(30) * 60).max(60);

                        // Combine overrides with dynamic variables
                        let full_overrides = json!({
                            "agent": agent_overrides["agent"],
                            "tts": agent_overrides["tts"],
                            "dynamicVariables": dynamic_variables
                        });

                        Ok(Json(WebCallInitResponse {
                            signed_url: data.signed_url,
                            user_id,
                            max_duration_secs,
                            cost_per_minute,
                            agent_overrides: full_overrides,
                        }))
                    }
                    Err(e) => {
                        error!("Failed to parse ElevenLabs response: {}", e);
                        Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": "Failed to get signed URL"})),
                        ))
                    }
                }
            } else {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                error!("ElevenLabs API error: {} - {}", status, body);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to get signed URL from ElevenLabs"})),
                ))
            }
        }
        Err(e) => {
            error!("Network error calling ElevenLabs: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Network error"})),
            ))
        }
    }
}

/// Request body for ending a web call
#[derive(Deserialize)]
pub struct EndWebCallRequest {
    duration_secs: i32,
}

/// End web call and deduct credits based on duration
pub async fn end_web_call(
    State(state): State<Arc<AppState>>,
    auth_user: crate::handlers::auth_middleware::AuthUser,
    Json(payload): Json<EndWebCallRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;
    let duration_secs = payload.duration_secs.max(0);

    // Calculate minutes (round up)
    let minutes = ((duration_secs as f32) / 60.0).ceil() as i32;

    if minutes == 0 {
        return Ok(Json(json!({
            "status": "ok",
            "message": "No charge for calls under 1 second",
            "duration_secs": duration_secs,
            "minutes_charged": 0
        })));
    }

    // Deduct credits based on minutes
    // Pass duration in seconds to deduct_user_credits which will calculate the cost
    if let Err(e) =
        crate::utils::usage::deduct_user_credits(&state, user_id, "web_call", Some(duration_secs))
    {
        error!("Failed to deduct credits for web call: {}", e);
        // Still return success - the call happened, we should log it
    }

    // Log the usage
    if let Err(e) = state.user_repository.log_usage(LogUsageParams {
        user_id,
        sid: None, // no call_sid for web calls
        activity_type: "web_call".to_string(),
        credits: None,
        time_consumed: Some(duration_secs),
        success: None,
        reason: None,
        status: Some("done".to_string()),
        recharge_threshold_timestamp: None,
        zero_credits_timestamp: None,
    }) {
        error!("Failed to log web call usage: {}", e);
    }

    Ok(Json(json!({
        "status": "ok",
        "duration_secs": duration_secs,
        "minutes_charged": minutes
    })))
}

/// Check if user has enough credits for continued web call
/// Returns remaining minutes available
pub async fn check_web_call_credits(
    State(state): State<Arc<AppState>>,
    auth_user: crate::handlers::auth_middleware::AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            ));
        }
        Err(e) => {
            error!("Database error: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            ));
        }
    };

    let is_us_ca = user.phone_number.starts_with("+1");

    // Calculate remaining minutes
    let remaining_minutes = if is_us_ca {
        // US/CA: credits_left is message count, 1 message = 1 minute
        (user.credits_left + user.credits).floor() as i32
    } else {
        // Euro: credits_left is EUR, 0.15 EUR per minute
        ((user.credits_left + user.credits) / 0.15).floor() as i32
    };

    let has_credits = remaining_minutes >= 1;

    Ok(Json(json!({
        "has_credits": has_credits,
        "remaining_minutes": remaining_minutes,
        "should_end_call": !has_credits
    })))
}
