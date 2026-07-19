//! Shared multi-round text agent runner.
//!
//! Transport adapters provide an explicit principal, message context, tool set,
//! and status sink. The principal determines both runtime dependencies and the
//! only tool executor the run can reach.

use super::{AgentStatus, ToolDispatchExtras, ToolDispatchResult};
use crate::models::user_models::User;
use crate::{AiChatOptions, AiProvider, AppState, ModelPurpose};
use axum::http::StatusCode;
use openai_api_rs::v1::chat_completion;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

const INTERNAL_TOOL_ERROR: &str =
    "Sorry, we encountered an issue processing your request. Our team has been notified.";
const ANONYMOUS_TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const MAX_ANONYMOUS_TOOL_RESULT_CHARACTERS: usize = 12_000;

pub fn anonymous_light_tool_tools() -> Vec<chat_completion::Tool> {
    let mut weather = crate::tool_call_utils::internet::get_weather_tool();
    weather.function.description = Some(
        "Fetch weather for an explicitly supplied location. Ask the user for a location when none \
         appears in the conversation."
            .to_string(),
    );
    vec![
        weather,
        crate::tool_call_utils::internet::get_firecrawl_search_tool(),
    ]
}

#[derive(Clone, Copy)]
pub enum AgentPrincipal<'a> {
    Account {
        state: &'a Arc<AppState>,
        user: &'a User,
    },
    AnonymousLightTool {
        device_id: i32,
        ai_config: &'a crate::AiConfig,
    },
}

impl<'a> AgentPrincipal<'a> {
    fn account_user(self) -> Option<&'a User> {
        match self {
            Self::Account { user, .. } => Some(user),
            Self::AnonymousLightTool { .. } => None,
        }
    }

    fn usage_user_id(self) -> Option<i32> {
        self.account_user().map(|user| user.id)
    }

    fn ai_config(self) -> &'a crate::AiConfig {
        match self {
            Self::Account { state, .. } => &state.ai_config,
            Self::AnonymousLightTool { ai_config, .. } => ai_config,
        }
    }

    fn usage_repository(
        self,
    ) -> Option<&'a Arc<crate::repositories::llm_usage_repository::LlmUsageRepository>> {
        match self {
            Self::Account { state, .. } => Some(&state.llm_usage_repository),
            Self::AnonymousLightTool { .. } => None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct AgentFailureMessages {
    pub length: &'static str,
    pub content_filter: &'static str,
    pub general: &'static str,
}

impl AgentFailureMessages {
    pub const fn account() -> Self {
        Self {
            length: "I apologize, but my response was too long. Could you please ask your question in a more specific way? (you were not charged for this message)",
            content_filter: "I apologize, but I cannot provide an answer to that question due to content restrictions. (you were not charged for this message)",
            general: "I apologize, but something went wrong while processing your request. (you were not charged for this message)",
        }
    }

    pub const fn anonymous_trial() -> Self {
        Self {
            length: "My response was too long. Please ask a more specific question.",
            content_filter:
                "I cannot provide an answer to that question due to content restrictions.",
            general: "Something went wrong while processing your request.",
        }
    }
}

#[derive(Debug, Error)]
pub enum AgentRunError {
    #[error("{log_msg}")]
    System { log_msg: String },
    #[error("agent tool returned an early response")]
    EarlyReturn {
        message: String,
        created_item_id: Option<i32>,
        status: StatusCode,
    },
}

pub struct AgentLoopInput<'a> {
    pub principal: AgentPrincipal<'a>,
    pub model_purpose: ModelPurpose,
    pub user_given_info: &'a str,
    pub image_url: Option<&'a str>,
    pub tools: &'a Vec<chat_completion::Tool>,
    pub completion_messages: Vec<chat_completion::ChatCompletionMessage>,
    pub skip_sms: bool,
    pub reasoning_tx: &'a Option<tokio::sync::mpsc::Sender<String>>,
    pub status_tx: Option<&'a tokio::sync::mpsc::Sender<AgentStatus>>,
    pub mock_llm_response: &'a mut Option<chat_completion::ChatCompletionResponse>,
    pub mock_tool_responses: &'a Option<HashMap<String, String>>,
    pub current_time: i32,
    pub failure_messages: AgentFailureMessages,
}

pub struct AgentLoopOutput {
    pub final_response: String,
    pub fail: bool,
    pub tool_answers: HashMap<String, String>,
    pub loop_messages: Vec<chat_completion::ChatCompletionMessage>,
    pub created_item_id: Option<i32>,
    pub active_provider: AiProvider,
    pub sticky_provider: Option<AiProvider>,
}

