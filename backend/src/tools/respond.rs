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

    async fn execute(&self, _ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing direct_response tool call");
        Ok(ToolResult::Answer(
            "No external data needed for this question. Answer the user's question directly and helpfully from your own knowledge.".to_string()
        ))
    }
}
