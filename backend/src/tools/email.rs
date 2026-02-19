use axum::http::StatusCode;
use openai_api_rs::v1::chat_completion;
use serde::Deserialize;

use crate::api::twilio_sms::TwilioResponse;
use crate::tools::registry::{
    write_outgoing_error_history, write_outgoing_history, ToolContext, ToolHandler, ToolResult,
};

// ─── fetch_emails ────────────────────────────────────────────────────────────

pub struct FetchEmailsHandler;

#[async_trait::async_trait]
impl ToolHandler for FetchEmailsHandler {
    fn name(&self) -> &'static str {
        "fetch_emails"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::email::get_fetch_emails_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing fetch_emails tool call");
        let response =
            crate::tool_call_utils::email::handle_fetch_emails(ctx.state, ctx.user_id).await;
        Ok(ToolResult::Answer(response))
    }
}

// ─── fetch_specific_email ────────────────────────────────────────────────────

pub struct FetchSpecificEmailHandler;

#[derive(Deserialize)]
struct EmailQuery {
    query: String,
}

#[async_trait::async_trait]
impl ToolHandler for FetchSpecificEmailHandler {
    fn name(&self) -> &'static str {
        "fetch_specific_email"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::email::get_fetch_specific_email_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing fetch_specific_email tool call");
        let query: EmailQuery = serde_json::from_str(ctx.arguments).map_err(|e| e.to_string())?;

        // First get the email ID
        let email_id = crate::tool_call_utils::email::handle_fetch_specific_email(
            ctx.state,
            ctx.user_id,
            &query.query,
        )
        .await;

        let auth_user = crate::handlers::auth_middleware::AuthUser {
            user_id: ctx.user_id,
            is_admin: false,
        };

        // Then fetch the complete email with that ID
        match crate::handlers::imap_handlers::fetch_single_imap_email(
            axum::extract::State(ctx.state.clone()),
            auth_user,
            axum::extract::Path(email_id),
        )
        .await
        {
            Ok(email) => {
                let email = &email["email"];
                let response = format!(
                    "From: {}\nSubject: {}\nDate: {}\n\n{}",
                    email["from"], email["subject"], email["date_formatted"], email["body"]
                );
                Ok(ToolResult::Answer(response))
            }
            Err(_) => Ok(ToolResult::Answer(
                "Failed to fetch the complete email".to_string(),
            )),
        }
    }
}

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
                        created_task_id: None,
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
                        created_task_id: None,
                    },
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                })
            }
        }
    }
}
