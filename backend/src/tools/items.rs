use openai_api_rs::v1::{chat_completion, types};
use serde::Deserialize;

use crate::tools::registry::{ToolContext, ToolHandler, ToolResult};

// --- list_tracked_items ---

pub struct ListTrackedItemsHandler;

#[async_trait::async_trait]
impl ToolHandler for ListTrackedItemsHandler {
    fn name(&self) -> &'static str {
        "list_tracked_items"
    }

    fn definition(&self) -> chat_completion::Tool {
        let properties = std::collections::HashMap::new();
        // No parameters needed - lists all pending items for the user

        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: "list_tracked_items".to_string(),
                description: Some(
                    "List all pending tracked items (invoices, shipments, deadlines, etc.) that the system has automatically detected from emails.".to_string(),
                ),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(properties),
                    required: None,
                },
            },
        }
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing list_tracked_items tool call");

        let items = ctx
            .state
            .item_repository
            .get_items(ctx.user_id)
            .map_err(|e| format!("Failed to get items: {}", e))?;

        if items.is_empty() {
            return Ok(ToolResult::Answer(
                "No tracked items right now. Items are auto-detected from your emails (invoices, packages, deadlines) or created manually.".to_string(),
            ));
        }

        let mut result = format!("{} item(s):\n", items.len());
        for item in &items {
            let kind_tag = if item.monitor {
                " [monitor]".to_string()
            } else {
                String::new()
            };

            result.push_str(&format!(
                "- [{}] {}{}\n",
                item.id.unwrap_or(0),
                item.summary,
                kind_tag,
            ));
        }

        Ok(ToolResult::Answer(result))
    }
}

// --- update_tracked_item ---

pub struct UpdateTrackedItemHandler;

#[derive(Deserialize)]
struct UpdateTrackedArgs {
    item_id: i32,
    action: String, // "complete" or "dismiss"
}

#[async_trait::async_trait]
impl ToolHandler for UpdateTrackedItemHandler {
    fn name(&self) -> &'static str {
        "update_tracked_item"
    }

    fn definition(&self) -> chat_completion::Tool {
        let mut properties = std::collections::HashMap::new();
        properties.insert(
            "item_id".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::Number),
                description: Some("The ID of the tracked item to update".to_string()),
                ..Default::default()
            }),
        );
        properties.insert(
            "action".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Action to take: 'complete' (user handled it) or 'dismiss' (not relevant)"
                        .to_string(),
                ),
                enum_values: Some(vec!["complete".to_string(), "dismiss".to_string()]),
                ..Default::default()
            }),
        );

        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: "update_tracked_item".to_string(),
                description: Some(
                    "Mark a tracked item as completed or dismissed. Use when the user says they paid a bill, received a package, handled a deadline, etc."
                        .to_string(),
                ),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(properties),
                    required: Some(vec!["item_id".to_string(), "action".to_string()]),
                },
            },
        }
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing update_tracked_item tool call");

        let args: UpdateTrackedArgs =
            serde_json::from_str(ctx.arguments).map_err(|e| e.to_string())?;

        match args.action.as_str() {
            "complete" | "dismiss" => {}
            other => return Err(format!("Unknown action: {}", other)),
        };

        let deleted = ctx
            .state
            .item_repository
            .delete_item(args.item_id, ctx.user_id)
            .map_err(|e| format!("Failed to delete item: {}", e))?;

        if deleted {
            Ok(ToolResult::Answer(format!(
                "Item {} {}.",
                args.item_id, args.action
            )))
        } else {
            Ok(ToolResult::Answer(format!(
                "Item {} not found or already handled.",
                args.item_id
            )))
        }
    }
}
