use openai_api_rs::v1::chat_completion;

use crate::tools::registry::{ToolContext, ToolHandler, ToolResult};

pub struct YouTubeHandler;

#[derive(serde::Deserialize)]
struct YouTubeQuery {
    query: String,
}

#[async_trait::async_trait]
impl ToolHandler for YouTubeHandler {
    fn name(&self) -> &'static str {
        "youtube"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::youtube::get_youtube_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing youtube tool call");
        let q: YouTubeQuery = serde_json::from_str(ctx.arguments)
            .map_err(|e| format!("Failed to parse YouTube arguments: {}", e))?;
        let response =
            crate::tool_call_utils::youtube::handle_youtube_tool(ctx.state, ctx.user_id, &q.query)
                .await;
        Ok(ToolResult::Answer(response))
    }
}
