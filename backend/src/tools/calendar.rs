use axum::http::StatusCode;
use openai_api_rs::v1::chat_completion;

use crate::api::twilio_sms::TwilioResponse;
use crate::tools::registry::{
    write_outgoing_error_history, write_outgoing_history, ToolContext, ToolHandler, ToolResult,
};

// ─── fetch_calendar_events ───────────────────────────────────────────────────

pub struct FetchEventsHandler;

#[async_trait::async_trait]
impl ToolHandler for FetchEventsHandler {
    fn name(&self) -> &'static str {
        "fetch_calendar_events"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::calendar::get_fetch_calendar_event_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing fetch_calendar_events tool call");
        let response = crate::tool_call_utils::calendar::handle_fetch_calendar_events(
            ctx.state,
            ctx.user_id,
            ctx.arguments,
        )
        .await;
        Ok(ToolResult::Answer(response))
    }
}

// ─── create_calendar_event (outgoing) ────────────────────────────────────────

pub struct CreateEventHandler;

#[async_trait::async_trait]
impl ToolHandler for CreateEventHandler {
    fn name(&self) -> &'static str {
        "create_calendar_event"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::calendar::get_create_calendar_event_tool()
    }

    fn is_outgoing(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing create_calendar_event tool call");
        match crate::tool_call_utils::calendar::handle_create_calendar_event(
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
                    "create_calendar_event",
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
                tracing::error!("Failed to handle calendar event creation: {}", e);
                write_outgoing_error_history(
                    ctx.state,
                    ctx.user_id,
                    "create_calendar_event",
                    &ctx.tool_call_id,
                    "Failed to create_calendar event",
                    ctx.current_time,
                );
                Ok(ToolResult::EarlyReturn {
                    response: TwilioResponse {
                        message: "Failed to process calendar event request".to_string(),
                        created_item_id: None,
                    },
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                })
            }
        }
    }
}
