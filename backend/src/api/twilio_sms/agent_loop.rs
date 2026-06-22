use super::{llm_call_with_gateway, tool_error_messages, ChatStatus, MessageChannel, SmsResult};
use crate::models::user_models::User;
use crate::{AiProvider, AppState, ModelPurpose};
use openai_api_rs::v1::chat_completion;
use std::collections::HashMap;
use std::sync::Arc;

pub(super) struct AgentLoopInput<'a> {
    pub state: &'a Arc<AppState>,
    pub user: &'a User,
    pub model_purpose: ModelPurpose,
    pub user_given_info: &'a str,
    pub image_url: Option<&'a str>,
    pub tools: &'a Vec<chat_completion::Tool>,
    pub completion_messages: Vec<chat_completion::ChatCompletionMessage>,
    pub channel: MessageChannel,
    pub reasoning_tx: &'a Option<tokio::sync::mpsc::Sender<String>>,
    pub status_tx: Option<&'a tokio::sync::mpsc::Sender<ChatStatus>>,
    pub mock_llm_response: &'a mut Option<chat_completion::ChatCompletionResponse>,
    pub mock_tool_responses: &'a Option<HashMap<String, String>>,
    pub current_time: i32,
}

pub(super) struct AgentLoopOutput {
    pub final_response: String,
    pub fail: bool,
    pub tool_answers: HashMap<String, String>,
    pub loop_messages: Vec<chat_completion::ChatCompletionMessage>,
    pub created_item_id: Option<i32>,
    pub active_provider: AiProvider,
    pub sticky_provider: Option<AiProvider>,
}

pub(super) async fn run_agent_loop(
    mut input: AgentLoopInput<'_>,
) -> Result<AgentLoopOutput, SmsResult> {
    const MAX_ROUNDS: u32 = 5;
    const TERMINAL_TOOLS: [&str; 1] = ["set_reminder"];

    let mut active_provider = AiProvider::Tinfoil;
    let mut sticky_provider: Option<AiProvider> = None;
    let mut fail = false;
    let mut tool_answers: HashMap<String, String> = HashMap::new();
    let mut created_item_id: Option<i32> = None;
    let mut loop_messages = input.completion_messages;
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
                        return Err(SmsResult::SystemError {
                            log_msg:
                                "No tool calls found in response despite tool_calls finish reason"
                                    .to_string(),
                        });
                    }
                };

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
                            emit_status(input.status_tx, ChatStatus::ToolCall { name: n.clone() });
                            n
                        }
                        None => {
                            log_tool_error(
                                input.user.id,
                                "unknown",
                                "llm_malformed",
                                "missing_function_name",
                                "Tool call missing function name",
                            );
                            return Err(SmsResult::SystemError {
                                log_msg: "Tool call missing function name".to_string(),
                            });
                        }
                    };

                    let arguments = match &tool_call.function.arguments {
                        Some(args) => args,
                        None => {
                            log_tool_error(
                                input.user.id,
                                name,
                                "llm_malformed",
                                "missing_arguments",
                                "Tool call missing arguments",
                            );
                            return Err(SmsResult::SystemError {
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

                    let extras = crate::agent_core::ToolDispatchExtras {
                        tools: input.tools,
                        completion_messages: &loop_messages,
                        assistant_content: result.choices[0].message.content.as_deref(),
                        tool_call: Some(tool_call),
                        image_url: input.image_url,
                        skip_sms: !input.channel.sends_sms(),
                    };
                    let dispatch_result = crate::agent_core::dispatch_tool(
                        input.state,
                        input.user,
                        name,
                        arguments,
                        &tool_call_id,
                        input.user_given_info,
                        input.current_time,
                        Some(extras),
                    )
                    .await;

                    match dispatch_result {
                        crate::agent_core::ToolDispatchResult::Answer(answer) => {
                            if TERMINAL_TOOLS.contains(&name.as_str()) {
                                final_response = answer.clone();
                                round_answers.insert(tool_call_id, answer);
                                tool_answers.extend(round_answers);
                                break 'agentic;
                            }
                            round_answers.insert(tool_call_id, answer);
                        }
                        crate::agent_core::ToolDispatchResult::AnswerWithTask {
                            answer,
                            task_id,
                        } => {
                            created_item_id = Some(task_id);
                            final_response = answer.clone();
                            round_answers.insert(tool_call_id, answer);
                            tool_answers.extend(round_answers);
                            break 'agentic;
                        }
                        crate::agent_core::ToolDispatchResult::EarlyReturn { response, status } => {
                            return Err(SmsResult::RawResponse { response, status });
                        }
                        crate::agent_core::ToolDispatchResult::SubscriptionRequired(msg) => {
                            tracing::info!(
                                "Attempted to use subscription-only tool {} without proper subscription",
                                name
                            );
                            round_answers.insert(tool_call_id, msg);
                        }
                        crate::agent_core::ToolDispatchResult::Unknown(msg) => {
                            tracing::error!("Unknown tool called: {}", name);
                            return Err(SmsResult::RawResponse {
                                status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                                response: super::TwilioResponse {
                                    message: msg,
                                    created_item_id: None,
                                },
                            });
                        }
                        crate::agent_core::ToolDispatchResult::Error(e) => {
                            log_tool_error(input.user.id, name, "execution", "handler_error", &e);
                            let user_facing_msg = if e.contains("plan")
                                || e.contains("feature")
                                || e.contains("upgrade")
                                || e.contains("Autopilot")
                            {
                                e
                            } else {
                                tool_error_messages::INTERNAL_ERROR.to_string()
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

                    let tool_message = crate::pg_models::NewPgMessageHistory {
                        user_id: input.user.id,
                        role: "tool".to_string(),
                        encrypted_content: answer.clone(),
                        tool_name: tool_call.function.name.clone(),
                        tool_call_id: Some(tool_call.id.clone()),
                        tool_calls_json: None,
                        created_at: hist_time + 1,
                        conversation_id: "".to_string(),
                    };
                    if let Err(e) = input
                        .state
                        .user_repository
                        .create_message_history(&tool_message)
                    {
                        tracing::error!("Failed to store tool response in history: {}", e);
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
) -> Result<crate::AiChatResult, SmsResult> {
    emit_status(input.status_tx, ChatStatus::Thinking);
    llm_call_with_gateway(
        input.state,
        input.model_purpose,
        loop_messages,
        input.tools,
        input.reasoning_tx,
        input.user.id,
        sticky_provider,
    )
    .await
    .map_err(|log_msg| SmsResult::SystemError { log_msg })
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

fn emit_status(status_tx: Option<&tokio::sync::mpsc::Sender<ChatStatus>>, status: ChatStatus) {
    if let Some(tx) = status_tx {
        let _ = tx.try_send(status);
    }
}

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

fn unix_now() -> i32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32
}
