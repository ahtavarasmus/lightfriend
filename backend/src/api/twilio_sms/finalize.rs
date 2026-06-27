use super::{llm_call_with_gateway, status, ChatStatus};
use crate::{AiChatOptions, AiProvider, AppState, ModelPurpose};
use openai_api_rs::v1::chat_completion;
use std::collections::HashMap;
use std::sync::Arc;

pub(super) struct FinalizeSmsResponseInput<'a> {
    pub state: &'a Arc<AppState>,
    pub user_id: i32,
    pub model_purpose: ModelPurpose,
    pub tools: &'a [chat_completion::Tool],
    pub loop_messages: Vec<chat_completion::ChatCompletionMessage>,
    pub tool_answers: &'a HashMap<String, String>,
    pub final_response: String,
    pub fail: bool,
    pub active_provider: AiProvider,
    pub sticky_provider: Option<AiProvider>,
    pub reasoning_tx: &'a Option<tokio::sync::mpsc::Sender<String>>,
    pub status_tx: Option<&'a tokio::sync::mpsc::Sender<ChatStatus>>,
}

pub(super) struct FinalizedSmsResponse {
    pub response_for_delivery: String,
    pub history_for_storage: String,
}

pub(super) async fn finalize_sms_response(
    mut input: FinalizeSmsResponseInput<'_>,
) -> FinalizedSmsResponse {
    let media_results_tag = extract_media_results_tag(input.tool_answers);
    let mut active_provider = input.active_provider;
    let mut sticky_provider = input.sticky_provider;

    let (final_response, history_for_storage) = if !input.fail {
        verify_response_with_retries(
            input.state,
            input.user_id,
            input.model_purpose,
            input.tools,
            input.reasoning_tx,
            input.status_tx,
            &input.final_response,
            &mut input.loop_messages,
            &mut active_provider,
            &mut sticky_provider,
        )
        .await
    } else {
        (input.final_response.clone(), input.final_response)
    };

    let final_response = if !input.fail {
        let condense_sticky_provider = if active_provider == AiProvider::Near {
            Some(active_provider)
        } else {
            sticky_provider
        };
        SmsResponse::new(
            final_response,
            input.state,
            input.user_id,
            condense_sticky_provider,
        )
        .await
        .into_inner()
    } else {
        SmsResponse::truncated(final_response).into_inner()
    };

    let response_for_delivery = append_media_results(final_response, &media_results_tag);

    FinalizedSmsResponse {
        response_for_delivery,
        history_for_storage,
    }
}

fn extract_media_results_tag(tool_answers: &HashMap<String, String>) -> String {
    for tool_answer in tool_answers.values() {
        tracing::debug!(
            "Checking tool answer for media (first 200 chars): {}",
            &tool_answer.chars().take(200).collect::<String>()
        );
        if let Some(start) = tool_answer.find("[MEDIA_RESULTS]") {
            if let Some(end) = tool_answer.find("[/MEDIA_RESULTS]") {
                let media_results_tag = tool_answer[start..end + 16].to_string();
                tracing::debug!(
                    "Found media results tag, length: {}",
                    media_results_tag.len()
                );
                return media_results_tag;
            }
        }
    }

    String::new()
}

