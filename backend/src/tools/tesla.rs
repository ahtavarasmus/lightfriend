use openai_api_rs::v1::chat_completion;

use crate::tools::registry::{ToolContext, ToolHandler, ToolResult};

// ─── control_tesla ───────────────────────────────────────────────────────────

pub struct TeslaControlHandler;

#[async_trait::async_trait]
impl ToolHandler for TeslaControlHandler {
    fn name(&self) -> &'static str {
        "control_tesla"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::tesla::get_tesla_control_tool()
    }

    fn is_restricted(&self) -> bool {
        true
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing control_tesla tool call");
        let response = crate::tool_call_utils::tesla::handle_tesla_command(
            ctx.state,
            ctx.user_id,
            ctx.arguments,
            false, // send notification for SMS-initiated commands
        )
        .await;
        Ok(ToolResult::Answer(response))
    }
}

// ─── switch_selected_tesla_vehicle ───────────────────────────────────────────

pub struct TeslaSwitchHandler;

#[async_trait::async_trait]
impl ToolHandler for TeslaSwitchHandler {
    fn name(&self) -> &'static str {
        "switch_selected_tesla_vehicle"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::tesla::get_tesla_switch_vehicle_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing switch_selected_tesla_vehicle tool call");
        let response =
            crate::tool_call_utils::tesla::handle_tesla_switch_vehicle(ctx.state, ctx.user_id)
                .await;
        Ok(ToolResult::Answer(response))
    }
}
