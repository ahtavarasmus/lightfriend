use openai_api_rs::v1::{chat_completion, types};
use std::collections::HashMap;

use crate::models::ontology_models::NewOntEvent;
use crate::tools::registry::{ToolContext, ToolHandler, ToolResult};

// ---------------------------------------------------------------------------
// SetReminderHandler - simple facade for reminders/notifications
// ---------------------------------------------------------------------------

pub struct SetReminderHandler;

#[async_trait::async_trait]
impl ToolHandler for SetReminderHandler {
    fn name(&self) -> &'static str {
        "set_reminder"
    }

    fn definition(&self) -> chat_completion::Tool {
        let mut properties = HashMap::new();

        properties.insert(
            "name".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Short name for the reminder (e.g. 'Take medication', 'Call dentist', 'Team standup')."
                        .to_string(),
                ),
                ..Default::default()
            }),
        );

        properties.insert(
            "when".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "When to fire. ISO datetime (e.g. '2026-03-19T14:30').".to_string(),
                ),
                ..Default::default()
            }),
        );

        properties.insert(
            "message".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Custom notification text. Defaults to the name if omitted.".to_string(),
                ),
                ..Default::default()
            }),
        );

        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: "set_reminder".to_string(),
                description: Some(
                    "Set a one-time reminder or notification. Use for: 'remind me to X', 'wake me at Y', 'notify me at Z'. For recurring reminders or complex rules, tell the user to set them from the dashboard rule builder."
                        .to_string(),
                ),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(properties),
                    required: Some(vec![
                        "name".to_string(),
                        "when".to_string(),
                    ]),
                },
            },
        }
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        let args: serde_json::Value =
            serde_json::from_str(ctx.arguments).map_err(|e| format!("Invalid JSON: {}", e))?;

        let name = args["name"].as_str().unwrap_or("Reminder").to_string();
        let when = args["when"].as_str().unwrap_or("").to_string();
        let message = args["message"].as_str().unwrap_or(&name).to_string();

        if when.is_empty() {
            return Err("'when' parameter is required".to_string());
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32;

        let tz_offset = crate::proactive::utils::user_tz_offset_secs(ctx.state, ctx.user_id);
        let remind_at = crate::proactive::utils::parse_iso_to_timestamp(&when, tz_offset)
            .ok_or_else(|| {
                format!(
                    "Invalid reminder timestamp '{}'. Use an ISO datetime like '2026-03-19T14:30'.",
                    when
                )
            })?;

        let new_event = NewOntEvent {
            user_id: ctx.user_id,
            description: message.clone(),
            remind_at: Some(remind_at as i32),
            due_at: Some(remind_at as i32),
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        };

        match ctx.state.ontology_repository.create_event(&new_event) {
            Ok(event) => Ok(ToolResult::Answer(format!(
                "Reminder '{}' set (event id={}). Will fire at the scheduled time.",
                name, event.id,
            ))),
            Err(e) => Err(format!("Failed to create reminder: {}", e)),
        }
    }
}

// ---------------------------------------------------------------------------
// CreateEventHandler - creates a tracked obligation linked to the triggering message
// ---------------------------------------------------------------------------

pub struct CreateEventHandler;

#[async_trait::async_trait]
impl ToolHandler for CreateEventHandler {
    fn name(&self) -> &'static str {
        "create_event"
    }

    fn auto_injected_params(&self) -> Vec<&'static str> {
        vec!["message_id"]
    }

    fn definition(&self) -> chat_completion::Tool {
        let mut properties = HashMap::new();
        properties.insert(
            "message_id".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::Number),
                description: Some(
                    "ID of the triggering message (auto-injected by rules)".to_string(),
                ),
                ..Default::default()
            }),
        );
        properties.insert(
            "description".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Short description of one concrete obligation to track, not a whole situation (e.g. 'Pay hotel deposit', 'Confirm train tickets', 'Invoice payment due')."
                        .to_string(),
                ),
                ..Default::default()
            }),
        );
        properties.insert(
            "remind_at_days".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::Number),
                description: Some(
                    "Days from now for the best reminder time. Default 7.".to_string(),
                ),
                ..Default::default()
            }),
        );
        properties.insert(
            "due_at_days".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::Number),
                description: Some(
                    "Days from now until the real deadline or last useful action time. Defaults to remind_at_days.".to_string(),
                ),
                ..Default::default()
            }),
        );

        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: "create_event".to_string(),
                description: Some(
                    "Create a tracked obligation on the user's dashboard for one concrete commitment. Links to the triggering message."
                        .to_string(),
                ),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(properties),
                    required: Some(vec![
                        "message_id".to_string(),
                        "description".to_string(),
                    ]),
                },
            },
        }
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        let args: serde_json::Value =
            serde_json::from_str(ctx.arguments).map_err(|e| format!("Invalid JSON: {}", e))?;

        let message_id = args["message_id"]
            .as_i64()
            .ok_or_else(|| "message_id is required".to_string())?;
        let description = args["description"]
            .as_str()
            .ok_or_else(|| "description is required".to_string())?
            .to_string();

        let remind_at_days = args["remind_at_days"].as_i64().unwrap_or(7).clamp(1, 90) as i32;
        let due_at_days = args["due_at_days"]
            .as_i64()
            .unwrap_or(remind_at_days as i64)
            .clamp(1, 90) as i32;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32;

        let new_event = NewOntEvent {
            user_id: ctx.user_id,
            description: description.clone(),
            remind_at: Some(now + remind_at_days * 86400),
            due_at: Some(now + due_at_days * 86400),
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        };

        let event = ctx
            .state
            .ontology_repository
            .create_event(&new_event)
            .map_err(|e| format!("Failed to create event: {}", e))?;

        // Link message to event
        let _ = ctx.state.ontology_repository.create_link(
            ctx.user_id,
            "Message",
            message_id as i32,
            "Event",
            event.id,
            "triggers",
            None,
        );

        Ok(ToolResult::Answer(format!(
            "Event '{}' created (id={}). Remind in {} days, due in {} days.",
            description, event.id, remind_at_days, due_at_days
        )))
    }
}

