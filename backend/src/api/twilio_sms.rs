use crate::repositories::user_repository::LogUsageParams;
use crate::AppState;
use crate::UserCoreOps;
use crate::{AiChatOptions, AiChatResult, AiProvider, ModelPurpose};
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

mod agent_loop;
mod assembly;
mod early_flow;
mod status;

/// Error messages for tool call failures - privacy-safe, user-facing
mod tool_error_messages {
    /// Generic error shown to users when a tool fails
    pub const INTERNAL_ERROR: &str =
        "Sorry, we encountered an issue processing your request. Our team has been notified.";
}

/// LLM call through the central AI gateway, used by the agentic loop.
#[allow(clippy::too_many_arguments)]
async fn llm_call_with_gateway(
    state: &Arc<AppState>,
    purpose: ModelPurpose,
    messages: &[chat_completion::ChatCompletionMessage],
    tools: &[chat_completion::Tool],
    reasoning_tx: &Option<tokio::sync::mpsc::Sender<String>>,
    user_id: i32,
    sticky_provider: Option<AiProvider>,
) -> Result<AiChatResult, String> {
    let request = chat_completion::ChatCompletionRequest::new(String::new(), messages.to_vec())
        .tools(tools.to_vec())
        .tool_choice(chat_completion::ToolChoiceType::Auto);

    state
        .ai_config
        .chat_completion_with_fallback(
            Some(&state.llm_usage_repository),
            user_id,
            purpose,
            "chat_main",
            &request,
            AiChatOptions {
                reasoning_tx: reasoning_tx.clone(),
                sticky_provider,
                ..AiChatOptions::default()
            },
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to get chat completion through AI gateway: {:?}", e);
            format!("Failed to get chat completion: {:?}", e)
        })
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
    /// A tool produced a fully-formed HTTP response.
    RawResponse {
        response: TwilioResponse,
        status: StatusCode,
    },
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
            SmsResult::RawResponse { response, status } => (status, headers, axum::Json(response)),
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
    Reasoning {
        snippet: String,
    },
    ToolCall {
        name: String,
    },
    /// Provider call failed, about to retry. `error` carries the
    /// underlying error text so the web SSE layer can surface it to
    /// browser devtools for diagnosis — the enclave runs without
    /// --debug-mode and its tracing logs aren't reachable from the
    /// host, so this is the only way we see what Tinfoil/OpenRouter
    /// actually returned.
    Retrying {
        attempt: u32,
        max: u32,
        error: String,
    },
    RetryingFollowup {
        attempt: u32,
        max: u32,
        error: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageChannel {
    Sms,
    WebChat,
}

impl Default for MessageChannel {
    fn default() -> Self {
        Self::Sms
    }
}

impl MessageChannel {
    fn sends_sms(self) -> bool {
        matches!(self, Self::Sms)
    }

    fn agent_mode(self) -> crate::agent_core::ChannelMode {
        match self {
            Self::Sms => crate::agent_core::ChannelMode::Sms,
            Self::WebChat => crate::agent_core::ChannelMode::WebChat,
        }
    }
}

/// Options for process_sms to control test behavior
#[derive(Default)]
pub struct ProcessSmsOptions {
    /// Channel/origin for this message.
    pub channel: MessageChannel,
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
            channel: MessageChannel::WebChat,
            ..Self::default()
        }
    }

    /// Create options for web chat with status streaming
    pub fn web_chat_streaming(tx: tokio::sync::mpsc::Sender<ChatStatus>) -> Self {
        Self {
            channel: MessageChannel::WebChat,
            status_tx: Some(tx),
            ..Self::default()
        }
    }

    /// Create options for testing with mock LLM response
    pub fn test_with_mock(
        mock_response: openai_api_rs::v1::chat_completion::ChatCompletionResponse,
    ) -> Self {
        Self {
            channel: MessageChannel::WebChat,
            mock_llm_response: Some(mock_response),
            ..Self::default()
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
        user_id: i32,
        sticky_provider: Option<crate::AiProvider>,
    ) -> Self {
        let content = if raw.chars().count() > Self::MAX_LENGTH {
            // Try to condense with LLM first, fall back to truncation
            condense_response(state, &raw, Self::MAX_LENGTH, user_id, sticky_provider)
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
    original: &str,
    max_chars: usize,
    user_id: i32,
    sticky_provider: Option<crate::AiProvider>,
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
        String::new(),
        vec![ChatCompletionMessage {
            role: MessageRole::user,
            content: Content::Text(prompt),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }],
    );

    match state
        .ai_config
        .chat_completion_with_fallback(
            Some(&state.llm_usage_repository),
            user_id,
            crate::ModelPurpose::Default,
            "condense_sms",
            &req,
            crate::AiChatOptions {
                sticky_provider,
                ..crate::AiChatOptions::default()
            },
        )
        .await
    {
        Ok(result) => {
            if let Some(choice) = result.response.choices.first() {
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
    let user = match early_flow::resolve_sms_user(state, &payload.from) {
        Ok(user) => user,
        Err(result) => return result.into_response(),
    };

    if let Err(result) = early_flow::check_sms_access(state, &user).await {
        return result.into_response();
    }

    tracing::info!(
        "Found user with ID: {} for phone number: {}",
        user.id,
        payload.from
    );

    if let Some(response) = early_flow::handle_sms_early_response(
        state,
        &user,
        &payload.body,
        options.channel,
        &start_time,
    )
    .await
    {
        return response;
    }

    let agent_input =
        match assembly::build_sms_agent_input(state, &user, &payload, options.channel).await {
            Ok(input) => input,
            Err(e) => {
                tracing::error!("Failed to build agent context: {}", e);
                return SmsResult::SystemError {
                    log_msg: format!("Failed to build agent context: {}", e),
                }
                .into_response();
            }
        };
    let ctx = agent_input.ctx;
    let user_given_info = agent_input.user_given_info;
    let image_url = agent_input.image_url;
    let tools = agent_input.tools;
    let completion_messages = agent_input.completion_messages;

    // Bridge channel: forward raw reasoning strings as ChatStatus::Reasoning events.
    // Only created for the web chat SSE path (when status_tx exists).
    let reasoning_tx = status::spawn_reasoning_bridge(options.status_tx.as_ref());

    let mut mock_llm_response = options.mock_llm_response.take();
    let mock_tool_responses = options.mock_tool_responses.take();
    let agent_loop_output = match agent_loop::run_agent_loop(agent_loop::AgentLoopInput {
        state,
        user: &user,
        model_purpose: ctx.model_purpose,
        user_given_info: &user_given_info,
        image_url: image_url.as_deref(),
        tools: &tools,
        completion_messages,
        channel: options.channel,
        reasoning_tx: &reasoning_tx,
        status_tx: options.status_tx.as_ref(),
        mock_llm_response: &mut mock_llm_response,
        mock_tool_responses: &mock_tool_responses,
        current_time: ctx.current_time_unix,
    })
    .await
    {
        Ok(output) => output,
        Err(result) => return result.into_response(),
    };

    let mut active_provider = agent_loop_output.active_provider;
    let mut sticky_provider = agent_loop_output.sticky_provider;
    let final_response = agent_loop_output.final_response;
    let fail = agent_loop_output.fail;
    let tool_answers = agent_loop_output.tool_answers;
    let mut loop_messages = agent_loop_output.loop_messages;
    let created_item_id = agent_loop_output.created_item_id;

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

    // id-verifier: drop any line where the model cited an ontology
    // [id=N] that doesn't match a row returned by any tool call in
    // this turn. Runs BEFORE truncation so the user-visible footer
    // (appended when anything is dropped) participates in the SMS
    // length budget. Runs only on success paths — failure messages
    // are canned strings without ids.
    //
    // Returns two parallel versions:
    //   - `user_facing`: what to send over SMS. `[id=N]` markers
    //     stripped, footer appended if any line was dropped.
    //   - `history`: what to store in conversation history. `[id=N]`
    //     markers PRESERVED so the LLM sees its own correctly-
    //     formatted prior turn next time and keeps citing ids. If we
    //     stored the stripped version, the model would quickly "learn"
    //     from its own history that citations are optional and drop
    //     them — which would defeat the verifier on the next turn.
    //
    // `history_for_storage` is used later when we build
    // `assistant_message`. It falls back to `final_response` on the
    // failure path (no verification ran, so history == user_facing).
    let (final_response, history_for_storage) = if !fail {
        let valid_ids = crate::utils::id_verifier::collect_tool_result_ids(&loop_messages);
        let mut verified = crate::utils::id_verifier::verify(&final_response, &valid_ids);

        // If the verifier stripped hallucinated content or detected the
        // LLM ignored citations entirely, retry with a correction hint.
        // Up to 3 retries. If it still fails, silently drop the bad lines.
        if verified.dropped_line || verified.missing_citations {
            for retry in 1..=5 {
                let error_msg = if verified.missing_citations {
                    "id-verifier detected missing citations"
                } else {
                    "id-verifier stripped hallucinated citation"
                };
                tracing::info!("{}, retry {}/5", error_msg, retry);
                options.emit_status(ChatStatus::Retrying {
                    attempt: retry,
                    max: 5,
                    error: error_msg.to_string(),
                });

                let correction = if verified.missing_citations {
                    "Your response did not include [id=N] citations for the items you mentioned. Rewrite your answer and include [id=N] from the tool results on each line that references a specific message, event, or person."
                } else {
                    "Your previous response contained fabricated information that was automatically detected and rejected. Rewrite your answer based strictly on what the tools actually returned. Do not make anything up."
                };

                loop_messages.push(chat_completion::ChatCompletionMessage {
                    role: chat_completion::MessageRole::assistant,
                    content: chat_completion::Content::Text(final_response.clone()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                });
                loop_messages.push(chat_completion::ChatCompletionMessage {
                    role: chat_completion::MessageRole::system,
                    content: chat_completion::Content::Text(correction.to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                });

                match llm_call_with_gateway(
                    state,
                    ctx.model_purpose,
                    &loop_messages,
                    &tools,
                    &reasoning_tx,
                    user.id,
                    sticky_provider,
                )
                .await
                {
                    Ok(r) => {
                        active_provider = r.provider;
                        if r.fallback_from.is_some() || sticky_provider.is_some() {
                            sticky_provider = Some(r.provider);
                        }
                        if let Some(text) = &r.response.choices[0].message.content {
                            let retry_verified =
                                crate::utils::id_verifier::verify(text, &valid_ids);
                            // Remove the correction messages for next iteration
                            loop_messages.pop();
                            loop_messages.pop();
                            if !retry_verified.dropped_line && !retry_verified.missing_citations {
                                verified = retry_verified;
                                break;
                            }
                            verified = retry_verified;
                        } else {
                            loop_messages.pop();
                            loop_messages.pop();
                            break;
                        }
                    }
                    Err(_) => {
                        loop_messages.pop();
                        loop_messages.pop();
                        break;
                    }
                }
            }

            // After retries, if still dropping lines or missing citations,
            // silently strip without the footer
            if verified.dropped_line || verified.missing_citations {
                tracing::info!("Id verifier still flagging after 5 retries, silently dropping");
                verified.user_facing = verified
                    .user_facing
                    .replace(crate::utils::id_verifier::STRIPPED_FOOTER, "")
                    .trim_end()
                    .to_string();
            }
        }

        (verified.user_facing, verified.history)
    } else {
        (final_response.clone(), final_response)
    };

    // Ensure response is within SMS character limit (truncate text BEFORE adding media)
    let final_response = if !fail {
        // For successful responses, use LLM condensing if needed
        let condense_sticky_provider = if active_provider == AiProvider::Near {
            Some(active_provider)
        } else {
            sticky_provider
        };
        SmsResponse::new(final_response, state, user.id, condense_sticky_provider)
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

    if let Err(e) = state.user_repository.delete_old_message_history(user.id) {
        tracing::error!("Failed to clean up old message history: {}", e);
    }

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // History storage uses the verifier's `history` version (tag
    // markers preserved, no footer, un-truncated). Next turn the LLM
    // will see its own correctly-cited prior turn and keep citing
    // ids — if we stored `final_response_with_notice` instead, the
    // stripped view would train the model to drop citations.
    let assistant_message = crate::pg_models::NewPgMessageHistory {
        user_id: user.id,
        role: "assistant".to_string(),
        encrypted_content: history_for_storage.clone(),
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

    // If this came from web chat, return the response without sending an SMS.
    if !options.channel.sends_sms() {
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
    let media_ref = media_sid.map(|s| crate::channels::traits::MediaRef::Url(s.clone()));
    match state
        .channel_router
        .send_to_user(&user, &clean_response, media_ref)
        .await
    {
        Ok(message_sid) => {
            let message_sid = message_sid.into_inner();
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
