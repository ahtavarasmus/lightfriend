use crate::context::ContextBuilder;
use crate::repositories::user_repository::LogUsageParams;
use crate::tool_call_utils::utils::ChatMessage;
use crate::AppState;
use crate::UserCoreOps;
use crate::{AiProvider, ModelPurpose};
use axum::{extract::Form, extract::State, http::StatusCode, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

// Thread-local storage for media SID mapping
thread_local! {
    static MEDIA_SID_MAP: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

use openai_api_rs::v1::chat_completion;

/// Error messages for tool call failures - privacy-safe, user-facing
mod tool_error_messages {
    /// Generic error shown to users when a tool fails
    pub const INTERNAL_ERROR: &str =
        "Sorry, we encountered an issue processing your request. Our team has been notified.";
}

/// Log a tool call error without exposing user content (privacy-safe)
fn log_tool_error(
    user_id: i32,
    tool_name: &str,
    category: &str,
    error_type: &str,
    error_msg: &str,
) {
    tracing::error!(
        user_id = user_id,
        tool_name = tool_name,
        error_category = category,
        error_type = error_type,
        "Tool execution failed: {}",
        error_msg
    );
}

// =============================================================================
// SmsResult - Standardized SMS processing outcomes
// =============================================================================

/// The standard response type for SMS processing.
/// This is the tuple returned by process_sms and related functions.
pub type SmsProcessResponse = (
    StatusCode,
    [(axum::http::HeaderName, &'static str); 1],
    axum::Json<TwilioResponse>,
);

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
                axum::Json(TwilioResponse {
                    message: response,
                    created_item_id: None,
                }),
            ),
            SmsResult::UserError { message, status } => (
                status,
                headers,
                axum::Json(TwilioResponse {
                    message,
                    created_item_id: None,
                }),
            ),
            SmsResult::SystemError { log_msg } => {
                tracing::error!("SMS system error: {}", log_msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    headers,
                    axum::Json(TwilioResponse {
                        message: tool_error_messages::INTERNAL_ERROR.to_string(),
                        created_item_id: None,
                    }),
                )
            }
            SmsResult::Cancelled { message } => (
                StatusCode::OK,
                headers,
                axum::Json(TwilioResponse {
                    message,
                    created_item_id: None,
                }),
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
            message:
                "Active subscription required. Please subscribe to continue using the service."
                    .to_string(),
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
    /// ID of item created during this conversation (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_item_id: Option<i32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TextBeeWebhookPayload {
    pub device_id: String, // Required for verification
    pub sender: String,    // Maps to 'from'
    pub recipient: String, // Maps to 'to' (your device's number)
    pub body: String,
}

/// Status updates emitted during process_sms for streaming to clients.
#[derive(Debug, Clone)]
pub enum ChatStatus {
    Thinking,
    ToolCall { name: String },
    Retrying { attempt: u32, max: u32 },
    RetryingFollowup { attempt: u32, max: u32 },
}

/// Options for process_sms to control test behavior
#[derive(Default)]
pub struct ProcessSmsOptions {
    /// Skip actual Twilio SMS sending
    pub skip_twilio_send: bool,
    /// Mock LLM response to use instead of calling real LLM API
    pub mock_llm_response: Option<openai_api_rs::v1::chat_completion::ChatCompletionResponse>,
    /// Optional channel for streaming status updates to callers (e.g. SSE endpoint)
    pub status_tx: Option<tokio::sync::mpsc::Sender<ChatStatus>>,
}

impl ProcessSmsOptions {
    /// Create options for normal production use
    pub fn production() -> Self {
        Self::default()
    }

    /// Create options for web chat (skip Twilio sending)
    pub fn web_chat() -> Self {
        Self {
            skip_twilio_send: true,
            mock_llm_response: None,
            status_tx: None,
        }
    }

    /// Create options for web chat with status streaming
    pub fn web_chat_streaming(tx: tokio::sync::mpsc::Sender<ChatStatus>) -> Self {
        Self {
            skip_twilio_send: true,
            mock_llm_response: None,
            status_tx: Some(tx),
        }
    }

    /// Create options for testing with mock LLM response
    pub fn test_with_mock(
        mock_response: openai_api_rs::v1::chat_completion::ChatCompletionResponse,
    ) -> Self {
        Self {
            skip_twilio_send: true,
            mock_llm_response: Some(mock_response),
            status_tx: None,
        }
    }

    /// Send a status update (no-op if no channel configured)
    fn emit_status(&self, status: ChatStatus) {
        if let Some(tx) = &self.status_tx {
            let _ = tx.try_send(status);
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
        state: &Arc<AppState>,
        provider: crate::AiProvider,
        model: &str,
    ) -> Self {
        let content = if raw.chars().count() > Self::MAX_LENGTH {
            // Try to condense with LLM first, fall back to truncation
            condense_response(state, provider, &raw, Self::MAX_LENGTH, model)
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
        Self {
            content: content.to_string(),
        }
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
    state: &Arc<AppState>,
    provider: crate::AiProvider,
    original: &str,
    max_chars: usize,
    model: &str,
) -> Result<String, String> {
    use openai_api_rs::v1::chat_completion::{
        ChatCompletionMessage, ChatCompletionRequest, Content, MessageRole,
    };

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
    );

    match state.ai_config.chat_completion(provider, &req).await {
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
        Err(e) => Err(format!("Failed to condense: {}", e)),
    }
}

/// Handler for TextBee SMS provider (alternative to Twilio)
pub async fn handle_textbee_sms(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TextBeeWebhookPayload>,
) -> (
    StatusCode,
    [(axum::http::HeaderName, &'static str); 1],
    axum::Json<TwilioResponse>,
) {
    tracing::debug!(
        "Received TextBee SMS from: {} to: {} via device: {}",
        payload.sender,
        payload.recipient,
        payload.device_id
    );

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
            tracing::warn!(
                "Device ID mismatch for user {}: expected {}, got {}",
                user.id,
                stored_device_id,
                payload.device_id
            );
            return SmsResult::UserError {
                message: "Invalid request source".to_string(),
                status: StatusCode::FORBIDDEN,
            }
            .into_response();
        }
    } else {
        tracing::error!("No TextBee credentials found for user {}", user.id);
        return SmsResult::UserError {
            message: "No credentials configured".to_string(),
            status: StatusCode::FORBIDDEN,
        }
        .into_response();
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
            }
            .into_response();
        }
    }

    // Process SMS in the background
    tokio::spawn(async move {
        let result = process_sms(&state, twilio_payload, ProcessSmsOptions::default()).await;
        if result.0 != StatusCode::OK {
            tracing::error!(
                "Background SMS processing failed with status: {:?}",
                result.0
            );
        }
    });

    // Immediately return a success response
    SmsResult::Success {
        response: "Message received, processing in progress".to_string(),
    }
    .into_response()
}

/// Handler for the regular SMS endpoint (Twilio webhook)
pub async fn handle_regular_sms(
    State(state): State<Arc<AppState>>,
    Form(payload): Form<TwilioWebhookPayload>,
) -> (
    StatusCode,
    [(axum::http::HeaderName, &'static str); 1],
    axum::Json<TwilioResponse>,
) {
    tracing::debug!("Received SMS from: {} to: {}", payload.from, payload.to);

    // Check for STOP command
    if payload.body.trim().to_uppercase() == "STOP" {
        if let Ok(Some(user)) = state.user_core.find_by_phone_number(&payload.from) {
            if let Err(e) = state.user_core.update_notify(user.id, false) {
                tracing::error!("Failed to update notify status: {}", e);
            } else {
                return SmsResult::Success {
                    response: "You have been unsubscribed from notifications.".to_string(),
                }
                .into_response();
            }
        }
    }

    // Process SMS in the background
    tokio::spawn(async move {
        let result = process_sms(&state, payload.clone(), ProcessSmsOptions::default()).await;
        if result.0 != StatusCode::OK {
            tracing::error!(
                "Background SMS processing failed with status: {:?}",
                result.0
            );
        }
    });

    // Immediately return a success response to Twilio
    SmsResult::Success {
        response: "Message received, processing in progress".to_string(),
    }
    .into_response()
}

pub async fn process_sms(
    state: &Arc<AppState>,
    payload: TwilioWebhookPayload,
    mut options: ProcessSmsOptions,
) -> (
    StatusCode,
    [(axum::http::HeaderName, &'static str); 1],
    axum::Json<TwilioResponse>,
) {
    let start_time = std::time::Instant::now(); // Track processing time
    let user = match state.user_core.find_by_phone_number(&payload.from) {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::error!("No user found for phone number: {}", payload.from);
            return SmsResult::user_not_found().into_response();
        }
        Err(e) => {
            tracing::error!(
                "Database error while finding user for phone number {}: {}",
                payload.from,
                e
            );
            return SmsResult::database_error(&e.to_string()).into_response();
        }
    };

    // Check if user has sufficient credits before processing the message
    if let Err(e) = crate::utils::usage::check_user_credits(state, &user, "message", None).await {
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
    tracing::info!(
        "Found user with ID: {} for phone number: {}",
        user.id,
        payload.from
    );

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
                        match state_clone
                            .twilio_message_service
                            .send_sms(&response_msg_clone, None, &user_clone)
                            .await
                        {
                            Ok(message_sid) => {
                                // Log usage (similar to regular message)
                                let processing_time_secs = start_time_clone.elapsed().as_secs();
                                if let Err(e) =
                                    state_clone.user_repository.log_usage(LogUsageParams {
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
                                    })
                                {
                                    tracing::error!("Failed to log SMS usage for cancel: {}", e);
                                }
                                // SMS credits deducted at Twilio status callback
                            }
                            Err(e) => {
                                tracing::error!("Failed to send cancel response message: {}", e);
                                // Log the failed attempt
                                let processing_time_secs = start_time_clone.elapsed().as_secs();
                                let error_status = format!("failed to send: {}", e);
                                if let Err(log_err) =
                                    state_clone.user_repository.log_usage(LogUsageParams {
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
                                    })
                                {
                                    tracing::error!(
                                        "Failed to log SMS usage after send error for cancel: {}",
                                        log_err
                                    );
                                }
                            }
                        }
                    });
                }

                return SmsResult::Cancelled {
                    message: response_msg,
                }
                .into_response();
            }
            Err(e) => {
                tracing::error!("Failed to cancel pending message: {}", e);
                return SmsResult::SystemError {
                    log_msg: format!("Failed to cancel pending message: {}", e),
                }
                .into_response();
            }
        }
    }

    // Log media information for admin user
    if user.id == 1 {
        if let (Some(num_media), Some(media_url), Some(content_type)) = (
            payload.num_media.as_ref(),
            payload.media_url0.as_ref(),
            payload.media_content_type0.as_ref(),
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

    // Build agent context (settings, timezone, contacts, LLM client, tools)
    let wants_history = !payload.body.to_lowercase().starts_with("forget");
    let mut builder = ContextBuilder::for_resolved_user(state, user.clone())
        .with_user_context()
        .with_tools()
        .with_mcp_tools();
    if wants_history {
        builder = builder.with_history();
    }
    let ctx = match builder.build().await {
        Ok(ctx) => ctx,
        Err(e) => {
            tracing::error!("Failed to build agent context: {}", e);
            return SmsResult::SystemError {
                log_msg: format!("Failed to build agent context: {}", e),
            }
            .into_response();
        }
    };

    let tz = ctx.timezone.as_ref().unwrap();
    let formatted_time = &tz.formatted_now;
    let timezone_str = &tz.tz_str;
    let offset = &tz.offset_string;
    let contacts_info = ctx.contacts_prompt_fragment.as_deref().unwrap_or("");
    let user_given_info = ctx.user_given_info.as_deref().unwrap_or("");

    // Start with the system message
    let mut chat_messages: Vec<ChatMessage> = vec![ChatMessage {
        role: "system".to_string(),
        content: chat_completion::Content::Text(format!("You are a direct and efficient AI assistant named lightfriend. The current date is {}. You must provide extremely concise responses (max 480 characters) while being accurate and helpful. Characters are expensive - be succinct! When listing items (emails, events, tasks), just use numbers and content directly without repeating labels like 'Subject:', 'Event:', etc. Example: 'Emails: 1. Meeting reminder (10am) 2. Invoice from John (9am)' NOT 'Emails: 1. Subject: Meeting reminder (10am) 2. Subject: Invoice from John (9am)'. Since users pay per message, always provide all available information immediately without asking follow-up questions unless confirming details for actions that involve sending information or making changes. Always use all tools immediately that you think will be needed to complete the user's query and base your response to those responses. IMPORTANT: For calendar events, you must return the exact output from the calendar tool without any modifications, additional text, or formatting. Never add bullet points, markdown formatting (like **, -, #), or any other special characters.{contacts_info}

### Tool Usage Guidelines:
- Provide all relevant details in the response immediately.
- Tools that involve sending or creating something, add the content to be sent into a queue which are automatically sent after 60 seconds unless user replies 'cancel'.
- Never recommend that the user check apps, websites, or services manually, as they may not have access (e.g., on a dumbphone). Instead, use tools like ask_perplexity to fetch the information yourself.
- When invoking a tool, always output the arguments as a flat JSON object directly matching the tool's parameters (e.g., {{\"query\": \"your value\"}} for ask_perplexity). Do NOT nest arguments inside an \"arguments\" key or any other wrapper—keep it simple and direct.
- CRITICAL: Never make up, guess, or extrapolate information you don't have. If tool results only cover partial data (e.g., weather forecast only until a certain time), clearly state what data you have and what timeframe it covers. Do not invent data beyond what the tools returned.

### Questions vs Items:
- If the user is ASKING A QUESTION about an event or topic (e.g. 'I have a dentist appointment tomorrow, what should I bring?', 'what's the weather like this weekend?'), answer the question directly using tools like ask_perplexity. Do NOT create an item.
- Only create an item when the user explicitly requests a reminder, scheduled action, or monitoring (e.g. 'remind me', 'text me at', 'notify me when', 'at 5pm do X').

### Event Reminder Timing:
- When the user mentions an event at a specific time, set next_check_at BEFORE the event so the reminder is useful:
  - 'meeting at 2pm' -> next_check_at 1:55 PM (5 min before - same-location event)
  - 'dinner at restaurant at 7pm' -> next_check_at 6:00 PM (60 min before - travel needed)
  - 'doctor appointment at 3pm' -> next_check_at 2:15 PM (45 min before - appointment)
- If the user gives an explicit reminder time, use it exactly: 'remind me at 1pm about my 2pm meeting' -> next_check_at 1:00 PM
- Always include the actual event time in the reminder message so the user knows when it starts.

### Date and Time Handling:
- Always work with times in the user's timezone: {} with offset {}.
- When user mentions times without dates, assume they mean the nearest future occurrence.
- RELATIVE TIMES: For 'in X hours/minutes', compute the exact next_check_at by adding X to the current time. Example: if current time is 14:30 and user says 'in 3 hours', next_check_at must be 17:30 (not an approximation).
- For displaying times to users:
  - Use 12-hour format with AM/PM (e.g., '2:30 PM')
  - Include timezone-adjusted dates in a friendly format (e.g., 'today', 'tomorrow', or 'Jun 15')
  - Show full date only when it's not today/tomorrow
- If no specific time is mentioned, use current time to 24 hours ahead.
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
        payload
            .body
            .trim_start_matches(|c: char| c.is_alphabetic())
            .trim()
            .to_string()
    } else {
        payload.body.clone()
    };

    // Delete media if present after processing
    if let (Some(num_media), Some(media_url), Some(_)) = (
        payload.num_media.as_ref(),
        payload.media_url0.as_ref(),
        payload.media_content_type0.as_ref(),
    ) {
        if num_media != "0" {
            // Extract message SID and media SID from URL
            // URL format: .../Messages/{message_sid}/Media/{media_sid}
            if let (Some(msg_part), Some(media_sid)) = (
                media_url.split("/Messages/").nth(1),
                media_url.split("/Media/").nth(1),
            ) {
                if let Some(message_sid) = msg_part.split("/Media/").next() {
                    tracing::debug!(
                        "Attempting to delete media {} from message {}",
                        media_sid,
                        message_sid
                    );
                    match state
                        .twilio_message_service
                        .delete_message_media(&user, message_sid, media_sid)
                        .await
                    {
                        Ok(_) => tracing::debug!("Successfully deleted media: {}", media_sid),
                        Err(e) => tracing::error!("Failed to delete media {}: {}", media_sid, e),
                    }
                }
            }
        }
    }

    // Add conversation history from context builder (already filtered and converted)
    if let Some(ref history) = ctx.conversation_history {
        for msg in history {
            let role = match msg.role {
                chat_completion::MessageRole::user => "user",
                chat_completion::MessageRole::assistant => "assistant",
                chat_completion::MessageRole::system => "system",
                _ => "user",
            };
            chat_messages.push(ChatMessage {
                role: role.to_string(),
                content: msg.content.clone(),
                tool_calls: None,
                tool_call_id: None,
            });
        }
    }

    let provider = ctx.provider;
    let needs_two_step = state.ai_config.needs_two_step_vision(provider);

    // Handle image if present
    let mut image_url = None;

    if let (Some(num_media), Some(media_url), Some(content_type)) = (
        payload.num_media.as_ref(),
        payload.media_url0.as_ref(),
        payload.media_content_type0.as_ref(),
    ) {
        if num_media != "0" && content_type.starts_with("image/") {
            image_url = Some(media_url.clone());

            tracing::debug!("setting image_url var to: {:#?}", image_url);

            // For providers that need two-step vision (e.g., Tinfoil):
            // Step 1: Call vision model to describe image
            // Step 2: Add description as text for tool-calling model
            if needs_two_step {
                tracing::info!(
                    "Using two-step vision for {:?} provider (user {})",
                    provider,
                    user.id
                );

                match state
                    .ai_config
                    .describe_image(provider, media_url, &processed_body)
                    .await
                {
                    Ok(description) => {
                        tracing::info!(
                            "Got vision description: {}",
                            &description[..description.len().min(100)]
                        );

                        // Build the message with image description + user's question
                        let combined_content = if processed_body.trim().is_empty() {
                            format!("[Image description: {}]", description)
                        } else {
                            format!(
                                "[Image description: {}]\n\nUser's question: {}",
                                description, processed_body
                            )
                        };

                        chat_messages.push(ChatMessage {
                            role: "user".to_string(),
                            content: chat_completion::Content::Text(combined_content),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                    Err(e) => {
                        tracing::error!(
                            "Vision description failed, falling back to direct image: {}",
                            e
                        );
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

    // Tools and model from context builder
    let tools = ctx.tools.unwrap_or_default();
    let model = ctx.model.clone();

    // Convert ChatMessage vec into ChatCompletionMessage vec
    let completion_messages: Vec<chat_completion::ChatCompletionMessage> = chat_messages
        .clone()
        .into_iter()
        .map(|msg| chat_completion::ChatCompletionMessage {
            role: match msg.role.as_str() {
                "user" => chat_completion::MessageRole::user,
                "assistant" => chat_completion::MessageRole::assistant,
                "system" => chat_completion::MessageRole::system,
                _ => chat_completion::MessageRole::user,
            },
            content: msg.content.clone(),
            name: None,
            tool_calls: msg.tool_calls.clone(),
            tool_call_id: msg.tool_call_id.clone(),
        })
        .collect();

    // Use mock response if provided (for testing), otherwise call real LLM with retry
    let result = if let Some(mock_response) = options.mock_llm_response.take() {
        tracing::debug!("Using mock LLM response for testing");
        mock_response
    } else {
        options.emit_status(ChatStatus::Thinking);
        const MAX_RETRIES: u32 = 3;
        let mut last_error = String::new();
        let mut attempt_result = None;

        for attempt in 1..=MAX_RETRIES {
            let request = chat_completion::ChatCompletionRequest::new(
                model.clone(),
                completion_messages.clone(),
            )
            .tools(tools.clone())
            .tool_choice(chat_completion::ToolChoiceType::Required);

            match ctx.client.chat_completion(request).await {
                Ok(result) => {
                    attempt_result = Some(result);
                    break;
                }
                Err(e) => {
                    last_error = format!("{:?}", e);
                    tracing::warn!(
                        "Chat completion attempt {}/{} failed: {:?}",
                        attempt,
                        MAX_RETRIES,
                        e
                    );
                    if attempt < MAX_RETRIES {
                        options.emit_status(ChatStatus::Retrying {
                            attempt: attempt + 1,
                            max: MAX_RETRIES,
                        });
                        // Wait before retry: 500ms, 1000ms
                        tokio::time::sleep(tokio::time::Duration::from_millis(
                            500 * attempt as u64,
                        ))
                        .await;
                    }
                }
            }
        }

        match attempt_result {
            Some(result) => result,
            None => {
                tracing::error!(
                    "Failed to get chat completion after {} attempts: {}",
                    MAX_RETRIES,
                    last_error
                );
                return SmsResult::SystemError {
                    log_msg: format!("Failed to get chat completion: {}", last_error),
                }
                .into_response();
            }
        }
    };

    let mut fail = false;
    let mut tool_answers: HashMap<String, String> = HashMap::new(); // tool_call id and answer
    let mut created_item_id: Option<i32> = None; // Track if an item was created during this conversation
    let final_response = match result.choices[0].finish_reason {
        None | Some(chat_completion::FinishReason::stop) => {
            tracing::debug!("Model provided direct response (no tool calls needed)");
            // Direct response from the model

            result.choices[0]
                .message
                .content
                .clone()
                .unwrap_or_default()
        }
        Some(chat_completion::FinishReason::tool_calls) => {
            tracing::debug!("Model requested tool calls - beginning tool execution phase");

            let tool_calls = match result.choices[0].message.tool_calls.as_ref() {
                Some(calls) => {
                    tracing::debug!("Found {} tool call(s) in response", calls.len());
                    calls
                }
                None => {
                    tracing::error!(
                        "No tool calls found in response despite tool_calls finish reason"
                    );
                    return SmsResult::SystemError {
                        log_msg: "No tool calls found in response despite tool_calls finish reason"
                            .to_string(),
                    }
                    .into_response();
                }
            };

            for tool_call in tool_calls {
                let tool_call_id = tool_call.id.clone();
                tracing::debug!(
                    "Processing tool call: {:?} with id: {:?}",
                    tool_call,
                    tool_call_id
                );
                let name = match &tool_call.function.name {
                    Some(n) => {
                        tracing::debug!("Tool call function name: {}", n);
                        options.emit_status(ChatStatus::ToolCall { name: n.clone() });
                        n
                    }
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
                        }
                        .into_response();
                    }
                };

                // Check if user has access to this tool
                if crate::tool_call_utils::utils::requires_subscription(
                    name,
                    user.sub_tier.clone(),
                    user.discount,
                ) {
                    tracing::info!(
                        "Attempted to use subscription-only tool {} without proper subscription",
                        name
                    );
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
                        }
                        .into_response();
                    }
                };

                // Handle MCP tool calls (dynamic, per-user)
                if crate::tool_call_utils::mcp::is_mcp_tool(name) {
                    tracing::debug!("Executing MCP tool call: {}", name);
                    let result = crate::tool_call_utils::mcp::handle_mcp_tool_call(
                        state, user.id, name, arguments,
                    )
                    .await;
                    tool_answers.insert(tool_call_id, result);
                    continue;
                }

                // Registry dispatch for static tools
                let ctx = crate::tools::registry::ToolContext {
                    state,
                    user: &user,
                    user_id: user.id,
                    arguments,
                    image_url: image_url.as_deref(),
                    tool_call_id: tool_call_id.clone(),
                    user_given_info,
                    current_time,
                    // Fields for create_task retry logic
                    client: Some(&ctx.client),
                    model: Some(&model),
                    tools: Some(&tools),
                    completion_messages: Some(&completion_messages),
                    assistant_content: result.choices[0].message.content.as_deref(),
                    tool_call: Some(tool_call),
                };

                match state.tool_registry.get(name) {
                    Some(handler) => match handler.execute(ctx).await {
                        Ok(crate::tools::registry::ToolResult::Answer(answer)) => {
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Ok(crate::tools::registry::ToolResult::AnswerWithTask {
                            answer,
                            task_id,
                        }) => {
                            created_item_id = Some(task_id);
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Ok(crate::tools::registry::ToolResult::EarlyReturn {
                            response,
                            status,
                        }) => {
                            let headers = [(axum::http::header::CONTENT_TYPE, "application/json")];
                            return (status, headers, Json(response));
                        }
                        Err(e) => {
                            log_tool_error(user.id, name, "execution", "handler_error", &e);
                            tool_answers.insert(
                                tool_call_id,
                                tool_error_messages::INTERNAL_ERROR.to_string(),
                            );
                        }
                    },
                    None => {
                        tracing::error!("Unknown tool called: {}", name);
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            Json(TwilioResponse {
                                message: format!("Unknown tool: {}", name),
                                created_item_id: None,
                            }),
                        );
                    }
                }
            }

            let mut follow_up_messages = completion_messages.clone();
            // Add the assistant's message with tool calls
            follow_up_messages.push(chat_completion::ChatCompletionMessage {
                role: chat_completion::MessageRole::assistant,
                content: chat_completion::Content::Text(
                    result.choices[0]
                        .message
                        .content
                        .clone()
                        .unwrap_or_default(),
                ),
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
                    created_at: current_time + 1,
                    conversation_id: "".to_string(),
                };

                if let Err(e) = state.user_repository.create_message_history(&tool_message) {
                    tracing::error!("Failed to store tool response in history: {}", e);
                }
            }

            tracing::debug!("Making follow-up request to model with tool call answers");
            options.emit_status(ChatStatus::Thinking);

            // Retry logic for follow-up call
            const FOLLOWUP_MAX_RETRIES: u32 = 3;
            let mut followup_last_error = String::new();
            let mut followup_result = None;

            for attempt in 1..=FOLLOWUP_MAX_RETRIES {
                let follow_up_req = chat_completion::ChatCompletionRequest::new(
                    model.clone(),
                    follow_up_messages.clone(),
                );

                match ctx.client.chat_completion(follow_up_req).await {
                    Ok(result) => {
                        followup_result = Some(result);
                        break;
                    }
                    Err(e) => {
                        followup_last_error = format!("{}", e);
                        tracing::warn!(
                            "Follow-up completion attempt {}/{} failed: {}",
                            attempt,
                            FOLLOWUP_MAX_RETRIES,
                            e
                        );
                        if attempt < FOLLOWUP_MAX_RETRIES {
                            options.emit_status(ChatStatus::RetryingFollowup {
                                attempt: attempt + 1,
                                max: FOLLOWUP_MAX_RETRIES,
                            });
                            tokio::time::sleep(tokio::time::Duration::from_millis(
                                500 * attempt as u64,
                            ))
                            .await;
                        }
                    }
                }
            }

            match followup_result {
                Some(follow_up_result) => {
                    tracing::debug!("Received follow-up response from model");
                    let response = follow_up_result.choices[0]
                        .message
                        .content
                        .clone()
                        .unwrap_or_default();

                    // If we got an empty response, fall back to the tool answer
                    if response.trim().is_empty() {
                        tracing::warn!("Follow-up response was empty, using tool answer directly");
                        // Check if direct_response - don't return internal hint
                        let was_direct_response = result.choices[0]
                            .message
                            .tool_calls
                            .as_ref()
                            .map(|calls| {
                                calls
                                    .iter()
                                    .any(|c| c.function.name.as_deref() == Some("direct_response"))
                            })
                            .unwrap_or(false);

                        if was_direct_response {
                            "I processed your request but couldn't generate a response.".to_string()
                        } else {
                            tool_answers
                                .values()
                                .next()
                                .map(|ans| truncate_nicely(ans, SmsResponse::MAX_LENGTH))
                                .unwrap_or_else(|| {
                                    "I processed your request but couldn't generate a response."
                                        .to_string()
                                })
                        }
                    } else {
                        response
                    }
                }
                None => {
                    tracing::error!(
                        "Failed to get follow-up completion after {} attempts: {}",
                        FOLLOWUP_MAX_RETRIES,
                        followup_last_error
                    );

                    // Check if this was a direct_response tool - don't return the internal hint
                    let was_direct_response = result.choices[0]
                        .message
                        .tool_calls
                        .as_ref()
                        .map(|calls| {
                            calls
                                .iter()
                                .any(|c| c.function.name.as_deref() == Some("direct_response"))
                        })
                        .unwrap_or(false);

                    if was_direct_response {
                        "I apologize, but I encountered an error. Please try again.".to_string()
                    } else {
                        // Return the tool answer directly, truncated to SMS limit
                        tool_answers
                            .values()
                            .next()
                            .map(|ans| truncate_nicely(ans, SmsResponse::MAX_LENGTH))
                            .unwrap_or_else(|| {
                                "I apologize, but I encountered an error processing your request. Please try again.".to_string()
                            })
                    }
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

    // Extract any [MEDIA_RESULTS] from tool answers and append to response
    // This ensures media results are passed through even if AI doesn't include them
    let mut media_results_tag = String::new();
    for tool_answer in tool_answers.values() {
        tracing::debug!(
            "Checking tool answer for media (first 200 chars): {}",
            &tool_answer.chars().take(200).collect::<String>()
        );
        if let Some(start) = tool_answer.find("[MEDIA_RESULTS]") {
            if let Some(end) = tool_answer.find("[/MEDIA_RESULTS]") {
                media_results_tag = tool_answer[start..end + 16].to_string();
                tracing::debug!(
                    "Found media results tag, length: {}",
                    media_results_tag.len()
                );
                break;
            }
        }
    }

    // Ensure response is within SMS character limit (truncate text BEFORE adding media)
    let final_response = if !fail {
        // For successful responses, use LLM condensing if needed
        SmsResponse::new(final_response, state, provider, &model)
            .await
            .into_inner()
    } else {
        // For failure messages, just truncate (they're already short)
        SmsResponse::truncated(final_response).into_inner()
    };

    // Append media results AFTER truncating (so they don't get cut off)
    // This ensures the [MEDIA_RESULTS] JSON is always complete for web chat parsing
    let final_response = if !media_results_tag.is_empty() {
        tracing::debug!("Appending media results to final response (after truncation)");
        format!("{}\n\n{}", final_response, media_results_tag)
    } else {
        tracing::debug!("No media results tag found in tool answers");
        final_response
    };

    let final_response_with_notice = final_response.clone();

    let processing_time_secs = start_time.elapsed().as_secs(); // Calculate processing time

    // Clean up old message history based on save_context setting
    let save_context = ctx
        .user_settings
        .as_ref()
        .and_then(|s| s.save_context)
        .unwrap_or(0);
    if let Err(e) = state
        .user_repository
        .delete_old_message_history(user.id, save_context as i64)
    {
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
    if let Err(e) = state
        .user_repository
        .create_message_history(&assistant_message)
    {
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

        // SMS credits deducted at Twilio status callback

        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            axum::Json(TwilioResponse {
                message: final_response_with_notice,
                created_item_id,
            }),
        );
    }

    // Extract filenames from the response and look up their media SIDs
    let mut media_sids = Vec::new();
    let clean_response = final_response_with_notice
        .lines()
        .filter_map(|line| {
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
        })
        .collect::<Vec<String>>()
        .join("\n");

    let media_sid = media_sids.first();
    let state_clone = state.clone();
    let msg_sid = payload.message_sid.clone();
    let user_clone = user.clone();

    tracing::debug!("going into deleting the incoming message handler");
    tokio::spawn(async move {
        if let Err(e) = state_clone
            .twilio_message_service
            .delete_message_with_retry(&user_clone, &msg_sid)
            .await
        {
            tracing::error!("Failed to delete incoming message {}: {}", msg_sid, e);
        }
    });

    // Send the actual message if not in test mode
    match state
        .twilio_message_service
        .send_sms(&clean_response, media_sid, &user)
        .await
    {
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

            // SMS credits deducted at Twilio status callback

            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Message sent successfully".to_string(),
                    created_item_id: None,
                }),
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
                    created_item_id: None,
                }),
            )
        }
    }
}