// ---------------------------------------------------------------------------
// UpdateEventHandler - updates a tracked obligation
// ---------------------------------------------------------------------------

pub struct UpdateEventHandler;

#[async_trait::async_trait]
impl ToolHandler for UpdateEventHandler {
    fn name(&self) -> &'static str {
        "update_event"
    }

    fn auto_injected_params(&self) -> Vec<&'static str> {
        vec!["message_id"]
    }

    fn definition(&self) -> chat_completion::Tool {
        let mut properties = HashMap::new();
        properties.insert(
            "event_id".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::Number),
                description: Some("ID of the event to update".to_string()),
                ..Default::default()
            }),
        );
        properties.insert(
            "message_id".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::Number),
                description: Some(
                    "ID of the triggering message (auto-injected by rules)".to_string(),
                ),
                ..Default::default()
            }),
        );
        properties.insert(
            "append_description".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Append this update text to the event description. Keep the original context and add only the new concrete change."
                        .to_string(),
                ),
                ..Default::default()
            }),
        );
        properties.insert(
            "status".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some("New status for the event".to_string()),
                enum_values: Some(vec![
                    "active".to_string(),
                    "completed".to_string(),
                    "dismissed".to_string(),
                ]),
                ..Default::default()
            }),
        );
        properties.insert(
            "remind_at".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Replace remind_at with this ISO datetime when the best reminder time changes."
                        .to_string(),
                ),
                ..Default::default()
            }),
        );
        properties.insert(
            "due_at".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Replace due_at with this ISO datetime when the actual deadline or last useful action time changes."
                        .to_string(),
                ),
                ..Default::default()
            }),
        );

        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: "update_event".to_string(),
                description: Some(
                    "Append new context to a tracked obligation and optionally update its status, reminder time, or due time."
                        .to_string(),
                ),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(properties),
                    required: Some(vec!["event_id".to_string()]),
                },
            },
        }
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        let args: serde_json::Value =
            serde_json::from_str(ctx.arguments).map_err(|e| format!("Invalid JSON: {}", e))?;

        let event_id = args["event_id"]
            .as_i64()
            .ok_or_else(|| "event_id is required".to_string())? as i32;
        let append_description = args["append_description"].as_str();
        let status = args["status"].as_str();
        let tz_offset = crate::proactive::utils::user_tz_offset_secs(ctx.state, ctx.user_id);
        let remind_at = args["remind_at"]
            .as_str()
            .map(|s| {
                crate::proactive::utils::parse_iso_to_timestamp(s, tz_offset)
                    .ok_or_else(|| "Invalid remind_at timestamp".to_string())
            })
            .transpose()?;
        let due_at = args["due_at"]
            .as_str()
            .map(|s| {
                crate::proactive::utils::parse_iso_to_timestamp(s, tz_offset)
                    .ok_or_else(|| "Invalid due_at timestamp".to_string())
            })
            .transpose()?;

        let event = ctx
            .state
            .ontology_repository
            .update_event(
                ctx.user_id,
                event_id,
                append_description,
                status,
                remind_at,
                due_at,
            )
            .map_err(|e| format!("Failed to update event: {}", e))?;

        // Always link the current message to the event
        if let Some(message_id) = args["message_id"].as_i64() {
            let _ = ctx.state.ontology_repository.create_link(
                ctx.user_id,
                "Message",
                message_id as i32,
                "Event",
                event_id,
                "updates",
                None,
            );
        }

        Ok(ToolResult::Answer(format!(
            "Event {} updated. Status: {}, description: '{}'.",
            event.id, event.status, event.description
        )))
    }
}
