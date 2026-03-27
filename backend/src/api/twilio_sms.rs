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

/// LLM call with retry logic, used by the agentic loop.
#[allow(clippy::too_many_arguments)]
async fn llm_call_with_retry(
    state: &Arc<AppState>,
    provider: AiProvider,
    model: &str,
    messages: &[chat_completion::ChatCompletionMessage],
    tools: &[chat_completion::Tool],
    reasoning_tx: &Option<tokio::sync::mpsc::Sender<String>>,
    options: &mut ProcessSmsOptions,
    max_retries: u32,
) -> Result<chat_completion::ChatCompletionResponse, String> {
    let mut last_error = String::new();

    for attempt in 1..=max_retries {
        let request =
            chat_completion::ChatCompletionRequest::new(model.to_string(), messages.to_vec())
                .tools(tools.to_vec())
                .tool_choice(chat_completion::ToolChoiceType::Auto);

        match state
            .ai_config
            .chat_completion_streaming(provider, &request, reasoning_tx.clone())
            .await
        {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = format!("{:?}", e);
                tracing::warn!(
                    "Chat completion attempt {}/{} failed: {:?}",
                    attempt,
                    max_retries,
                    e
                );
                if attempt < max_retries {
                    options.emit_status(ChatStatus::Retrying {
                        attempt: attempt + 1,
                        max: max_retries,
                    });
                    tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt as u64))
                        .await;
                }
            }
        }
    }

    tracing::error!(
        "Failed to get chat completion after {} attempts: {}",
        max_retries,
        last_error
    );
    Err(format!("Failed to get chat completion: {}", last_error))
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
    Reasoning { snippet: String },
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
    /// Mock tool responses: when a tool name matches a key, return the value instead of executing.
    pub mock_tool_responses: Option<std::collections::HashMap<String, String>>,
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
            mock_tool_responses: None,
        }
    }

    /// Create options for web chat with status streaming
    pub fn web_chat_streaming(tx: tokio::sync::mpsc::Sender<ChatStatus>) -> Self {
        Self {
            skip_twilio_send: true,
            mock_llm_response: None,
            status_tx: Some(tx),
            mock_tool_responses: None,
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
            mock_tool_responses: None,
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

    // Step 2: Map to Twilio payload format
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
    let user_message = crate::pg_models::NewPgMessageHistory {
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
    let events_info = ctx.events_prompt_fragment.as_deref().unwrap_or("");
    let user_given_info = ctx.user_given_info.as_deref().unwrap_or("");
    let cancel_hint = if options.skip_twilio_send {
        ""
    } else {
        " User can reply 'cancel' to stop."
    };

    // Start with the system message
    let mut chat_messages: Vec<ChatMessage> = vec![ChatMessage {
        role: "system".to_string(),
        content: chat_completion::Content::Text(format!("You are lightfriend, a concise AI assistant. Current date: {}. Max 480 characters per response. Characters are expensive - be succinct! ALWAYS list multiple items one per line, never in a paragraph. Example:\n1. Dad (WhatsApp) - Pick up car\n2. Lisa (Signal) - Review contract Provide all information immediately; only ask follow-ups when confirming send/create actions. Call all needed tools upfront.{contacts_info}{events_info}

### Tool Usage:
- Always use tools to fetch current data. Never answer data questions from conversation history alone - the history may be days old.
- Use tools to fetch information directly (users may only have a dumbphone).
- Send/create tools queue content for 60 seconds.{cancel_hint}
- State only what tool results returned. Note any gaps in data coverage.

### Behavior:
- 'remind me', 'notify me at X', 'wake me at Y' -> use set_reminder immediately. Never answer these directly.
- Recurring reminders ('daily at 9am remind me to X', 'every weekday at 8am') -> use set_reminder with a recurring pattern.
- Complex recurring schedules with AI logic or tool actions (daily email briefings, check conditions) -> use create_rule with recurring schedule + llm logic.
- Tracking conditions (notify me when X emails me, alert me if Y happens) -> use create_rule with ontology_change trigger.
- When in doubt between set_reminder and create_rule: if it's just a notification with a message, use set_reminder. If it needs LLM evaluation, data fetching, or tool execution, use create_rule.

### Date and Time:
- User timezone: {} with offset {}. Nearest future occurrence for ambiguous times.
- Relative times: compute exactly (14:30 + 'in 3 hours' = 17:30).
- Display: 12-hour AM/PM, 'today'/'tomorrow'/date. Default range: now to 24h ahead.
- 'Today' = 00:00-23:59 today. 'Tomorrow' = 00:00-23:59 tomorrow. 'This week' = remaining days. 'Next week' = Mon-Sun.

NEVER use emojis - they cost extra in SMS encoding. Respond in plain text only. User information: {}. Use tools to fetch latest information before answering.", formatted_time, timezone_str, offset, user_given_info)),
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

            // Build content parts for vision: text (if any) + image
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
    let mut tools = ctx.tools.unwrap_or_default();
    let model = ctx.model.clone();

    // Inject ontology query tools (dynamically built with user-specific enum values)
    let ontology_user_data = {
        let mut dynamic_enums = std::collections::HashMap::new();
        if let Ok(persons) = state.ontology_repository.get_persons(user.id) {
            let names: Vec<String> = persons.iter().map(|p| p.name.clone()).collect();
            dynamic_enums.insert("person_names".to_string(), names);
        }
        crate::ontology::registry::OntologyUserData { dynamic_enums }
    };
    tools.extend(
        state
            .ontology_registry
            .build_query_tools(&ontology_user_data),
    );

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

    // Bridge channel: forward raw reasoning strings as ChatStatus::Reasoning events.
    // Only created for the web chat SSE path (when status_tx exists).
    let reasoning_tx: Option<tokio::sync::mpsc::Sender<String>> =
        if let Some(ref status_tx) = options.status_tx {
            let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(32);
            let status_tx = status_tx.clone();
            tokio::spawn(async move {
                while let Some(snippet) = rx.recv().await {
                    let _ = status_tx.send(ChatStatus::Reasoning { snippet }).await;
                }
            });
            Some(tx)
        } else {
            None
        };

    // Terminal tools: produce final response, break the loop without going back to LLM
    let terminal_tools = ["create_rule", "set_reminder"];

    let mut fail = false;
    let mut tool_answers: HashMap<String, String> = HashMap::new();
    let mut created_item_id: Option<i32> = None;
    let mut loop_messages = completion_messages.clone();

    const MAX_ROUNDS: u32 = 5;
    const MAX_RETRIES: u32 = 3;

    let mut final_response = String::new();

    'agentic: for round in 0..MAX_ROUNDS {
        tracing::debug!("Agentic loop round {}/{}", round + 1, MAX_ROUNDS);

        // Use mock response if provided (for testing, first round only)
        let result = if round == 0 {
            if let Some(mock_response) = options.mock_llm_response.take() {
                tracing::debug!("Using mock LLM response for testing");
                mock_response
            } else {
                options.emit_status(ChatStatus::Thinking);
                match llm_call_with_retry(
                    state,
                    ctx.provider,
                    &model,
                    &loop_messages,
                    &tools,
                    &reasoning_tx,
                    &mut options,
                    MAX_RETRIES,
                )
                .await
                {
                    Ok(r) => r,
                    Err(log_msg) => {
                        return SmsResult::SystemError { log_msg }.into_response();
                    }
                }
            }
        } else {
            options.emit_status(ChatStatus::Thinking);
            match llm_call_with_retry(
                state,
                ctx.provider,
                &model,
                &loop_messages,
                &tools,
                &reasoning_tx,
                &mut options,
                MAX_RETRIES,
            )
            .await
            {
                Ok(r) => r,
                Err(log_msg) => {
                    return SmsResult::SystemError { log_msg }.into_response();
                }
            }
        };

        match result.choices[0].finish_reason {
            None | Some(chat_completion::FinishReason::stop) => {
                tracing::debug!("Model provided direct response (no tool calls)");
                final_response = result.choices[0]
                    .message
                    .content
                    .clone()
                    .unwrap_or_default();
                break 'agentic;
            }
            Some(chat_completion::FinishReason::length) => {
                fail = true;
                final_response = "I apologize, but my response was too long. Could you please ask your question in a more specific way? (you were not charged for this message)".to_string();
                break 'agentic;
            }
            Some(chat_completion::FinishReason::content_filter) => {
                fail = true;
                final_response = "I apologize, but I cannot provide an answer to that question due to content restrictions. (you were not charged for this message)".to_string();
                break 'agentic;
            }
            Some(chat_completion::FinishReason::null) => {
                fail = true;
                final_response = "I apologize, but something went wrong while processing your request. (you were not charged for this message)".to_string();
                break 'agentic;
            }
            Some(chat_completion::FinishReason::tool_calls) => {
                tracing::debug!("Model requested tool calls (round {})", round + 1);

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
                            log_msg:
                                "No tool calls found in response despite tool_calls finish reason"
                                    .to_string(),
                        }
                        .into_response();
                    }
                };

                // Add assistant message with tool calls to loop messages
                loop_messages.push(chat_completion::ChatCompletionMessage {
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

                let mut round_answers: HashMap<String, String> = HashMap::new();

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

                    // Check subscription access
                    if crate::tool_call_utils::utils::requires_subscription(
                        name,
                        user.sub_tier.clone(),
                    ) {
                        tracing::info!(
                            "Attempted to use subscription-only tool {} without proper subscription",
                            name
                        );
                        round_answers.insert(tool_call_id, format!("This feature ({}) requires a subscription. Please visit our website to subscribe.", name));
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

                    // Mock tool responses for testing
                    if let Some(ref mock_map) = options.mock_tool_responses {
                        if let Some(mock_result) = mock_map.get(name) {
                            tracing::debug!("Using mock response for tool: {}", name);
                            round_answers.insert(tool_call_id.clone(), mock_result.clone());

                            if terminal_tools.contains(&name.as_str()) {
                                final_response = mock_result.clone();
                                tool_answers.extend(round_answers);
                                break 'agentic;
                            }
                            continue;
                        }
                    }

                    // Ontology query tools (dynamic, per-user)
                    if name.starts_with("query_") {
                        tracing::debug!("Executing ontology tool call: {}", name);
                        let result =
                            crate::tools::ontology::handle_query(name, arguments, state, user.id)
                                .await;
                        let answer = match result {
                            Ok(answer) => answer,
                            Err(e) => e,
                        };
                        round_answers.insert(tool_call_id, answer);
                        continue;
                    }

                    // Handle MCP tool calls (dynamic, per-user)
                    if crate::tool_call_utils::mcp::is_mcp_tool(name) {
                        tracing::debug!("Executing MCP tool call: {}", name);
                        let result = crate::tool_call_utils::mcp::handle_mcp_tool_call(
                            state, user.id, name, arguments,
                        )
                        .await;
                        round_answers.insert(tool_call_id, result);
                        continue;
                    }

                    // Registry dispatch for static tools
                    let tool_ctx = crate::tools::registry::ToolContext {
                        state,
                        user: &user,
                        user_id: user.id,
                        arguments,
                        image_url: image_url.as_deref(),
                        tool_call_id: tool_call_id.clone(),
                        user_given_info,
                        current_time,
                        client: Some(&ctx.client),
                        model: Some(&model),
                        tools: Some(&tools),
                        completion_messages: Some(&loop_messages),
                        assistant_content: result.choices[0].message.content.as_deref(),
                        tool_call: Some(tool_call),
                    };

                    match state.tool_registry.get(name) {
                        Some(handler) => match handler.execute(tool_ctx).await {
                            Ok(crate::tools::registry::ToolResult::Answer(answer)) => {
                                // Terminal tool: break the loop
                                if terminal_tools.contains(&name.as_str()) {
                                    final_response = answer.clone();
                                    round_answers.insert(tool_call_id, answer);
                                    tool_answers.extend(round_answers);
                                    break 'agentic;
                                }
                                round_answers.insert(tool_call_id, answer);
                            }
                            Ok(crate::tools::registry::ToolResult::AnswerWithTask {
                                answer,
                                task_id,
                            }) => {
                                created_item_id = Some(task_id);
                                final_response = answer.clone();
                                round_answers.insert(tool_call_id, answer);
                                tool_answers.extend(round_answers);
                                break 'agentic;
                            }
                            Ok(crate::tools::registry::ToolResult::EarlyReturn {
                                response,
                                status,
                            }) => {
                                let headers =
                                    [(axum::http::header::CONTENT_TYPE, "application/json")];
                                return (status, headers, Json(response));
                            }
                            Err(e) => {
                                log_tool_error(user.id, name, "execution", "handler_error", &e);
                                let error_msg = e.to_string();
                                let user_facing_msg = if error_msg.contains("plan")
                                    || error_msg.contains("feature")
                                    || error_msg.contains("upgrade")
                                    || error_msg.contains("Autopilot")
                                {
                                    error_msg
                                } else {
                                    tool_error_messages::INTERNAL_ERROR.to_string()
                                };
                                round_answers.insert(tool_call_id, user_facing_msg.clone());
                                // Terminal tools that error should still break the loop
                                if terminal_tools.contains(&name.as_str()) {
                                    final_response = user_facing_msg;
                                    tool_answers.extend(round_answers);
                                    break 'agentic;
                                }
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

                // Store tool responses in history and add to loop messages
                let hist_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32;

                for tool_call in tool_calls {
                    let answer = round_answers
                        .get(&tool_call.id)
                        .cloned()
                        .unwrap_or_default();

                    // Store in history
                    let tool_message = crate::pg_models::NewPgMessageHistory {
                        user_id: user.id,
                        role: "tool".to_string(),
                        encrypted_content: answer.clone(),
                        tool_name: tool_call.function.name.clone(),
                        tool_call_id: Some(tool_call.id.clone()),
                        tool_calls_json: None,
                        created_at: hist_time + 1,
                        conversation_id: "".to_string(),
                    };
                    if let Err(e) = state.user_repository.create_message_history(&tool_message) {
                        tracing::error!("Failed to store tool response in history: {}", e);
                    }

                    // Add to loop messages for next round
                    loop_messages.push(chat_completion::ChatCompletionMessage {
                        role: chat_completion::MessageRole::tool,
                        content: chat_completion::Content::Text(answer),
                        name: None,
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                    });
                }

                tool_answers.extend(round_answers);
                // Loop continues to next round
            }
        }
    }

    // If loop exhausted without a final response, use last tool answer
    if final_response.is_empty() && !tool_answers.is_empty() {
        tracing::warn!(
            "Agentic loop exhausted {} rounds without terminal tool",
            MAX_ROUNDS
        );
        final_response = tool_answers.values().last().cloned().unwrap_or_else(|| {
            "I was unable to complete your request. Please try again.".to_string()
        });
    } else if final_response.is_empty() {
        final_response = "I was unable to process your request. Please try again.".to_string();
    }

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

    let assistant_message = crate::pg_models::NewPgMessageHistory {
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