pub async fn run_agent_loop(
    mut input: AgentLoopInput<'_>,
) -> Result<AgentLoopOutput, AgentRunError> {
    const MAX_ROUNDS: u32 = 5;
    const TERMINAL_TOOLS: [&str; 1] = ["set_reminder"];

    let mut active_provider = AiProvider::Tinfoil;
    let mut sticky_provider: Option<AiProvider> = None;
    let mut fail = false;
    let mut tool_answers: HashMap<String, String> = HashMap::new();
    let mut created_item_id: Option<i32> = None;
    let mut loop_messages = std::mem::take(&mut input.completion_messages);
    let mut final_response = String::new();

    'agentic: for round in 0..MAX_ROUNDS {
        tracing::debug!("Agentic loop round {}/{}", round + 1, MAX_ROUNDS);

        let result = if round == 0 {
            if let Some(mock_response) = input.mock_llm_response.take() {
                tracing::debug!("Using mock LLM response for testing");
                mock_response
            } else {
                call_llm_round(&input, &loop_messages, sticky_provider)
                    .await
                    .map(|r| {
                        apply_provider_result(&mut active_provider, &mut sticky_provider, &r);
                        r.response
                    })?
            }
        } else {
            call_llm_round(&input, &loop_messages, sticky_provider)
                .await
                .map(|r| {
                    apply_provider_result(&mut active_provider, &mut sticky_provider, &r);
                    r.response
                })?
        };

        let choice = result
            .choices
            .first()
            .ok_or_else(|| AgentRunError::System {
                log_msg: "AI provider returned no choices".to_string(),
            })?;

        match choice.finish_reason {
            None | Some(chat_completion::FinishReason::stop) => {
                tracing::debug!("Model provided direct response (no tool calls)");
                final_response = choice.message.content.clone().unwrap_or_default();
                break 'agentic;
            }
            Some(chat_completion::FinishReason::length) => {
                fail = true;
                final_response = input.failure_messages.length.to_string();
                break 'agentic;
            }
            Some(chat_completion::FinishReason::content_filter) => {
                fail = true;
                final_response = input.failure_messages.content_filter.to_string();
                break 'agentic;
            }
            Some(chat_completion::FinishReason::null) => {
                fail = true;
                final_response = input.failure_messages.general.to_string();
                break 'agentic;
            }
            Some(chat_completion::FinishReason::tool_calls) => {
                tracing::debug!("Model requested tool calls (round {})", round + 1);

                let tool_calls = match choice.message.tool_calls.as_ref() {
                    Some(calls) if !calls.is_empty() => {
                        tracing::debug!("Found {} tool call(s) in response", calls.len());
                        calls
                    }
                    _ => {
                        tracing::error!(
                            "No tool calls found in response despite tool_calls finish reason"
                        );
                        return Err(AgentRunError::System {
                            log_msg:
                                "No tool calls found in response despite tool_calls finish reason"
                                    .to_string(),
                        });
                    }
                };

                loop_messages.push(chat_completion::ChatCompletionMessage {
                    role: chat_completion::MessageRole::assistant,
                    content: chat_completion::Content::Text(
                        choice.message.content.clone().unwrap_or_default(),
                    ),
                    name: None,
                    tool_calls: choice.message.tool_calls.clone(),
                    tool_call_id: None,
                });

                let mut round_answers: HashMap<String, String> = HashMap::new();

                for tool_call in tool_calls {
                    let tool_call_id = tool_call.id.clone();
                    tracing::debug!(tool_call_id = %tool_call_id, "Processing agent tool call");
                    let name = match &tool_call.function.name {
                        Some(n) => {
                            tracing::debug!("Tool call function name: {}", n);
                            emit_status(input.status_tx, AgentStatus::ToolCall { name: n.clone() });
                            n
                        }
                        None => {
                            log_tool_error(
                                input.principal,
                                "unknown",
                                "llm_malformed",
                                "missing_function_name",
                                "Tool call missing function name",
                            );
                            return Err(AgentRunError::System {
                                log_msg: "Tool call missing function name".to_string(),
                            });
                        }
                    };

                    let arguments = match &tool_call.function.arguments {
                        Some(args) => args,
                        None => {
                            log_tool_error(
                                input.principal,
                                name,
                                "llm_malformed",
                                "missing_arguments",
                                "Tool call missing arguments",
                            );
                            return Err(AgentRunError::System {
                                log_msg: format!("Tool call {} missing arguments", name),
                            });
                        }
                    };

                    if let Some(ref mock_map) = input.mock_tool_responses {
                        if let Some(mock_result) = mock_map.get(name) {
                            tracing::debug!("Using mock response for tool: {}", name);
                            round_answers.insert(tool_call_id.clone(), mock_result.clone());

                            if TERMINAL_TOOLS.contains(&name.as_str()) {
                                final_response = mock_result.clone();
                                tool_answers.extend(round_answers);
                                break 'agentic;
                            }
                            continue;
                        }
                    }

                    let extras = ToolDispatchExtras {
                        tools: input.tools,
                        completion_messages: &loop_messages,
                        assistant_content: choice.message.content.as_deref(),
                        tool_call: Some(tool_call),
                        image_url: input.image_url,
                        skip_sms: input.skip_sms,
                    };
                    let dispatch_result = dispatch_tool_for_principal(
                        input.principal,
                        name,
                        arguments,
                        &tool_call_id,
                        input.user_given_info,
                        input.current_time,
                        extras,
                    )
                    .await;

                    match dispatch_result {
                        ToolDispatchResult::Answer(answer) => {
                            if TERMINAL_TOOLS.contains(&name.as_str()) {
                                final_response = answer.clone();
                                round_answers.insert(tool_call_id, answer);
                                tool_answers.extend(round_answers);
                                break 'agentic;
                            }
                            round_answers.insert(tool_call_id, answer);
                        }
                        ToolDispatchResult::AnswerWithTask { answer, task_id } => {
                            created_item_id = Some(task_id);
                            final_response = answer.clone();
                            round_answers.insert(tool_call_id, answer);
                            tool_answers.extend(round_answers);
                            break 'agentic;
                        }
                        ToolDispatchResult::EarlyReturn { response, status } => {
                            return Err(AgentRunError::EarlyReturn {
                                message: response.message,
                                created_item_id: response.created_item_id,
                                status,
                            });
                        }
                        ToolDispatchResult::SubscriptionRequired(msg) => {
                            tracing::info!(
                                "Attempted to use subscription-only tool {} without proper subscription",
                                name
                            );
                            round_answers.insert(tool_call_id, msg);
                        }
                        ToolDispatchResult::Unknown(msg) => {
                            tracing::error!("Unknown tool called: {}", name);
                            return Err(AgentRunError::EarlyReturn {
                                status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                                message: msg,
                                created_item_id: None,
                            });
                        }
                        ToolDispatchResult::Error(e) => {
                            log_tool_error(input.principal, name, "execution", "handler_error", &e);
                            let user_facing_msg = if e.contains("plan")
                                || e.contains("feature")
                                || e.contains("upgrade")
                                || e.contains("Autopilot")
                            {
                                e
                            } else {
                                INTERNAL_TOOL_ERROR.to_string()
                            };
                            round_answers.insert(tool_call_id, user_facing_msg.clone());
                            if TERMINAL_TOOLS.contains(&name.as_str()) {
                                final_response = user_facing_msg;
                                tool_answers.extend(round_answers);
                                break 'agentic;
                            }
                        }
                    }
                }

                let hist_time = unix_now();
                for tool_call in tool_calls {
                    let answer = round_answers
                        .get(&tool_call.id)
                        .cloned()
                        .unwrap_or_default();

                    if let AgentPrincipal::Account { state, user } = input.principal {
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
                        if let Err(e) = state.user_repository.create_message_history(&tool_message)
                        {
                            tracing::error!("Failed to store tool response in history: {}", e);
                        }
                    }

                    loop_messages.push(chat_completion::ChatCompletionMessage {
                        role: chat_completion::MessageRole::tool,
                        content: chat_completion::Content::Text(answer),
                        name: None,
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                    });
                }

                tool_answers.extend(round_answers);
            }
        }
    }

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

    Ok(AgentLoopOutput {
        final_response,
        fail,
        tool_answers,
        loop_messages,
        created_item_id,
        active_provider,
        sticky_provider,
    })
}

