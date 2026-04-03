use axum::http::StatusCode;
use openai_api_rs::v1::chat_completion;

use crate::api::twilio_sms::TwilioResponse;
use crate::tools::registry::{
    write_outgoing_error_history, write_outgoing_history, ToolContext, ToolHandler, ToolResult,
};

// ─── send_email (outgoing) ───────────────────────────────────────────────────

pub struct SendEmailHandler;

#[async_trait::async_trait]
impl ToolHandler for SendEmailHandler {
    fn name(&self) -> &'static str {
        "send_email"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::email::get_send_email_tool()
    }

    fn is_outgoing(&self) -> bool {
        true
    }

    fn is_restricted(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing send_email tool call");
        match crate::tool_call_utils::email::handle_send_email(
            ctx.state,
            ctx.user_id,
            ctx.arguments,
            ctx.user,
            ctx.skip_sms,
        )
        .await
        {
            Ok((status, _headers, axum::Json(twilio_response))) => {
                write_outgoing_history(
                    ctx.state,
                    ctx.user_id,
                    "send_email",
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
                tracing::error!("Failed to handle email sending: {}", e);
                write_outgoing_error_history(
                    ctx.state,
                    ctx.user_id,
                    "send_email",
                    &ctx.tool_call_id,
                    "Failed to send email",
                    ctx.current_time,
                );
                Ok(ToolResult::EarlyReturn {
                    response: TwilioResponse {
                        message: "Failed to process email request".to_string(),
                        created_item_id: None,
                    },
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                })
            }
        }
    }
}

// ─── respond_to_email (outgoing) ─────────────────────────────────────────────

pub struct RespondEmailHandler;

#[async_trait::async_trait]
impl ToolHandler for RespondEmailHandler {
    fn name(&self) -> &'static str {
        "respond_to_email"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::email::get_respond_to_email_tool()
    }

    fn auto_injected_params(&self) -> Vec<&'static str> {
        vec!["email_id"]
    }

    fn is_outgoing(&self) -> bool {
        true
    }

    fn is_restricted(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing respond_to_email tool call");
        match crate::tool_call_utils::email::handle_respond_to_email(
            ctx.state,
            ctx.user_id,
            ctx.arguments,
            ctx.user,
            ctx.skip_sms,
        )
        .await
        {
            Ok((status, _headers, axum::Json(twilio_response))) => {
                write_outgoing_history(
                    ctx.state,
                    ctx.user_id,
                    "respond_to_email",
                    &ctx.tool_call_id,
                    &twilio_response.message,
                    // Use current_time + 1 to match original behavior
                    ctx.current_time + 1,
                );
                Ok(ToolResult::EarlyReturn {
                    response: twilio_response,
                    status,
                })
            }
            Err(e) => {
                tracing::error!("Failed to handle respond_to_email: {}", e);
                write_outgoing_error_history(
                    ctx.state,
                    ctx.user_id,
                    "respond_to_email",
                    &ctx.tool_call_id,
                    "Failed to send email",
                    ctx.current_time,
                );
                Ok(ToolResult::EarlyReturn {
                    response: TwilioResponse {
                        message: "Failed to process respond_to_email request".to_string(),
                        created_item_id: None,
                    },
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                })
            }
        }
    }
}
