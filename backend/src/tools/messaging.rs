use axum::http::StatusCode;
use openai_api_rs::v1::chat_completion;

use crate::api::twilio_sms::TwilioResponse;
use crate::tools::registry::{
    write_outgoing_error_history, write_outgoing_history, ToolContext, ToolHandler, ToolResult,
};

// ─── search_chat_contacts ────────────────────────────────────────────────────

pub struct SearchContactsHandler;

#[async_trait::async_trait]
impl ToolHandler for SearchContactsHandler {
    fn name(&self) -> &'static str {
        "search_chat_contacts"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::bridge::get_search_chat_contacts_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing search_chat_contacts tool call");
        let response = crate::tool_call_utils::bridge::handle_search_chat_contacts(
            ctx.state,
            ctx.user_id,
            ctx.arguments,
        )
        .await;
        Ok(ToolResult::Answer(response))
    }
}

// ─── fetch_recent_messages ───────────────────────────────────────────────────

pub struct FetchRecentHandler;

#[async_trait::async_trait]
impl ToolHandler for FetchRecentHandler {
    fn name(&self) -> &'static str {
        "fetch_recent_messages"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::bridge::get_fetch_recent_messages_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing fetch_recent_messages tool call");
        let response = crate::tool_call_utils::bridge::handle_fetch_recent_messages(
            ctx.state,
            ctx.user_id,
            ctx.arguments,
        )
        .await;
        Ok(ToolResult::Answer(response))
    }
}

// ─── fetch_chat_messages ─────────────────────────────────────────────────────

pub struct FetchMessagesHandler;

#[async_trait::async_trait]
impl ToolHandler for FetchMessagesHandler {
    fn name(&self) -> &'static str {
        "fetch_chat_messages"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::bridge::get_fetch_chat_messages_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing fetch_chat_messages tool call");
        let response = crate::tool_call_utils::bridge::handle_fetch_chat_messages(
            ctx.state,
            ctx.user_id,
            ctx.arguments,
        )
        .await;
        Ok(ToolResult::Answer(response))
    }
}

// ─── send_chat_message (outgoing) ────────────────────────────────────────────

pub struct SendMessageHandler;

#[async_trait::async_trait]
impl ToolHandler for SendMessageHandler {
    fn name(&self) -> &'static str {
        "send_chat_message"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::bridge::get_send_chat_message_tool()
    }

    fn is_outgoing(&self) -> bool {
        true
    }

    fn is_restricted(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing send_chat_message tool call");
        match crate::tool_call_utils::bridge::handle_send_chat_message(
            ctx.state,
            ctx.user_id,
            ctx.arguments,
            ctx.user,
            ctx.image_url,
        )
        .await
        {
            Ok((status, _headers, axum::Json(twilio_response))) => {
                write_outgoing_history(
                    ctx.state,
                    ctx.user_id,
                    "send_chat_message",
                    &ctx.tool_call_id,
                    &twilio_response.message,
                    ctx.current_time,
                );
                Ok(ToolResult::EarlyReturn {
                    response: twilio_response,
                    status,
                })
            }
            Err(e) => {
                tracing::error!("Failed to handle chat message sending: {}", e);
                write_outgoing_error_history(
                    ctx.state,
                    ctx.user_id,
                    "send_chat_message",
                    &ctx.tool_call_id,
                    "Failed to send chat message",
                    ctx.current_time,
                );
                Ok(ToolResult::EarlyReturn {
                    response: TwilioResponse {
                        message: "Failed to process chat message request".to_string(),
                        created_item_id: None,
                    },
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                })
            }
        }
    }
}
