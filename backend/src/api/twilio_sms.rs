use std::sync::Arc;
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::cell::RefCell;
use axum::{
    extract::Form,
    extract::State,
    http::StatusCode,
    Json,
};
use crate::tool_call_utils::utils::{
    ChatMessage, create_openai_client_for_user,
};
use crate::repositories::user_repository::LogUsageParams;
use crate::{ModelPurpose, AiProvider};
use chrono::Utc;

// Thread-local storage for media SID mapping
thread_local! {
    static MEDIA_SID_MAP: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

use openai_api_rs::v1::chat_completion;

/// Error messages for tool call failures - privacy-safe, user-facing
mod tool_error_messages {
    /// Internal Lightfriend errors (our fault)
    pub const INTERNAL_ERROR: &str = "Sorry, we encountered an issue processing your request. Our team has been notified.";
    /// External service errors (not our fault)
    pub const PERPLEXITY_UNAVAILABLE: &str = "Sorry, I couldn't reach the search service right now.";
    pub const WEATHER_UNAVAILABLE: &str = "Sorry, I couldn't get the weather information right now.";
    pub const FIRECRAWL_UNAVAILABLE: &str = "Sorry, I couldn't search the web right now.";
    pub const DIRECTIONS_UNAVAILABLE: &str = "Sorry, I couldn't get directions right now.";
}

/// Log a tool call error without exposing user content (privacy-safe)
fn log_tool_error(user_id: i32, tool_name: &str, category: &str, error_type: &str, error_msg: &str) {
    tracing::error!(
        user_id = user_id,
        tool_name = tool_name,
        error_category = category,
        error_type = error_type,
        "Tool execution failed: {}", error_msg
    );
}

// =============================================================================
// SmsResult - Standardized SMS processing outcomes
// =============================================================================

/// The standard response type for SMS processing.
/// This is the tuple returned by process_sms and related functions.
pub type SmsProcessResponse = (StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<TwilioResponse>);

/// Represents the outcome of SMS processing.
/// Use this to build consistent responses across all error paths.
#[derive(Debug)]
pub enum SmsResult {
    /// Successful response - user should be charged
    Success { response: String },
    /// User-caused error (no credits, no subscription, etc.) - don't charge
    UserError { message: String, status: StatusCode },
    /// System error (our fault) - don't charge, log internally
    SystemError { log_msg: String },
    /// Cancel command received - don't charge
    Cancelled { message: String },
}

impl SmsResult {
    /// Convert to the standard response tuple
    pub fn into_response(self) -> SmsProcessResponse {
        let headers = [(axum::http::header::CONTENT_TYPE, "application/json")];
        match self {
            SmsResult::Success { response } => (
                StatusCode::OK,
                headers,
                axum::Json(TwilioResponse { message: response }),
            ),
            SmsResult::UserError { message, status } => (
                status,
                headers,
                axum::Json(TwilioResponse { message }),
            ),
            SmsResult::SystemError { log_msg } => {
                tracing::error!("SMS system error: {}", log_msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    headers,
                    axum::Json(TwilioResponse {
                        message: tool_error_messages::INTERNAL_ERROR.to_string(),
                    }),
                )
            }
            SmsResult::Cancelled { message } => (
                StatusCode::OK,
                headers,
                axum::Json(TwilioResponse { message }),
            ),
        }
    }

    /// Check if this result should trigger credit deduction
    pub fn should_charge(&self) -> bool {
        matches!(self, SmsResult::Success { .. })
    }

    /// Helper to create a user not found error
    pub fn user_not_found() -> Self {
        SmsResult::UserError {
            message: "User not found".to_string(),
            status: StatusCode::NOT_FOUND,
        }
    }

    /// Helper to create an insufficient credits error
    pub fn insufficient_credits() -> Self {
        SmsResult::UserError {
            message: "Insufficient credits. Please add more credits to continue.".to_string(),
            status: StatusCode::PAYMENT_REQUIRED,
        }
    }

    /// Helper to create a no subscription error
    pub fn no_subscription() -> Self {
        SmsResult::UserError {
            message: "Active subscription required. Please subscribe to continue using the service.".to_string(),
            status: StatusCode::FORBIDDEN,
        }
    }

    /// Helper to create a deactivated phone error
    pub fn phone_deactivated() -> Self {
        SmsResult::UserError {
            message: "Phone service is currently deactivated for this number.".to_string(),
            status: StatusCode::FORBIDDEN,
        }
    }

    /// Helper to create a database error
    pub fn database_error(context: &str) -> Self {
        SmsResult::SystemError {
            log_msg: format!("Database error: {}", context),
        }
    }
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
    #[serde(rename = "MessageSid")]
    pub message_sid: String,
}

#[derive(Serialize, Debug)]
pub struct TwilioResponse {
    #[serde(rename = "Message")]
    pub message: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TextBeeWebhookPayload {
    pub device_id: String,  // Required for verification
    pub sender: String,     // Maps to 'from'
    pub recipient: String,  // Maps to 'to' (your device's number)
    pub body: String,
}

/// Options for process_sms to control test behavior
#[derive(Default)]
pub struct ProcessSmsOptions {
    /// Skip actual Twilio SMS sending
    pub skip_twilio_send: bool,
    /// Skip credit deduction (for callers that handle credits themselves)
    pub skip_credit_deduction: bool,
    /// Mock LLM response to use instead of calling real LLM API
    pub mock_llm_response: Option<openai_api_rs::v1::chat_completion::ChatCompletionResponse>,
}

impl ProcessSmsOptions {
    /// Create options for normal production use
    pub fn production() -> Self {
        Self::default()
    }

    /// Create options for web chat (skip Twilio, skip credits - caller handles credits)
    pub fn web_chat() -> Self {
        Self {
            skip_twilio_send: true,
            skip_credit_deduction: true,
            mock_llm_response: None,
        }
    }

    /// Create options for testing with mock LLM response (deducts credits for testing)
    pub fn test_with_mock(mock_response: openai_api_rs::v1::chat_completion::ChatCompletionResponse) -> Self {
        Self {
            skip_twilio_send: true,
            skip_credit_deduction: false, // Still deduct credits so we can test credit deduction
            mock_llm_response: Some(mock_response),
        }
    }
}

// =============================================================================
// SmsResponse - Centralizes SMS length enforcement
// =============================================================================

/// Wrapper for SMS response content that enforces the 480 character limit.
/// All SMS responses should go through this struct to ensure proper length handling.
pub struct SmsResponse {
    content: String,
}

impl SmsResponse {
    /// Maximum SMS response length in characters
    pub const MAX_LENGTH: usize = 480;

    /// Create a new SMS response, automatically condensing with LLM if needed.
    /// Use this for normal responses where we want intelligent condensing.
    pub async fn new(
        raw: String,
        client: &openai_api_rs::v1::api::OpenAIClient,
        model: &str,
    ) -> Self {
        let content = if raw.chars().count() > Self::MAX_LENGTH {
            // Try to condense with LLM first, fall back to truncation
            condense_response(client, &raw, Self::MAX_LENGTH, model)
                .await
                .unwrap_or_else(|_| truncate_nicely(&raw, Self::MAX_LENGTH))
        } else {
            raw
        };
        Self { content }
    }

    /// Create a response with simple truncation (no LLM condensing).
    /// Use this for error messages or when LLM is not available.
    pub fn truncated(raw: String) -> Self {
        let content = if raw.chars().count() > Self::MAX_LENGTH {
            truncate_nicely(&raw, Self::MAX_LENGTH)
        } else {
            raw
        };
        Self { content }
    }

    /// Create a response that's already known to be within limits.
    /// Panics in debug mode if content exceeds limit.
    pub fn from_static(content: &'static str) -> Self {
        debug_assert!(
            content.chars().count() <= Self::MAX_LENGTH,
            "Static response exceeds SMS limit: {} chars",
            content.chars().count()
        );
        Self { content: content.to_string() }
    }

    /// Get the content as a String
    pub fn into_inner(self) -> String {
        self.content
    }

    /// Get a reference to the content
    pub fn as_str(&self) -> &str {
        &self.content
    }
}

/// Get the model to use based on provider and purpose.
/// Uses centralized AiConfig from AppState.
pub fn get_model(state: &Arc<AppState>, provider: AiProvider, purpose: ModelPurpose) -> String {
    state.ai_config.model(provider, purpose).to_string()
}

/// Truncate a string nicely at word boundaries, adding "..." if truncated
fn truncate_nicely(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    // Leave room for "..."
    let target_len = max_chars.saturating_sub(3);

    // Find a good break point (end of sentence or word)
    let chars: Vec<char> = text.chars().collect();
    let mut break_point = target_len;

    // Try to find end of sentence within last 50 chars
    for i in (target_len.saturating_sub(50)..=target_len).rev() {
        if i < chars.len() && (chars[i] == '.' || chars[i] == '!' || chars[i] == '?') {
            // Check if next char is space or end
            if i + 1 >= chars.len() || chars[i + 1].is_whitespace() {
                break_point = i + 1;
                return chars[..break_point].iter().collect();
            }
        }
    }

    // Otherwise find last space
    for i in (0..=target_len).rev() {
        if i < chars.len() && chars[i].is_whitespace() {
            break_point = i;
            break;
        }
    }

    let truncated: String = chars[..break_point].iter().collect();
    format!("{}...", truncated.trim_end())
}

/// Ask LLM to condense a response to fit within max_chars
async fn condense_response(
    client: &openai_api_rs::v1::api::OpenAIClient,
    original: &str,
    max_chars: usize,
    model: &str,
) -> Result<String, String> {
    use openai_api_rs::v1::chat_completion::{ChatCompletionRequest, ChatCompletionMessage, MessageRole, Content};

    let prompt = format!(
        "Condense the following message to fit within {} characters while preserving the key information. \
        Keep it natural and conversational. Do NOT use markdown, bullets, or special formatting. \
        Just output the condensed message, nothing else.\n\nOriginal message:\n{}",
        max_chars, original
    );

    let req = ChatCompletionRequest::new(
        model.to_string(),
        vec![ChatCompletionMessage {
            role: MessageRole::user,
            content: Content::Text(prompt),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }],
    ).max_tokens(300);

    match client.chat_completion(req).await {
        Ok(response) => {
            if let Some(choice) = response.choices.first() {
                if let Some(content) = &choice.message.content {
                    let condensed = content.trim().to_string();
                    // If still too long, truncate nicely
                    if condensed.chars().count() > max_chars {
                        return Ok(truncate_nicely(&condensed, max_chars));
                    }
                    return Ok(condensed);
                }
            }
            Err("No response from condensing".to_string())
        }
        Err(e) => Err(format!("Failed to condense: {}", e))
    }
}

/// Handler for TextBee SMS provider (alternative to Twilio)
pub async fn handle_textbee_sms(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TextBeeWebhookPayload>,
) -> (StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<TwilioResponse>) {
    tracing::debug!("Received TextBee SMS from: {} to: {} via device: {}", payload.sender, payload.recipient, payload.device_id);

    // Step 1: Find user by sender phone (from)
    let user = match state.user_core.find_by_phone_number(&payload.sender) {
        Ok(Some(u)) => u,
        Ok(None) => {
            tracing::error!("No user found for phone: {}", payload.sender);
            return SmsResult::user_not_found().into_response();
        }
        Err(e) => {
            tracing::error!("Error finding user: {}", e);
            return SmsResult::database_error(&e.to_string()).into_response();
        }
    };

    // Step 2: Verify device_id matches user's stored TextBee credentials
    if let Ok((stored_device_id, _api_key)) = state.user_core.get_textbee_credentials(user.id) {
        if payload.device_id != stored_device_id {
            tracing::warn!("Device ID mismatch for user {}: expected {}, got {}", user.id, stored_device_id, payload.device_id);
            return SmsResult::UserError {
                message: "Invalid request source".to_string(),
                status: StatusCode::FORBIDDEN,
            }.into_response();
        }
    } else {
        tracing::error!("No TextBee credentials found for user {}", user.id);
        return SmsResult::UserError {
            message: "No credentials configured".to_string(),
            status: StatusCode::FORBIDDEN,
        }.into_response();
    }

    // Step 3: Map to Twilio payload format
    let twilio_payload = TwilioWebhookPayload {
        from: payload.sender.clone(),
        to: payload.recipient,
        body: payload.body.clone(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
        message_sid: format!("tb_{}", Utc::now().timestamp()),
    };

    // Check for STOP command
    if payload.body.trim().to_uppercase() == "STOP" {
        if let Err(e) = state.user_core.update_notify(user.id, false) {
            tracing::error!("Failed to update notify status: {}", e);
        } else {
            return SmsResult::Success {
                response: "You have been unsubscribed from notifications.".to_string(),
            }.into_response();
        }
    }

    // Process SMS in the background
    tokio::spawn(async move {
        let result = process_sms(&state, twilio_payload, ProcessSmsOptions::default()).await;
        if result.0 != StatusCode::OK {
            tracing::error!("Background SMS processing failed with status: {:?}", result.0);
        }
    });

    // Immediately return a success response
    SmsResult::Success {
        response: "Message received, processing in progress".to_string(),
    }.into_response()
}



/// Handler for the regular SMS endpoint (Twilio webhook)
pub async fn handle_regular_sms(
    State(state): State<Arc<AppState>>,
    Form(payload): Form<TwilioWebhookPayload>,
) -> (StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<TwilioResponse>) {
    tracing::debug!("Received SMS from: {} to: {}", payload.from, payload.to);

    // First check if this user has a discount_tier == msg - they shouldn't be using this endpoint
    match state.user_core.find_by_phone_number(&payload.from) {
        Ok(Some(user)) => {
            if let Some(tier) = user.discount_tier {
                if tier == "msg" {
                    tracing::warn!("User {} with discount_tier=msg attempted to use regular SMS endpoint", user.id);
                    return SmsResult::UserError {
                        message: "Please use your dedicated SMS endpoint. Contact support if you need help.".to_string(),
                        status: StatusCode::FORBIDDEN,
                    }.into_response();
                }
            }
        },
        Ok(None) => {
            tracing::error!("No user found for phone number: {}", payload.from);
            return SmsResult::user_not_found().into_response();
        },
        Err(e) => {
            tracing::error!("Database error while finding user: {}", e);
            return SmsResult::database_error(&e.to_string()).into_response();
        }
    }

    // Check for STOP command
    if payload.body.trim().to_uppercase() == "STOP" {
        if let Ok(Some(user)) = state.user_core.find_by_phone_number(&payload.from) {
            if let Err(e) = state.user_core.update_notify(user.id, false) {
                tracing::error!("Failed to update notify status: {}", e);
            } else {
                return SmsResult::Success {
                    response: "You have been unsubscribed from notifications.".to_string(),
                }.into_response();
            }
        }
    }

    // Process SMS in the background
    tokio::spawn(async move {
        let result = process_sms(&state, payload.clone(), ProcessSmsOptions::default()).await;
        if result.0 != StatusCode::OK {
            tracing::error!("Background SMS processing failed with status: {:?}", result.0);
        }
    });

    // Immediately return a success response to Twilio
    SmsResult::Success {
        response: "Message received, processing in progress".to_string(),
    }.into_response()
}


pub async fn process_sms(
    state: &Arc<AppState>,
    payload: TwilioWebhookPayload,
    options: ProcessSmsOptions,
) -> (StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<TwilioResponse>) {
    let start_time = std::time::Instant::now(); // Track processing time
    let user = match state.user_core.find_by_phone_number(&payload.from) {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::error!("No user found for phone number: {}", payload.from);
            return SmsResult::user_not_found().into_response();
        },
        Err(e) => {
            tracing::error!("Database error while finding user for phone number {}: {}", payload.from, e);
            return SmsResult::database_error(&e.to_string()).into_response();
        }
    };

    // Check if user has sufficient credits before processing the message
    if let Err(e) = crate::utils::usage::check_user_credits(&state, &user, "message", None).await {
        // Distinguish between different error types
        let result = if e.contains("deactivated") {
            tracing::warn!("User {} phone service is deactivated", user.id);
            SmsResult::phone_deactivated()
        } else if e.contains("subscription") {
            tracing::warn!("User {} has no active subscription", user.id);
            SmsResult::no_subscription()
        } else {
            tracing::warn!("User {} has insufficient credits: {}", user.id, e);
            SmsResult::insufficient_credits()
        };
        return result.into_response();
    }
    tracing::info!("Found user with ID: {} for phone number: {}", user.id, payload.from);

    // Handle 'cancel' message specially
    if payload.body.trim().to_lowercase() == "c" {
        match crate::tool_call_utils::utils::cancel_pending_message(state, user.id).await {
            Ok(canceled) => {
                let response_msg = if canceled {
                    "The message got discarded.".to_string()
                } else {
                    "Couldn't find a message to cancel".to_string()
                };

                // Only send actual SMS if not skipping Twilio
                if !options.skip_twilio_send {
                    let state_clone = state.clone();
                    let user_clone = user.clone();
                    let response_msg_clone = response_msg.clone();
                    let start_time_clone = start_time;

                    tokio::spawn(async move {
                        match crate::api::twilio_utils::send_conversation_message(
                            &state_clone,
                            &response_msg_clone,
                            None,
                            &user_clone
                        ).await {
                            Ok(message_sid) => {
                                // Log usage (similar to regular message)
                                let processing_time_secs = start_time_clone.elapsed().as_secs();
                                if let Err(e) = state_clone.user_repository.log_usage(LogUsageParams {
                                    user_id: user_clone.id,
                                    sid: Some(message_sid.clone()),
                                    activity_type: "sms".to_string(),
                                    credits: None,
                                    time_consumed: Some(processing_time_secs as i32),
                                    success: Some(true),
                                    reason: Some("cancel handling".to_string()),
                                    status: None,
                                    recharge_threshold_timestamp: None,
                                    zero_credits_timestamp: None,
                                }) {
                                    tracing::error!("Failed to log SMS usage for cancel: {}", e);
                                }
                                if let Err(e) = crate::utils::usage::deduct_user_credits(&state_clone, user_clone.id, "message", None) {
                                    tracing::error!("Failed to deduct user credits for cancel: {}", e);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to send cancel response message: {}", e);
                                // Log the failed attempt
                                let processing_time_secs = start_time_clone.elapsed().as_secs();
                                let error_status = format!("failed to send: {}", e);
                                if let Err(log_err) = state_clone.user_repository.log_usage(LogUsageParams {
                                    user_id: user_clone.id,
                                    sid: None,
                                    activity_type: "sms".to_string(),
                                    credits: None,
                                    time_consumed: Some(processing_time_secs as i32),
                                    success: Some(false),
                                    reason: Some("cancel handling".to_string()),
                                    status: Some(error_status),
                                    recharge_threshold_timestamp: None,
                                    zero_credits_timestamp: None,
                                }) {
                                    tracing::error!("Failed to log SMS usage after send error for cancel: {}", log_err);
                                }
                            }
                        }
                    });
                }

                return SmsResult::Cancelled { message: response_msg }.into_response();
            }
            Err(e) => {
                tracing::error!("Failed to cancel pending message: {}", e);
                return SmsResult::SystemError {
                    log_msg: format!("Failed to cancel pending message: {}", e),
                }.into_response();
            }
        }
    }
    
    // Log media information for admin user
    if user.id == 1 {
        if let (Some(num_media), Some(media_url), Some(content_type)) = (
            payload.num_media.as_ref(),
            payload.media_url0.as_ref(),
            payload.media_content_type0.as_ref()
        ) {
            tracing::debug!("Media information:");
            tracing::debug!("  Number of media items: {}", num_media);
            tracing::debug!("  Media URL: {}", media_url);
            tracing::debug!("  Content type: {}", content_type);
        }
    }

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Store user's message in history
    let user_message = crate::models::user_models::NewMessageHistory {
        user_id: user.id,
        role: "user".to_string(),
        encrypted_content: payload.body.clone(),
        tool_name: None,
        tool_call_id: None,
        tool_calls_json: None,
        created_at: current_time,
        conversation_id: "".to_string(),
    };

    if let Err(e) = state.user_repository.create_message_history(&user_message) {
        tracing::error!("Failed to store user message in history: {}", e);
    }

    
    // Get user settings to access timezone
    let user_settings = match state.user_core.get_user_settings(user.id) {
        Ok(settings) => settings,
        Err(e) => {
            tracing::error!("Failed to get user settings: {}", e);
            return SmsResult::SystemError {
                log_msg: format!("Failed to get user settings: {}", e),
            }.into_response();
        }
    };

    let user_info= match state.user_core.get_user_info(user.id) {
        Ok(settings) => settings,
        Err(e) => {
            tracing::error!("Failed to get user info: {}", e);
            return SmsResult::SystemError {
                log_msg: format!("Failed to get user info: {}", e),
            }.into_response();
        }
    };

    let user_given_info = match user_info.clone().info {
        Some(info) => info,
        None => "".to_string()
    };

    let timezone_str = match user_info.timezone {
        Some(ref tz) => tz.as_str(),
        None => "UTC",
    };


    // Get timezone offset using jiff
    let (hours, minutes) = match crate::api::elevenlabs::get_offset_with_jiff(timezone_str) {
        Ok((h, m)) => (h, m),
        Err(_) => {
            tracing::error!("Failed to get timezone offset for {}, defaulting to UTC", timezone_str);
            (0, 0) // UTC default
        }
    };

    // Calculate total offset in seconds
    let offset_seconds = hours * 3600 + minutes * 60 * if hours >= 0 { 1 } else { -1 };

    // Create FixedOffset for chrono
    let user_timezone = chrono::FixedOffset::east_opt(offset_seconds)
        .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap()); // Fallback to UTC if invalid

    // Format current time in RFC3339 for the user's timezone
    let formatted_time = Utc::now().with_timezone(&user_timezone).to_rfc3339();

    // Format offset string (e.g., "+02:00" or "-05:30")
    let offset = format!("{}{:02}:{:02}", 
        if hours >= 0 { "+" } else { "-" },
        hours.abs(),
        minutes.abs()
    );

    // Start with the system message
    let mut chat_messages: Vec<ChatMessage> = vec![ChatMessage {
        role: "system".to_string(),
        content: chat_completion::Content::Text(format!("You are a direct and efficient AI assistant named lightfriend. The current date is {}. You must provide extremely concise responses (max 480 characters) while being accurate and helpful. Characters are expensive - be succinct! When listing items (emails, events, tasks), just use numbers and content directly without repeating labels like 'Subject:', 'Event:', etc. Example: 'Emails: 1. Meeting reminder (10am) 2. Invoice from John (9am)' NOT 'Emails: 1. Subject: Meeting reminder (10am) 2. Subject: Invoice from John (9am)'. Since users pay per message, always provide all available information immediately without asking follow-up questions unless confirming details for actions that involve sending information or making changes. Always use all tools immediately that you think will be needed to complete the user's query and base your response to those responses. IMPORTANT: For calendar events, you must return the exact output from the calendar tool without any modifications, additional text, or formatting. Never add bullet points, markdown formatting (like **, -, #), or any other special characters.

### Tool Usage Guidelines:
- Provide all relevant details in the response immediately.
- Tools that involve sending or creating something, add the content to be sent into a queue which are automatically sent after 60 seconds unless user replies 'cancel'.
- Never recommend that the user check apps, websites, or services manually, as they may not have access (e.g., on a dumbphone). Instead, use tools like ask_perplexity to fetch the information yourself.
- When invoking a tool, always output the arguments as a flat JSON object directly matching the tool's parameters (e.g., {{\"query\": \"your value\"}} for ask_perplexity). Do NOT nest arguments inside an \"arguments\" key or any other wrapper—keep it simple and direct.
- CRITICAL: Never make up, guess, or extrapolate information you don't have. If tool results only cover partial data (e.g., weather forecast only until a certain time), clearly state what data you have and what timeframe it covers. Do not invent data beyond what the tools returned.

### Date and Time Handling:
- Always work with times in the user's timezone: {} with offset {}.
- When user mentions times without dates, assume they mean the nearest future occurrence.
- For time inputs to tools, convert to RFC3339 format in UTC (e.g., '2024-03-23T14:30:00Z').
- For displaying times to users:
  - Use 12-hour format with AM/PM (e.g., '2:30 PM')
  - Include timezone-adjusted dates in a friendly format (e.g., 'today', 'tomorrow', or 'Jun 15')
  - Show full date only when it's not today/tomorrow
- If no specific time is mentioned:
  - For calendar queries: Show today's events (and tomorrow's if after 6 PM)
  - For other time ranges: Use current time to 24 hours ahead
- For queries about:
  - 'Today': Use 00:00 to 23:59 of the current day in user's timezone
  - 'Tomorrow': Use 00:00 to 23:59 of tomorrow in user's timezone
  - 'This week': Use remaining days of current week
  - 'Next week': Use Monday to Sunday of next week

Never use markdown, HTML, or any special formatting characters in responses. Return all information in plain text only. User information: {}. Always use tools to fetch the latest information before answering.", formatted_time, timezone_str, offset, user_given_info)),
        tool_calls: None,
        tool_call_id: None,
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
                tracing::debug!("Attempting to delete media with SID: {}", media_sid);
                match crate::api::twilio_utils::delete_twilio_message_media(state, media_sid, &user).await {
                    Ok(_) => tracing::debug!("Successfully deleted media: {}", media_sid),
                    Err(e) => tracing::error!("Failed to delete media {}: {}", media_sid, e),
                }
            }
        }
    }

    // Only include conversation history if message doesn't start with "forget"
    if !payload.body.to_lowercase().starts_with("forget") {

        // Get user's save_context setting
        let save_context = user_settings.save_context.unwrap_or(0);
        
        if save_context > 0 {
            // Get the last N back-and-forth exchanges based on save_context
            let history = state.user_repository
                .get_conversation_history(
                    user.id,
                    save_context as i64,
                    true,
                )
                .unwrap_or_default();

            let mut context_messages: Vec<ChatMessage> = Vec::new();

            // Process messages in chronological order
            for msg in history.into_iter().rev() {
                // Skip assistant messages that triggered tool calls (they have empty content)
                // Including these with structured format causes the model to mimic the pattern
                if msg.role == "assistant" && msg.tool_calls_json.is_some() {
                    continue;
                }

                // For tool messages: format as context that won't be mimicked
                // BUT skip tools that should always be called fresh
                if msg.role == "tool" {
                    const SKIP_FROM_HISTORY: &[&str] = &[
                        "fetch_chat_messages",
                        "fetch_recent_messages",
                        "fetch_emails",
                        "fetch_specific_email",
                        "get_weather",
                        "fetch_calendar_events",
                        "direct_response",
                    ];

                    // Skip these tool results - LLM should call them fresh each time
                    if let Some(ref tool_name) = msg.tool_name {
                        if SKIP_FROM_HISTORY.contains(&tool_name.as_str()) {
                            continue;
                        }
                    }

                    let context_content = format!("[Previous result: {}]", msg.encrypted_content);
                    let chat_msg = ChatMessage {
                        role: "assistant".to_string(),
                        content: chat_completion::Content::Text(context_content),
                        tool_calls: None,
                        tool_call_id: None,
                    };
                    context_messages.push(chat_msg);
                    continue;
                }

                let role = match msg.role.as_str() {
                    "user" => "user",
                    "assistant" => "assistant",
                    _ => continue,
                };

                // Skip messages with empty content (can cause API errors)
                if msg.encrypted_content.trim().is_empty() {
                    continue;
                }

                // Regular messages (user, final assistant responses)
                let chat_msg = ChatMessage {
                    role: role.to_string(),
                    content: chat_completion::Content::Text(msg.encrypted_content.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                };
                context_messages.push(chat_msg);
            }
            
            // Combine system message with conversation history
            chat_messages.extend(context_messages);
        }
    }
    // Get the user's LLM provider preference from settings
    let llm_provider_preference = state.user_core.get_llm_provider(user.id).unwrap_or(None);
    let provider = state.ai_config.provider_for_user_with_preference(llm_provider_preference.as_deref());
    let needs_two_step = state.ai_config.needs_two_step_vision(provider);

    // Handle image if present
    let mut image_url = None;

    if let (Some(num_media), Some(media_url), Some(content_type)) = (
        payload.num_media.as_ref(),
        payload.media_url0.as_ref(),
        payload.media_content_type0.as_ref()
    ) {
        if num_media != "0" && content_type.starts_with("image/") {
            image_url = Some(media_url.clone());

            tracing::debug!("setting image_url var to: {:#?}", image_url);

            // For providers that need two-step vision (e.g., Tinfoil):
            // Step 1: Call vision model to describe image
            // Step 2: Add description as text for tool-calling model
            if needs_two_step {
                tracing::info!("Using two-step vision for {:?} provider (user {})", provider, user.id);

                match state.ai_config.describe_image(provider, media_url, &processed_body).await {
                    Ok(description) => {
                        tracing::info!("Got vision description: {}", &description[..description.len().min(100)]);

                        // Build the message with image description + user's question
                        let combined_content = if processed_body.trim().is_empty() {
                            format!("[Image description: {}]", description)
                        } else {
                            format!("[Image description: {}]\n\nUser's question: {}", description, processed_body)
                        };

                        chat_messages.push(ChatMessage {
                            role: "user".to_string(),
                            content: chat_completion::Content::Text(combined_content),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                    Err(e) => {
                        tracing::error!("Vision description failed, falling back to direct image: {}", e);
                        // Fall back to direct image if vision description fails
                        let mut content_parts = vec![];
                        if !processed_body.trim().is_empty() {
                            content_parts.push(chat_completion::ImageUrl {
                                r#type: chat_completion::ContentType::text,
                                text: Some(processed_body.clone()),
                                image_url: None,
                            });
                        }
                        content_parts.push(chat_completion::ImageUrl {
                            r#type: chat_completion::ContentType::image_url,
                            text: None,
                            image_url: Some(chat_completion::ImageUrlType {
                                url: media_url.clone(),
                            }),
                        });
                        chat_messages.push(ChatMessage {
                            role: "user".to_string(),
                            content: chat_completion::Content::ImageUrl(content_parts),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                }
            } else {
                // Provider supports vision + tools together (e.g., OpenRouter GPT-4o)
                // Build content parts for vision: text (if any) + image
                let mut content_parts = vec![];

                // Add text part first if there's accompanying text
                if !processed_body.trim().is_empty() {
                    content_parts.push(chat_completion::ImageUrl {
                        r#type: chat_completion::ContentType::text,
                        text: Some(processed_body.clone()),
                        image_url: None,
                    });
                }

                // Add image part
                content_parts.push(chat_completion::ImageUrl {
                    r#type: chat_completion::ContentType::image_url,
                    text: None,
                    image_url: Some(chat_completion::ImageUrlType {
                        url: media_url.clone(),
                    }),
                });

                chat_messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: chat_completion::Content::ImageUrl(content_parts),
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
        } else {
            // Add regular text message if no image
            chat_messages.push(ChatMessage {
                role: "user".to_string(),
                content: chat_completion::Content::Text(processed_body),
                tool_calls: None,
                tool_call_id: None,
            });
        }
    } else {
        // Add regular text message if no media
        chat_messages.push(ChatMessage {
            role: "user".to_string(),
            content: chat_completion::Content::Text(processed_body),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    // Define tools
    let tools = vec![
        crate::tool_call_utils::bridge::get_send_chat_message_tool(),
        crate::tool_call_utils::bridge::get_fetch_chat_messages_tool(),
        crate::tool_call_utils::bridge::get_fetch_recent_messages_tool(),
        crate::tool_call_utils::bridge::get_search_chat_contacts_tool(), // idk if we need this
        crate::tool_call_utils::email::get_fetch_emails_tool(),
        crate::tool_call_utils::email::get_fetch_specific_email_tool(),
        crate::tool_call_utils::email::get_send_email_tool(),
        crate::tool_call_utils::email::get_respond_to_email_tool(),
        crate::tool_call_utils::calendar::get_fetch_calendar_event_tool(),
        crate::tool_call_utils::calendar::get_create_calendar_event_tool(),
        crate::tool_call_utils::management::get_create_task_tool(),
        crate::tool_call_utils::management::get_update_monitoring_status_tool(),
        crate::tool_call_utils::internet::get_scan_qr_code_tool(),
        crate::tool_call_utils::internet::get_ask_perplexity_tool(),
        crate::tool_call_utils::internet::get_firecrawl_search_tool(),
        crate::tool_call_utils::internet::get_weather_tool(),
        crate::tool_call_utils::internet::get_directions_tool(),
        crate::tool_call_utils::tesla::get_tesla_control_tool(),
        crate::tool_call_utils::tesla::get_tesla_switch_vehicle_tool(),
        crate::tool_call_utils::internet::get_direct_response_tool(),
    ];

    // Create client for the user's provider
    let client = match create_openai_client_for_user(state, user.id) {
        Ok((client, _)) => client,
        Err(e) => {
            tracing::error!("Failed to create AI client: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Failed to initialize AI service".to_string(),
                })
            );
        }
    };

    // Convert ChatMessage vec into ChatCompletionMessage vec
    let completion_messages: Vec<chat_completion::ChatCompletionMessage> = chat_messages.clone()
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
            tool_calls: msg.tool_calls.clone(),
            tool_call_id: msg.tool_call_id.clone(),
        })
        .collect();


    // Get the model for this provider
    // For two-step vision providers, images were already converted to text,
    // so we always use the Default (tool-calling) model
    let model = get_model(state, provider, ModelPurpose::Default);

    // Use mock response if provided (for testing), otherwise call real LLM
    let result = if let Some(mock_response) = options.mock_llm_response {
        tracing::debug!("Using mock LLM response for testing");
        mock_response
    } else {
        match client.chat_completion(chat_completion::ChatCompletionRequest::new(
                model.clone(),
            completion_messages.clone(),
        )
        .tools(tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .max_tokens(250)).await {
            Ok(result) => result,
            Err(e) => {
                tracing::error!("Failed to get chat completion: {}", e);
                return SmsResult::SystemError {
                    log_msg: format!("Failed to get chat completion: {}", e),
                }.into_response();
            }
        }
    };

    let mut fail = false;
    let mut tool_answers: HashMap<String, String> = HashMap::new(); // tool_call id and answer
    let final_response = match result.choices[0].finish_reason {
        None | Some(chat_completion::FinishReason::stop) => {
            tracing::debug!("Model provided direct response (no tool calls needed)");
            // Direct response from the model
            
            result.choices[0].message.content.clone().unwrap_or_default()
        }
        Some(chat_completion::FinishReason::tool_calls) => {
            tracing::debug!("Model requested tool calls - beginning tool execution phase");

                        
            let tool_calls = match result.choices[0].message.tool_calls.as_ref() {
                Some(calls) => {
                    tracing::debug!("Found {} tool call(s) in response", calls.len());
                    calls
                },
                None => {
                    tracing::error!("No tool calls found in response despite tool_calls finish reason");
                    return SmsResult::SystemError {
                        log_msg: "No tool calls found in response despite tool_calls finish reason".to_string(),
                    }.into_response();
                }
            };

            for tool_call in tool_calls {
                let tool_call_id = tool_call.id.clone();
                tracing::debug!("Processing tool call: {:?} with id: {:?}", tool_call, tool_call_id);
                let name = match &tool_call.function.name {
                    Some(n) => {
                        tracing::debug!("Tool call function name: {}", n);
                        n
                    },
                    None => {
                        log_tool_error(
                            user.id,
                            "unknown",
                            "llm_malformed",
                            "missing_function_name",
                            "Tool call missing function name",
                        );
                        return SmsResult::SystemError {
                            log_msg: "Tool call missing function name".to_string(),
                        }.into_response();
                    },
                };

                // Check if user has access to this tool
                if crate::tool_call_utils::utils::requires_subscription(name, user.sub_tier.clone(), user.discount) {
                    tracing::info!("Attempted to use subscription-only tool {} without proper subscription", name);
                    tool_answers.insert(tool_call_id, format!("This feature ({}) requires a subscription. Please visit our website to subscribe.", name));
                    continue;
                }
                let arguments = match &tool_call.function.arguments {
                    Some(args) => args,
                    None => {
                        log_tool_error(
                            user.id,
                            name,
                            "llm_malformed",
                            "missing_arguments",
                            "Tool call missing arguments",
                        );
                        return SmsResult::SystemError {
                            log_msg: format!("Tool call {} missing arguments", name),
                        }.into_response();
                    }
                };
                if name == "ask_perplexity" {
                    tracing::debug!("Executing ask_perplexity tool call");
                    #[derive(Deserialize, Serialize)]
                    struct PerplexityQuestion {
                        query: String,
                    }

                    let c: PerplexityQuestion = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            log_tool_error(
                                user.id,
                                "ask_perplexity",
                                "llm_malformed",
                                "json_parse_failure",
                                &e.to_string(),
                            );
                            tool_answers.insert(tool_call_id, tool_error_messages::INTERNAL_ERROR.to_string());
                            continue;
                        }
                    };
                    let query = format!("User info: {}. Query: {}", user_given_info, c.query);

                    let sys_prompt = format!("You are assisting an AI text messaging service. The questions you receive are from text messaging conversations where users are seeking information or help. Please note: 1. Provide clear, conversational responses that can be easily read from a small screen 2. Avoid using any markdown, HTML, or other markup languages 3. Keep responses concise but informative 4. When listing multiple points, use simple numbering (1, 2, 3) 5. Focus on the most relevant information that addresses the user's immediate needs. This is what you should know about the user who this information is going to in their own words: {}", user_given_info);
                    match crate::utils::tool_exec::ask_perplexity(state, &query, &sys_prompt).await {
                        Ok(answer) => {
                            tracing::debug!("Successfully received Perplexity answer");
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            log_tool_error(
                                user.id,
                                "ask_perplexity",
                                "external_service",
                                "api_failure",
                                &e.to_string(),
                            );
                            tool_answers.insert(tool_call_id, tool_error_messages::PERPLEXITY_UNAVAILABLE.to_string());
                            continue;
                        }
                    };
                } else if name == "get_weather" {
                    tracing::debug!("Executing get_weather tool call");
                    #[derive(Deserialize, Serialize)]
                    struct WeatherQuestion {
                        location: String,
                        units: String,
                        forecast_type: Option<String>,
                    }
                    let c: WeatherQuestion = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            log_tool_error(
                                user.id,
                                "get_weather",
                                "llm_malformed",
                                "json_parse_failure",
                                &e.to_string(),
                            );
                            tool_answers.insert(tool_call_id, tool_error_messages::INTERNAL_ERROR.to_string());
                            continue;
                        }
                    };
                    let location = c.location;
                    let units = c.units;
                    let forecast_type = c.forecast_type.unwrap_or_else(|| "current".to_string());

                    match crate::utils::tool_exec::get_weather(state, &location, &units, &forecast_type, user.id).await {
                        Ok(answer) => {
                            tracing::debug!("Successfully received weather answer");
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            log_tool_error(
                                user.id,
                                "get_weather",
                                "external_service",
                                "api_failure",
                                &e.to_string(),
                            );
                            tool_answers.insert(tool_call_id, tool_error_messages::WEATHER_UNAVAILABLE.to_string());
                            continue;
                        }
                    };

                } else if name == "search_firecrawl" {
                    tracing::debug!("Executing search_firecrawl tool call");
                    #[derive(Deserialize, Serialize)]
                    struct FireCrawlQuestion {
                        query: String,
                    }
                    let c: FireCrawlQuestion = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            log_tool_error(
                                user.id,
                                "search_firecrawl",
                                "llm_malformed",
                                "json_parse_failure",
                                &e.to_string(),
                            );
                            tool_answers.insert(tool_call_id, tool_error_messages::INTERNAL_ERROR.to_string());
                            continue;
                        }
                    };
                    let query = c.query;
                    match crate::utils::tool_exec::handle_firecrawl_search(query, 5).await {
                        Ok(answer) => {
                            tracing::debug!("Successfully received fire crawl answer");
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            log_tool_error(
                                user.id,
                                "search_firecrawl",
                                "external_service",
                                "api_failure",
                                &e.to_string(),
                            );
                            tool_answers.insert(tool_call_id, tool_error_messages::FIRECRAWL_UNAVAILABLE.to_string());
                            continue;
                        }
                    };
                } else if name == "get_directions" {
                    tracing::debug!("Executing get_directions tool call");
                    #[derive(Deserialize, Serialize)]
                    struct DirectionsQuestion {
                        start_address: String,
                        end_address: String,
                        mode: Option<String>,
                    }
                    let c: DirectionsQuestion = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            log_tool_error(
                                user.id,
                                "get_directions",
                                "llm_malformed",
                                "json_parse_failure",
                                &e.to_string(),
                            );
                            tool_answers.insert(tool_call_id, tool_error_messages::INTERNAL_ERROR.to_string());
                            continue;
                        }
                    };
                    let start_address = c.start_address;
                    let end_address = c.end_address;
                    let mode = c.mode;
                    match crate::tool_call_utils::internet::handle_directions_tool(start_address, end_address, mode).await {
                        Ok(answer) => {
                            tracing::debug!("Successfully received directions answer");
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            log_tool_error(
                                user.id,
                                "get_directions",
                                "external_service",
                                "api_failure",
                                &e.to_string(),
                            );
                            tool_answers.insert(tool_call_id, tool_error_messages::DIRECTIONS_UNAVAILABLE.to_string());
                            continue;
                        }
                    };
                } else if name == "fetch_emails" {
                    tracing::debug!("Executing fetch_emails tool call");
                    let response = crate::tool_call_utils::email::handle_fetch_emails(state, user.id).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "fetch_specific_email" {
                    tracing::debug!("Executing fetch_specific_email tool call");
                    #[derive(Deserialize)]
                    struct EmailQuery {
                        query: String,
                    }
                    
                    let query: EmailQuery = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            log_tool_error(
                                user.id,
                                "fetch_specific_email",
                                "llm_malformed",
                                "json_parse_failure",
                                &e.to_string(),
                            );
                            tool_answers.insert(tool_call_id, tool_error_messages::INTERNAL_ERROR.to_string());
                            continue;
                        }
                    };

                    // First get the email ID
                    let email_id = crate::tool_call_utils::email::handle_fetch_specific_email(state, user.id, &query.query).await;
                    let auth_user = crate::handlers::auth_middleware::AuthUser {
                        user_id: user.id,
                        is_admin: false,
                    };
                    
                    // Then fetch the complete email with that ID
                    match crate::handlers::imap_handlers::fetch_single_imap_email(axum::extract::State(state.clone()), auth_user, axum::extract::Path(email_id)).await {
                        Ok(email) => {
                            let email = &email["email"];
                            
                            // Format the response with all email details
                            let response = format!(
                                "From: {}\nSubject: {}\nDate: {}\n\n{}",
                                email["from"],
                                email["subject"],
                                email["date_formatted"],
                                email["body"]
                            );
                            tool_answers.insert(tool_call_id, response);
                        },
                        Err(_) => {
                            tool_answers.insert(tool_call_id, "Failed to fetch the complete email".to_string());
                        }
                    }
                } else if name == "send_email" {
                    tracing::debug!("Executing send_email tool call");
                    match crate::tool_call_utils::email::handle_send_email(
                        state,
                        user.id,
                        arguments,
                        &user,
                    ).await {
                        Ok((status, headers, Json(twilio_response))) => {
                            let history_entry = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "assistant".to_string(),
                                encrypted_content: twilio_response.message.clone(),
                                tool_name: Some("send_email".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                tool_calls_json: None,
                                created_at: chrono::Utc::now().timestamp() as i32,
                                conversation_id: "".to_string(),
                            };
                            if let Err(e) = state.user_repository.create_message_history(&history_entry) {
                                tracing::error!("Failed to store email tool message in history: {}", e);
                            }
                            // Store the matching "tool" response history before returning
                            let tool_message = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "tool".to_string(),
                                encrypted_content: twilio_response.message.clone(),  // Or "Email sent successfully" if you want a standard msg
                                tool_name: Some("send_email".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                tool_calls_json: None,
                                created_at: current_time,
                                conversation_id: "".to_string(),
                            };
                            if let Err(e) = state.user_repository.create_message_history(&tool_message) {
                                tracing::error!("Failed to store tool response for send_email: {}", e);
                            }
                            return (status, headers, Json(twilio_response));
                        }
                        Err(e) => {
                            tracing::error!("Failed to handle email sending: {}", e);
                            let error_msg = "Failed to send email".to_string();
                            let tool_message = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "tool".to_string(),
                                encrypted_content: error_msg.clone(),
                                tool_name: Some("send_email".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                tool_calls_json: None,
                                created_at: current_time,
                                conversation_id: "".to_string(),
                            };
                            if let Err(store_e) = state.user_repository.create_message_history(&tool_message) {
                                tracing::error!("Failed to store tool error response for send_email: {}", store_e);
                            }
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                [(axum::http::header::CONTENT_TYPE, "application/json")],
                                axum::Json(TwilioResponse {
                                    message: "Failed to process email request".to_string(),
                                })
                            );
                        }
                    }
                } else if name == "respond_to_email" {
                    tracing::debug!("Executing respond_to_email tool call");
                    match crate::tool_call_utils::email::handle_respond_to_email(
                        state,
                        user.id,
                        arguments,
                        &user,
                    ).await {
                        Ok((status, headers, Json(twilio_response))) => {
                            let history_entry = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "assistant".to_string(),
                                encrypted_content: twilio_response.message.clone(),
                                tool_name: Some("respond_to_email".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                tool_calls_json: None,
                                created_at: chrono::Utc::now().timestamp() as i32,
                                conversation_id: "".to_string(),
                            };
                            if let Err(e) = state.user_repository.create_message_history(&history_entry) {
                                tracing::error!("Failed to store respond_to_email tool message in history: {}", e);
                            }
                            // Store the matching "tool" response history before returning
                            let tool_message = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "tool".to_string(),
                                encrypted_content: twilio_response.message.clone(),  // Or "Email sent successfully" if you want a standard msg
                                tool_name: Some("respond_to_email".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                tool_calls_json: None,
                                created_at: current_time+1,
                                conversation_id: "".to_string(),
                            };
                            if let Err(e) = state.user_repository.create_message_history(&tool_message) {
                                tracing::error!("Failed to store tool response for send_email: {}", e);
                            }
                            return (status, headers, Json(twilio_response));
                        }
                        Err(e) => {
                            tracing::error!("Failed to handle respond_to_email: {}", e);
                            // OPTIONAL NEW: Store error as "tool" response for consistency
                            let error_msg = "Failed to send email".to_string();
                            let tool_message = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "tool".to_string(),
                                encrypted_content: error_msg.clone(),
                                tool_name: Some("respond_to_email".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                tool_calls_json: None,
                                created_at: current_time,
                                conversation_id: "".to_string(),
                            };
                            if let Err(store_e) = state.user_repository.create_message_history(&tool_message) {
                                tracing::error!("Failed to store tool error response for send_email: {}", store_e);
                            }
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                [(axum::http::header::CONTENT_TYPE, "application/json")],
                                axum::Json(TwilioResponse {
                                    message: "Failed to process respond_to_email request".to_string(),
                                })
                            );
                        }
                    }
                } else if name == "create_task" {
                    tracing::debug!("Executing create_task tool call");
                    match crate::tool_call_utils::management::handle_create_task(state, user.id, arguments).await {
                        Ok(answer) => {
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            tracing::error!("Failed to create task: {}", e);
                            tool_answers.insert(tool_call_id, format!("Sorry, I couldn't create the task: {}", e));
                        }
                    }
                } else if name == "update_monitoring_status" {
                    tracing::debug!("Executing update_monitoring_status tool call");
                    match crate::tool_call_utils::management::handle_set_proactive_agent(state, user.id, arguments).await {
                        Ok(answer) => {
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            tracing::error!("Failed to toggle monitoring status: {}", e);
                            tool_answers.insert(tool_call_id, "Sorry, I failed to toggle monitoring status. (Contact rasmus@ahtava.com pls:D)".to_string());
                        }
                    }
                } else if name == "create_calendar_event" {
                    tracing::debug!("Executing create_calendar_event tool call");
                    match crate::tool_call_utils::calendar::handle_create_calendar_event(
                        state,
                        user.id,
                        arguments,
                        &user,
                    ).await {
                        Ok((status, headers, Json(twilio_response))) => {
                            let history_entry = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "assistant".to_string(),
                                encrypted_content: twilio_response.message.clone(),
                                tool_name: Some("create_calendar_event".to_string()),
                                tool_call_id: Some(tool_call.id.clone()), 
                                tool_calls_json: None,
                                created_at: chrono::Utc::now().timestamp() as i32,
                                conversation_id: "".to_string(),
                            };

                            if let Err(e) = state.user_repository.create_message_history(&history_entry) {
                                tracing::error!("Failed to store calendar tool message in history: {}", e);
                            }
                            // Store the matching "tool" response history before returning
                            let tool_message = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "tool".to_string(),
                                encrypted_content: twilio_response.message.clone(),  
                                tool_name: Some("create_calendar_event".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                tool_calls_json: None,
                                created_at: current_time,
                                conversation_id: "".to_string(),
                            };
                            if let Err(e) = state.user_repository.create_message_history(&tool_message) {
                                tracing::error!("Failed to store tool response for create_calendar_event: {}", e);
                            }

                            return (status, headers, Json(twilio_response));
                        }
                        Err(e) => {
                            tracing::error!("Failed to handle calendar event creation: {}", e);
                            // Store error as "tool" response for consistency
                            let error_msg = "Failed to create_calendar event".to_string();
                            let tool_message = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "tool".to_string(),
                                encrypted_content: error_msg.clone(),
                                tool_name: Some("create_calendar_event".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                tool_calls_json: None,
                                created_at: current_time,
                                conversation_id: "".to_string(),
                            };
                            if let Err(store_e) = state.user_repository.create_message_history(&tool_message) {
                                tracing::error!("Failed to store tool error response for create_calendar_event: {}", store_e);
                            }
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                [(axum::http::header::CONTENT_TYPE, "application/json")],
                                axum::Json(TwilioResponse {
                                    message: "Failed to process calendar event request".to_string(),
                                })
                            );
                        }
                    }
                } else if name == "search_chat_contacts" {
                    tracing::debug!("Executing search_chat_contacts tool call");
                    let response = crate::tool_call_utils::bridge::handle_search_chat_contacts(
                        state,
                        user.id,
                        arguments,
                    ).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "fetch_recent_messages" {
                    tracing::debug!("Executing fetch_recent_messages tool call");
                    let response = crate::tool_call_utils::bridge::handle_fetch_recent_messages(
                        state,
                        user.id,
                        arguments,
                    ).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "fetch_chat_messages" {
                    tracing::debug!("Executing fetch_chat_messages tool call");
                    let response = crate::tool_call_utils::bridge::handle_fetch_chat_messages(
                        state,
                        user.id,
                        arguments,
                    ).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "send_chat_message" {
                    tracing::debug!("Executing send_chat_message tool call");
                    match crate::tool_call_utils::bridge::handle_send_chat_message(
                        state,
                        user.id,
                        arguments,
                        &user,
                        image_url.as_deref(),
                    ).await {
                        Ok((status, headers, Json(twilio_response))) => {
                            let history_entry = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "assistant".to_string(),
                                encrypted_content: twilio_response.message.clone(),
                                tool_name: Some("send_chat_message".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                tool_calls_json: None,
                                created_at: chrono::Utc::now().timestamp() as i32,
                                conversation_id: "".to_string(),
                            };
                            if let Err(e) = state.user_repository.create_message_history(&history_entry) {
                                tracing::error!("Failed to store send chat message tool message in history: {}", e);
                            }
                            // Store the matching "tool" response history before returning
                            let tool_message = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "tool".to_string(),
                                encrypted_content: twilio_response.message.clone(), 
                                tool_name: Some("send_chat_message".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                tool_calls_json: None,
                                created_at: current_time,
                                conversation_id: "".to_string(),
                            };
                            if let Err(e) = state.user_repository.create_message_history(&tool_message) {
                                tracing::error!("Failed to store tool response for send_chat_message: {}", e);
                            }
                            return (status, headers, Json(twilio_response));
                        }
                        Err(e) => {
                            tracing::error!("Failed to handle chat message sending: {}", e);
                            // Store error as "tool" response for consistency
                            let error_msg = "Failed to send chat message".to_string();
                            let tool_message = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "tool".to_string(),
                                encrypted_content: error_msg.clone(),
                                tool_name: Some("send_chat_message".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                tool_calls_json: None,
                                created_at: current_time,
                                conversation_id: "".to_string(),
                            };
                            if let Err(store_e) = state.user_repository.create_message_history(&tool_message) {
                                tracing::error!("Failed to store tool error response for send_chat_message: {}", store_e);
                            }
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                [(axum::http::header::CONTENT_TYPE, "application/json")],
                                axum::Json(TwilioResponse {
                                    message: "Failed to process chat message request".to_string(),
                                })
                            );
                        }
                    }
                } else if name == "scan_qr_code" {
                    tracing::debug!("Executing scan_qr_code tool call with url: {:#?}", image_url);
                    let response = crate::tool_call_utils::internet::handle_qr_scan(image_url.as_deref()).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "fetch_calendar_events" {
                    tracing::debug!("Executing fetch_calendar_events tool call");
                    let response = crate::tool_call_utils::calendar::handle_fetch_calendar_events(
                        state,
                        user.id,
                        arguments,
                    ).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "control_tesla" {
                    tracing::debug!("Executing control_tesla tool call");
                    let response = crate::tool_call_utils::tesla::handle_tesla_command(
                        state,
                        user.id,
                        arguments,
                        false, // send notification for SMS-initiated commands
                    ).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "switch_selected_tesla_vehicle" {
                    tracing::debug!("Executing switch_selected_tesla_vehicle tool call");
                    let response = crate::tool_call_utils::tesla::handle_tesla_switch_vehicle(
                        state,
                        user.id,
                    ).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "direct_response" {
                    tracing::debug!("Executing direct_response tool call");
                    // Parse the response from the tool call arguments
                    #[derive(Deserialize)]
                    struct DirectResponse {
                        response: String,
                    }
                    if let Ok(args) = serde_json::from_str::<DirectResponse>(arguments) {
                        tool_answers.insert(tool_call_id, args.response);
                    }
                } else {
                    // Unknown tool - this is a system error, not a user error
                    tracing::error!("Unknown tool called: {}", name);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        Json(TwilioResponse {
                            message: format!("Unknown tool: {}", name),
                        }),
                    );
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

            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            // Store tool responses in history
            for (tool_call_id, tool_response) in tool_answers.iter() {
                let tool_message = crate::models::user_models::NewMessageHistory {
                    user_id: user.id,
                    role: "tool".to_string(),
                    encrypted_content: tool_response.clone(),
                    tool_name: None, // We could store this if needed
                    tool_call_id: Some(tool_call_id.clone()),
                    tool_calls_json: None,
                    created_at: current_time+1,
                    conversation_id: "".to_string(),
                };

                if let Err(e) = state.user_repository.create_message_history(&tool_message) {
                    tracing::error!("Failed to store tool response in history: {}", e);
                }
            }


            tracing::debug!("Making follow-up request to model with tool call answers");
            let follow_up_req = chat_completion::ChatCompletionRequest::new(
                model.clone(),
                follow_up_messages,
            )
            .max_tokens(250); // Allow up to ~480 chars for follow-up messages

            match client.chat_completion(follow_up_req).await {
                Ok(follow_up_result) => {
                    tracing::debug!("Received follow-up response from model");
                    let response = follow_up_result.choices[0].message.content.clone().unwrap_or_default();

                    // If we got an empty response, fall back to the tool answer
                    if response.trim().is_empty() {
                        tracing::warn!("Follow-up response was empty, using tool answer directly");
                        tool_answers.values().next()
                            .map(|ans| truncate_nicely(ans, SmsResponse::MAX_LENGTH))
                            .unwrap_or_else(|| "I processed your request but couldn't generate a response.".to_string())
                    } else {
                        response
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to get follow-up completion: {}", e);

                    // Return the tool answer directly, truncated to SMS limit
                    tool_answers.values().next()
                        .map(|ans| truncate_nicely(ans, SmsResponse::MAX_LENGTH))
                        .unwrap_or_else(|| "I apologize, but I encountered an error processing your request. Please try again.".to_string())
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

    // Ensure response is within SMS character limit
    let final_response = if !fail {
        // For successful responses, use LLM condensing if needed
        SmsResponse::new(final_response, &client, &model).await.into_inner()
    } else {
        // For failure messages, just truncate (they're already short)
        SmsResponse::truncated(final_response).into_inner()
    };

    let final_response_with_notice = final_response.clone();

    let processing_time_secs = start_time.elapsed().as_secs(); // Calculate processing time

    // Clean up old message history based on save_context setting
    let save_context = user_settings.save_context.unwrap_or(0);
    if let Err(e) = state.user_repository.delete_old_message_history(
        user.id,
        save_context as i64
    ) {
        tracing::error!("Failed to clean up old message history: {}", e);
    }

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let assistant_message = crate::models::user_models::NewMessageHistory {
        user_id: user.id,
        role: "assistant".to_string(),
        encrypted_content: final_response_with_notice.clone(),
        tool_name: None,
        tool_call_id: None,
        tool_calls_json: None,
        created_at: current_time,
        conversation_id: "".to_string(),
    };

    // Store messages in history
    if let Err(e) = state.user_repository.create_message_history(&assistant_message) {
        tracing::error!("Failed to store assistant message in history: {}", e);
    }

    // If skipping Twilio send (test mode), still deduct credits but skip actual send
    if options.skip_twilio_send {
        // Log the usage without sending the message
        if let Err(e) = state.user_repository.log_usage(LogUsageParams {
            user_id: user.id,
            sid: None,
            activity_type: "sms_test".to_string(),
            credits: None,
            time_consumed: Some(processing_time_secs as i32),
            success: None,
            reason: None,
            status: None,
            recharge_threshold_timestamp: None,
            zero_credits_timestamp: None,
        }) {
            tracing::error!("Failed to log test SMS usage: {}", e);
        }

        // Deduct credits unless caller handles credits themselves
        if !options.skip_credit_deduction {
            if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user.id, "message", None) {
                tracing::error!("Failed to deduct user credits: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(TwilioResponse {
                        message: "Failed to process credits".to_string(),
                    })
                );
            }
        }

        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            axum::Json(TwilioResponse {
                message: final_response_with_notice,
            })
        );
    }

    // Extract filenames from the response and look up their media SIDs
    let mut media_sids = Vec::new();
    let clean_response = final_response_with_notice.lines().filter_map(|line| {
        // Look for lines that contain filenames from the media map
        MEDIA_SID_MAP.with(|map| {
            let map = map.borrow();
            for (filename, media_sid) in map.iter() {
                if line.contains(filename) {
                    media_sids.push(media_sid.clone());
                    return None; // Remove the line containing the filename
                }
            }
            Some(line.to_string())
        })
    }).collect::<Vec<String>>().join("\n");

    let media_sid = media_sids.first();
    let state_clone = state.clone();
    let msg_sid = payload.message_sid.clone();
    let user_clone = user.clone();

    tracing::debug!("going into deleting the incoming message handler");
    tokio::spawn(async move {
        if let Err(e) = crate::api::twilio_utils::delete_twilio_message(&state_clone, &msg_sid, &user_clone).await {
            tracing::error!("Failed to delete incoming message {}: {}", msg_sid, e);
        }
    });

    // Send the actual message if not in test mode
    match crate::api::twilio_utils::send_conversation_message(
        state,
        &clean_response,
        media_sid,
        &user
    ).await {
        Ok(message_sid) => {
            // Log the SMS usage metadata and store message history

            // Log usage
            if let Err(e) = state.user_repository.log_usage(LogUsageParams {
                user_id: user.id,
                sid: Some(message_sid.clone()),
                activity_type: "sms".to_string(),
                credits: None,
                time_consumed: Some(processing_time_secs as i32),
                success: None,
                reason: None,
                status: None,
                recharge_threshold_timestamp: None,
                zero_credits_timestamp: None,
            }) {
                tracing::error!("Failed to log SMS usage: {}", e);
            }

            if let Err(e) = crate::utils::usage::deduct_user_credits(state, user.id, "message", None) {
                tracing::error!("Failed to deduct user credits: {}", e);
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
                        tracing::debug!("User {} credits is under threshold, attempting automatic charge", user.id);
                        // Get user information
                        if user.charge_when_under {
                            use axum::extract::{State, Path};
                            let state_clone = Arc::clone(state);
                            tokio::spawn(async move {
                                let _ = crate::handlers::stripe_handlers::automatic_charge(
                                    State(state_clone),
                                    Path(user.id),
                                ).await;
                                tracing::debug!("Recharged the user successfully back up!");
                            });
                        }
                    }
                },
                Err(e) => tracing::error!("Failed to check if user credits is under threshold: {}", e),
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
            tracing::error!("Failed to send conversation message: {}", e);
            // Log the failed attempt with error message in status
            let error_status = format!("failed to send: {}", e);
            if let Err(log_err) = state.user_repository.log_usage(LogUsageParams {
                user_id: user.id,
                sid: None,
                activity_type: "sms".to_string(),
                credits: None,
                time_consumed: Some(processing_time_secs as i32),
                success: Some(false),
                reason: None,
                status: Some(error_status),
                recharge_threshold_timestamp: None,
                zero_credits_timestamp: None,
            }) {
                tracing::error!("Failed to log SMS usage after send error: {}", log_err);
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

