use openai_api_rs::v1::chat_completion;

use crate::tools::registry::{ToolContext, ToolHandler, ToolResult};

pub struct DirectResponseHandler;

#[async_trait::async_trait]
impl ToolHandler for DirectResponseHandler {
    fn name(&self) -> &'static str {
        "direct_response"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::internet::get_direct_response_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing direct_response tool call");
        let args: serde_json::Value = serde_json::from_str(ctx.arguments).unwrap_or_default();
        let response = args["response"]
            .as_str()
            .unwrap_or("I'm not sure how to respond to that.")
            .to_string();
        Ok(ToolResult::Answer(response))
    }
}