async fn call_llm_round(
    input: &AgentLoopInput<'_>,
    loop_messages: &[chat_completion::ChatCompletionMessage],
    sticky_provider: Option<AiProvider>,
) -> Result<crate::AiChatResult, AgentRunError> {
    emit_status(input.status_tx, AgentStatus::Thinking);
    let request =
        chat_completion::ChatCompletionRequest::new(String::new(), loop_messages.to_vec())
            .tools(input.tools.to_vec())
            .tool_choice(chat_completion::ToolChoiceType::Auto);
    let usage_user_id = input.principal.usage_user_id();
    let usage_repository = input.principal.usage_repository();

    input
        .principal
        .ai_config()
        .chat_completion_with_fallback(
            usage_repository,
            usage_user_id.unwrap_or(0),
            input.model_purpose,
            "chat_main",
            &request,
            AiChatOptions {
                reasoning_tx: input.reasoning_tx.clone(),
                sticky_provider,
                ..AiChatOptions::default()
            },
        )
        .await
        .map_err(|error| {
            tracing::error!(
                "Failed to get chat completion through AI gateway: {:?}",
                error
            );
            AgentRunError::System {
                log_msg: format!("Failed to get chat completion: {:?}", error),
            }
        })
}

fn apply_provider_result(
    active_provider: &mut AiProvider,
    sticky_provider: &mut Option<AiProvider>,
    result: &crate::AiChatResult,
) {
    *active_provider = result.provider;
    if result.fallback_from.is_some() || sticky_provider.is_some() {
        *sticky_provider = Some(result.provider);
    }
}

