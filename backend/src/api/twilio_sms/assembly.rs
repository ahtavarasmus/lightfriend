use super::{MessageChannel, TwilioWebhookPayload};
use crate::context::{AgentContext, ContextBuilder, ContextError};
use crate::models::user_models::User;
use crate::tool_call_utils::utils::ChatMessage;
use crate::AppState;
use openai_api_rs::v1::chat_completion;
use std::sync::Arc;

pub(super) struct SmsAgentInput {
    pub ctx: AgentContext,
    pub user_given_info: String,
    pub image_url: Option<String>,
    pub tools: Vec<chat_completion::Tool>,
    pub completion_messages: Vec<chat_completion::ChatCompletionMessage>,
}

pub(super) async fn build_sms_agent_input(
    state: &Arc<AppState>,
    user: &User,
    payload: &TwilioWebhookPayload,
    channel: MessageChannel,
) -> Result<SmsAgentInput, ContextError> {
    log_admin_media_info(user, payload);
    persist_incoming_message(state, user, payload);

    let ctx = build_agent_context(state, user, payload).await?;
    let user_given_info = ctx.user_given_info.clone().unwrap_or_default();
    let mut chat_messages = build_initial_messages(&ctx, channel);
    let processed_body = strip_forget_prefix(&payload.body);
    delete_incoming_media_if_present(state, user, payload).await;

    add_history_messages(&mut chat_messages, &ctx);

    let image_url = add_current_user_message(&mut chat_messages, payload, &processed_body);
    let tools = crate::agent_core::build_tools(state, user.id, true).await;
    let completion_messages = to_completion_messages(chat_messages);

    Ok(SmsAgentInput {
        ctx,
        user_given_info,
        image_url,
        tools,
        completion_messages,
    })
}

fn log_admin_media_info(user: &User, payload: &TwilioWebhookPayload) {
    if user.id != 1 {
        return;
    }

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

fn persist_incoming_message(state: &Arc<AppState>, user: &User, payload: &TwilioWebhookPayload) {
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

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
}

async fn build_agent_context(
    state: &Arc<AppState>,
    user: &User,
    payload: &TwilioWebhookPayload,
) -> Result<AgentContext, ContextError> {
    let wants_history = !payload.body.to_lowercase().starts_with("forget");
    let mut builder = ContextBuilder::for_resolved_user(state, user.clone()).with_user_context();
    if wants_history {
        builder = builder.with_history();
    }
    builder.build().await
}

fn build_initial_messages(ctx: &AgentContext, channel: MessageChannel) -> Vec<ChatMessage> {
    let system_prompt_text = crate::agent_core::build_system_prompt(ctx, channel.agent_mode());

    vec![ChatMessage {
        role: "system".to_string(),
        content: chat_completion::Content::Text(system_prompt_text),
        tool_calls: None,
        tool_call_id: None,
    }]
}

fn strip_forget_prefix(body: &str) -> String {
    if body.to_lowercase().starts_with("forget") {
        body.trim_start_matches(|c: char| c.is_alphabetic())
            .trim()
            .to_string()
    } else {
        body.to_string()
    }
}

async fn delete_incoming_media_if_present(
    state: &Arc<AppState>,
    user: &User,
    payload: &TwilioWebhookPayload,
) {
    if let (Some(num_media), Some(media_url), Some(_)) = (
        payload.num_media.as_ref(),
        payload.media_url0.as_ref(),
        payload.media_content_type0.as_ref(),
    ) {
        if num_media != "0" {
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
                        .delete_message_media(user, message_sid, media_sid)
                        .await
                    {
                        Ok(_) => tracing::debug!("Successfully deleted media: {}", media_sid),
                        Err(e) => tracing::error!("Failed to delete media {}: {}", media_sid, e),
                    }
                }
            }
        }
    }
}

fn add_history_messages(chat_messages: &mut Vec<ChatMessage>, ctx: &AgentContext) {
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
}

fn add_current_user_message(
    chat_messages: &mut Vec<ChatMessage>,
    payload: &TwilioWebhookPayload,
    processed_body: &str,
) -> Option<String> {
    if let (Some(num_media), Some(media_url), Some(content_type)) = (
        payload.num_media.as_ref(),
        payload.media_url0.as_ref(),
        payload.media_content_type0.as_ref(),
    ) {
        if num_media != "0" && content_type.starts_with("image/") {
            let image_url = Some(media_url.clone());
            tracing::debug!("setting image_url var to: {:#?}", image_url);
            chat_messages.push(ChatMessage {
                role: "user".to_string(),
                content: chat_completion::Content::ImageUrl(build_image_content(
                    processed_body,
                    media_url,
                )),
                tool_calls: None,
                tool_call_id: None,
            });
            return image_url;
        }
    }

    chat_messages.push(ChatMessage {
        role: "user".to_string(),
        content: chat_completion::Content::Text(processed_body.to_string()),
        tool_calls: None,
        tool_call_id: None,
    });
    None
}

fn build_image_content(processed_body: &str, media_url: &str) -> Vec<chat_completion::ImageUrl> {
    let mut content_parts = vec![];

    if !processed_body.trim().is_empty() {
        content_parts.push(chat_completion::ImageUrl {
            r#type: chat_completion::ContentType::text,
            text: Some(processed_body.to_string()),
            image_url: None,
        });
    }

    content_parts.push(chat_completion::ImageUrl {
        r#type: chat_completion::ContentType::image_url,
        text: None,
        image_url: Some(chat_completion::ImageUrlType {
            url: media_url.to_string(),
        }),
    });

    content_parts
}

fn to_completion_messages(
    chat_messages: Vec<ChatMessage>,
) -> Vec<chat_completion::ChatCompletionMessage> {
    chat_messages
        .into_iter()
        .map(|msg| chat_completion::ChatCompletionMessage {
            role: match msg.role.as_str() {
                "user" => chat_completion::MessageRole::user,
                "assistant" => chat_completion::MessageRole::assistant,
                "system" => chat_completion::MessageRole::system,
                _ => chat_completion::MessageRole::user,
            },
            content: msg.content,
            name: None,
            tool_calls: msg.tool_calls,
            tool_call_id: msg.tool_call_id,
        })
        .collect()
}
