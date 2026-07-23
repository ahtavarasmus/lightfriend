use crate::AppState;
use crate::UserCoreOps;
use crate::{AiChatOptions, AiChatResult, AiProvider, ModelPurpose};
use axum::{extract::Form, extract::State, http::StatusCode, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use openai_api_rs::v1::chat_completion;

mod assembly;
mod delivery;
mod early_flow;
pub(crate) mod finalize;
mod status;

pub use crate::agent_core::AgentStatus as ChatStatus;

mod tool_error_messages {
    pub const INTERNAL_ERROR: &str =
        "Sorry, we encountered an issue processing your request. Our team has been notified.";
}

/// Follow-up model call used by response verification and SMS condensation.
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

/// Channel-specific behavior surrounding the shared agent runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageChannel {
    #[default]
    Sms,
    WebChat,
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
}

/// Get the model to use based on provider and purpose.
/// Uses centralized AiConfig from AppState.
pub fn get_model(state: &Arc<AppState>, provider: AiProvider, purpose: ModelPurpose) -> String {
    state.ai_config.model(provider, purpose).to_string()
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
    let start_time = std::time::Instant::now();
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

    let assembly::SmsAgentInput {
        ctx: agent_ctx,
        user_given_info,
        image_url,
        tools,
        completion_messages,
    } = match assembly::build_sms_agent_input(state, &user, &payload, options.channel).await {
        Ok(input) => input,
        Err(e) => {
            tracing::error!("Failed to build agent context: {}", e);
            return SmsResult::SystemError {
                log_msg: format!("Failed to build agent context: {}", e),
            }
            .into_response();
        }
    };

    // Bridge channel: forward raw reasoning strings as ChatStatus::Reasoning events.
    // Only created for the web chat SSE path (when status_tx exists).
    let reasoning_tx = status::spawn_reasoning_bridge(options.status_tx.as_ref());

    let mut mock_llm_response = options.mock_llm_response.take();
    let mock_tool_responses = options.mock_tool_responses.take();
    let agent_loop_output =
        match crate::agent_core::runner::run_agent_loop(crate::agent_core::runner::AgentLoopInput {
            principal: crate::agent_core::runner::AgentPrincipal::Account { state, user: &user },
            model_purpose: agent_ctx.model_purpose,
            user_given_info: &user_given_info,
            image_url: image_url.as_deref(),
            tools: &tools,
            completion_messages,
            skip_sms: !options.channel.sends_sms(),
            reasoning_tx: &reasoning_tx,
            status_tx: options.status_tx.as_ref(),
            mock_llm_response: &mut mock_llm_response,
            mock_tool_responses: &mock_tool_responses,
            current_time: agent_ctx.current_time_unix,
            failure_messages: crate::agent_core::runner::AgentFailureMessages::account(),
        })
        .await
        {
            Ok(output) => output,
            Err(crate::agent_core::runner::AgentRunError::System { log_msg }) => {
                return SmsResult::SystemError { log_msg }.into_response();
            }
            Err(crate::agent_core::runner::AgentRunError::EarlyReturn {
                message,
                created_item_id,
                status,
            }) => {
                return SmsResult::RawResponse {
                    response: TwilioResponse {
                        message,
                        created_item_id,
                    },
                    status,
                }
                .into_response();
            }
        };

    let created_item_id = agent_loop_output.created_item_id;
    let finalized_response = finalize::finalize_sms_response(finalize::FinalizeSmsResponseInput {
        state,
        user_id: user.id,
        model_purpose: agent_ctx.model_purpose,
        tools: &tools,
        loop_messages: agent_loop_output.loop_messages,
        tool_answers: &agent_loop_output.tool_answers,
        final_response: agent_loop_output.final_response,
        fail: agent_loop_output.fail,
        active_provider: agent_loop_output.active_provider,
        sticky_provider: agent_loop_output.sticky_provider,
        provider_cost_usd: agent_loop_output.provider_cost_usd,
        reasoning_tx: &reasoning_tx,
        status_tx: options.status_tx.as_ref(),
    })
    .await;

    if options.channel == MessageChannel::WebChat
        && crate::services::metronome_billing::metronome_enabled()
    {
        let billed_cost = crate::services::usage_pricing::billable_customer_cost_usd(
            finalized_response.provider_cost_usd,
        );
        if billed_cost > 0.0 {
            if let Err(error) = crate::services::metronome_billing::enqueue_usage(
                state,
                user.id,
                "web_chat",
                billed_cost as f32,
                None,
            ) {
                tracing::error!(
                    user_id = user.id,
                    billed_cost,
                    "Failed to queue actual web chat usage: {error}"
                );
            }
        }
    }

    let processing_time_secs = start_time.elapsed().as_secs();

    delivery::deliver_sms_response(delivery::DeliverSmsResponseInput {
        state,
        user: &user,
        payload: &payload,
        channel: options.channel,
        response_for_delivery: finalized_response.response_for_delivery,
        history_for_storage: finalized_response.history_for_storage,
        created_item_id,
        processing_time_secs,
    })
    .await
}