fn log_tool_error(
    principal: AgentPrincipal<'_>,
    tool_name: &str,
    category: &str,
    error_type: &str,
    error_msg: &str,
) {
    let account_user_id = principal.account_user().map(|user| user.id);
    let anonymous_device_id = match principal {
        AgentPrincipal::AnonymousLightTool { device_id, .. } => Some(device_id),
        AgentPrincipal::Account { .. } => None,
    };
    tracing::error!(
        account_user_id,
        anonymous_device_id,
        tool_name = tool_name,
        error_category = category,
        error_type = error_type,
        "Tool execution failed: {}",
        error_msg
    );
}

fn emit_status(status_tx: Option<&tokio::sync::mpsc::Sender<AgentStatus>>, status: AgentStatus) {
    if let Some(tx) = status_tx {
        let _ = tx.try_send(status);
    }
}

async fn dispatch_tool_for_principal(
    principal: AgentPrincipal<'_>,
    name: &str,
    arguments: &str,
    tool_call_id: &str,
    user_given_info: &str,
    current_time: i32,
    extras: ToolDispatchExtras<'_>,
) -> ToolDispatchResult {
    match principal {
        AgentPrincipal::Account { state, user } => {
            crate::agent_core::dispatch_tool(
                state,
                user,
                name,
                arguments,
                tool_call_id,
                user_given_info,
                current_time,
                Some(extras),
            )
            .await
        }
        AgentPrincipal::AnonymousLightTool { .. } => {
            dispatch_anonymous_light_tool(name, arguments).await
        }
    }
}

async fn dispatch_anonymous_light_tool(name: &str, arguments: &str) -> ToolDispatchResult {
    match name {
        "search_firecrawl" => {
            #[derive(serde::Deserialize)]
            struct SearchArguments {
                query: String,
            }

            let arguments = match serde_json::from_str::<SearchArguments>(arguments) {
                Ok(arguments) => arguments,
                Err(error) => return ToolDispatchResult::Error(error.to_string()),
            };
            match tokio::time::timeout(
                ANONYMOUS_TOOL_TIMEOUT,
                crate::utils::tool_exec::handle_firecrawl_search(arguments.query, 5),
            )
            .await
            {
                Ok(Ok(answer)) => ToolDispatchResult::Answer(bound_anonymous_tool_result(&answer)),
                Ok(Err(error)) => ToolDispatchResult::Error(error.to_string()),
                Err(_) => ToolDispatchResult::Error("Public web search timed out".to_string()),
            }
        }
        "get_weather" => {
            #[derive(serde::Deserialize)]
            struct WeatherArguments {
                location: String,
                units: String,
                forecast_type: Option<String>,
            }

            let arguments = match serde_json::from_str::<WeatherArguments>(arguments) {
                Ok(arguments) => arguments,
                Err(error) => return ToolDispatchResult::Error(error.to_string()),
            };
            let forecast_type = arguments
                .forecast_type
                .unwrap_or_else(|| "current".to_string());
            match tokio::time::timeout(
                ANONYMOUS_TOOL_TIMEOUT,
                crate::utils::tool_exec::get_weather_for_location(
                    &arguments.location,
                    &arguments.units,
                    &forecast_type,
                    None,
                ),
            )
            .await
            {
                Ok(Ok(answer)) => ToolDispatchResult::Answer(bound_anonymous_tool_result(&answer)),
                Ok(Err(error)) => ToolDispatchResult::Error(error.to_string()),
                Err(_) => ToolDispatchResult::Error("Public weather lookup timed out".to_string()),
            }
        }
        _ => {
            ToolDispatchResult::Unknown("Tool is not available in the anonymous trial".to_string())
        }
    }
}

fn bound_anonymous_tool_result(result: &str) -> String {
    if result.chars().count() <= MAX_ANONYMOUS_TOOL_RESULT_CHARACTERS {
        return result.to_string();
    }

    const SUFFIX: &str = " [truncated]";
    let keep = MAX_ANONYMOUS_TOOL_RESULT_CHARACTERS - SUFFIX.chars().count();
    let mut bounded = result.chars().take(keep).collect::<String>();
    bounded.push_str(SUFFIX);
    bounded
}

fn unix_now() -> i32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32
}