#[allow(clippy::too_many_arguments)]
async fn verify_response_with_retries(
    state: &Arc<AppState>,
    user_id: i32,
    model_purpose: ModelPurpose,
    tools: &[chat_completion::Tool],
    reasoning_tx: &Option<tokio::sync::mpsc::Sender<String>>,
    status_tx: Option<&tokio::sync::mpsc::Sender<ChatStatus>>,
    final_response: &str,
    loop_messages: &mut Vec<chat_completion::ChatCompletionMessage>,
    active_provider: &mut AiProvider,
    sticky_provider: &mut Option<AiProvider>,
) -> (String, String) {
    let valid_ids = crate::utils::id_verifier::collect_tool_result_ids(loop_messages);
    let mut verified = crate::utils::id_verifier::verify(final_response, &valid_ids);

    if verified.dropped_line || verified.missing_citations {
        for retry in 1..=5 {
            let error_msg = if verified.missing_citations {
                "id-verifier detected missing citations"
            } else {
                "id-verifier stripped hallucinated citation"
            };
            tracing::info!("{}, retry {}/5", error_msg, retry);
            status::emit_status(
                status_tx,
                ChatStatus::Retrying {
                    attempt: retry,
                    max: 5,
                    error: error_msg.to_string(),
                },
            );

            let correction = if verified.missing_citations {
                "Your response did not include [id=N] citations for the items you mentioned. Rewrite your answer and include [id=N] from the tool results on each line that references a specific message, event, or person."
            } else {
                "Your previous response contained fabricated information that was automatically detected and rejected. Rewrite your answer based strictly on what the tools actually returned. Do not make anything up."
            };

            loop_messages.push(chat_completion::ChatCompletionMessage {
                role: chat_completion::MessageRole::assistant,
                content: chat_completion::Content::Text(final_response.to_string()),
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
                model_purpose,
                loop_messages,
                tools,
                reasoning_tx,
                user_id,
                *sticky_provider,
            )
            .await
            {
                Ok(result) => {
                    *active_provider = result.provider;
                    if result.fallback_from.is_some() || sticky_provider.is_some() {
                        *sticky_provider = Some(result.provider);
                    }
                    if let Some(text) = &result.response.choices[0].message.content {
                        let retry_verified = crate::utils::id_verifier::verify(text, &valid_ids);
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
}

fn append_media_results(final_response: String, media_results_tag: &str) -> String {
    if !media_results_tag.is_empty() {
        tracing::debug!("Appending media results to final response (after truncation)");
        format!("{}\n\n{}", final_response, media_results_tag)
    } else {
        tracing::debug!("No media results tag found in tool answers");
        final_response
    }
}

pub(super) struct SmsResponse {
    content: String,
}

impl SmsResponse {
    const MAX_LENGTH: usize = 480;

    async fn new(
        raw: String,
        state: &Arc<AppState>,
        user_id: i32,
        sticky_provider: Option<AiProvider>,
    ) -> Self {
        let content = if raw.chars().count() > Self::MAX_LENGTH {
            condense_response(state, &raw, Self::MAX_LENGTH, user_id, sticky_provider)
                .await
                .unwrap_or_else(|_| truncate_nicely(&raw, Self::MAX_LENGTH))
        } else {
            raw
        };
        Self { content }
    }

    fn truncated(raw: String) -> Self {
        let content = if raw.chars().count() > Self::MAX_LENGTH {
            truncate_nicely(&raw, Self::MAX_LENGTH)
        } else {
            raw
        };
        Self { content }
    }

    fn into_inner(self) -> String {
        self.content
    }
}

fn truncate_nicely(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let target_len = max_chars.saturating_sub(3);
    let chars: Vec<char> = text.chars().collect();
    let mut break_point = target_len;

    for i in (target_len.saturating_sub(50)..=target_len).rev() {
        if i < chars.len()
            && (chars[i] == '.' || chars[i] == '!' || chars[i] == '?')
            && (i + 1 >= chars.len() || chars[i + 1].is_whitespace())
        {
            break_point = i + 1;
            return chars[..break_point].iter().collect();
        }
    }

    for i in (0..=target_len).rev() {
        if i < chars.len() && chars[i].is_whitespace() {
            break_point = i;
            break;
        }
    }

    let truncated: String = chars[..break_point].iter().collect();
    format!("{}...", truncated.trim_end())
}

async fn condense_response(
    state: &Arc<AppState>,
    original: &str,
    max_chars: usize,
    user_id: i32,
    sticky_provider: Option<AiProvider>,
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
            ModelPurpose::Default,
            "condense_sms",
            &req,
            AiChatOptions {
                sticky_provider,
                ..AiChatOptions::default()
            },
        )
        .await
    {
        Ok(result) => {
            if let Some(choice) = result.response.choices.first() {
                if let Some(content) = &choice.message.content {
                    let condensed = content.trim().to_string();
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
