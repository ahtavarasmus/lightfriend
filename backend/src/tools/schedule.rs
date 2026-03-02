use openai_api_rs::v1::chat_completion;

use crate::tools::registry::{ToolContext, ToolHandler, ToolResult};

// --- create_item ---------------------------------------------------------------

pub struct CreateItemHandler;

#[async_trait::async_trait]
impl ToolHandler for CreateItemHandler {
    fn name(&self) -> &'static str {
        "create_item"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::management::get_create_item_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing create_item tool call");

        // create_item needs the full completion context for retry logic
        let client = ctx.client.ok_or("create_item requires client")?;
        let model = ctx.model.ok_or("create_item requires model")?;
        let tools = ctx.tools.ok_or("create_item requires tools")?;
        let completion_messages = ctx
            .completion_messages
            .ok_or("create_item requires completion_messages")?;
        let tool_call = ctx.tool_call.ok_or("create_item requires tool_call")?;

        match crate::tool_call_utils::management::handle_create_item_with_retry(
            ctx.state,
            ctx.user_id,
            ctx.arguments,
            client,
            model,
            tools,
            completion_messages,
            tool_call,
            ctx.assistant_content,
        )
        .await
        {
            Ok(task_result) => Ok(ToolResult::AnswerWithTask {
                answer: task_result.message,
                task_id: task_result.task_id,
            }),
            Err(e) => {
                tracing::error!("Failed to create item after retry: {}", e);
                Err(format!("Sorry, I couldn't create the item: {}", e))
            }
        }
    }
}
