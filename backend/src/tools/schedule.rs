use openai_api_rs::v1::chat_completion;

use crate::tools::registry::{ToolContext, ToolHandler, ToolResult};

// ─── create_task ─────────────────────────────────────────────────────────────

pub struct CreateTaskHandler;

#[async_trait::async_trait]
impl ToolHandler for CreateTaskHandler {
    fn name(&self) -> &'static str {
        "create_task"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::management::get_create_task_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing create_task tool call");

        // create_task needs the full completion context for retry logic
        let client = ctx.client.ok_or("create_task requires client")?;
        let model = ctx.model.ok_or("create_task requires model")?;
        let tools = ctx.tools.ok_or("create_task requires tools")?;
        let completion_messages = ctx
            .completion_messages
            .ok_or("create_task requires completion_messages")?;
        let tool_call = ctx.tool_call.ok_or("create_task requires tool_call")?;

        match crate::tool_call_utils::management::handle_create_task_with_retry(
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
                tracing::error!("Failed to create task after retry: {}", e);
                Ok(ToolResult::Answer(format!(
                    "Sorry, I couldn't create the task: {}",
                    e
                )))
            }
        }
    }
}

// ─── update_monitoring_status ────────────────────────────────────────────────

pub struct UpdateMonitoringHandler;

#[async_trait::async_trait]
impl ToolHandler for UpdateMonitoringHandler {
    fn name(&self) -> &'static str {
        "update_monitoring_status"
    }

    fn definition(&self) -> chat_completion::Tool {
        crate::tool_call_utils::management::get_update_monitoring_status_tool()
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing update_monitoring_status tool call");
        match crate::tool_call_utils::management::handle_set_proactive_agent(
            ctx.state,
            ctx.user_id,
            ctx.arguments,
        )
        .await
        {
            Ok(answer) => Ok(ToolResult::Answer(answer)),
            Err(e) => {
                tracing::error!("Failed to toggle monitoring status: {}", e);
                Ok(ToolResult::Answer(
                    "Sorry, I failed to toggle monitoring status. (Contact rasmus@ahtava.com pls:D)"
                        .to_string(),
                ))
            }
        }
    }
}
