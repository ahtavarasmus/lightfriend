use openai_api_rs::v1::{chat_completion, types};
use std::collections::HashMap;

use crate::api::twilio_sms::TwilioResponse;
use crate::models::ontology_models::NewOntEvent;
use crate::tools::registry::{write_outgoing_history, ToolContext, ToolHandler, ToolResult};
use axum::http::StatusCode;

fn direct_reminder_response(ctx: &ToolContext<'_>, message: String) -> ToolResult {
    write_outgoing_history(
        ctx.state,
        ctx.user_id,
        "set_reminder",
        &ctx.tool_call_id,
        &message,
        ctx.current_time,
    );
    ToolResult::EarlyReturn {
        response: TwilioResponse {
            message,
            created_item_id: None,
        },
        status: StatusCode::OK,
    }
}

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
                    "Exact local wall-clock time in the user's stored timezone, as ISO datetime without an offset (e.g. '2026-03-19T14:30'). Ask for clarification instead of guessing a date or time.".to_string(),
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

        let timezone = match crate::proactive::utils::reminder_timezone(ctx.state, ctx.user_id, now)
        {
            Ok(timezone) => timezone,
            Err(question) => return Ok(direct_reminder_response(&ctx, question)),
        };
        let remind_at =
            match crate::proactive::utils::parse_reminder_time_in_zone(&when, &timezone, now) {
                Ok(remind_at) => remind_at,
                Err(question) => return Ok(direct_reminder_response(&ctx, question)),
            };

        let new_event = NewOntEvent {
            user_id: ctx.user_id,
            description: message.clone(),
            remind_at: Some(remind_at as i32),
            due_at: Some(remind_at as i32),
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        };

        let created = ctx
            .state
            .ontology_repository
            .create_reminder(&new_event, &timezone)
            .map_err(|e| format!("Failed to create reminder: {}", e))?;
        // Re-read the row used as source of truth for the direct confirmation.
        let persisted = ctx
            .state
            .ontology_repository
            .get_event(ctx.user_id, created.id)
            .map_err(|e| format!("Failed to verify persisted reminder: {}", e))?;
        let persisted_at = persisted
            .remind_at
            .ok_or_else(|| "Persisted reminder has no time".to_string())?;
        let persisted_zone = persisted
            .reminder_timezone
            .as_deref()
            .ok_or_else(|| "Persisted reminder has no timezone".to_string())?;
        let rendered =
            crate::proactive::utils::format_persisted_local_time(persisted_at, persisted_zone)?;
        Ok(direct_reminder_response(
            &ctx,
            format!(
                "Reminder '{}' persisted for {} (event id={}).",
                name, rendered, persisted.id
            ),
        ))
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
        let now = chrono::Utc::now().timestamp() as i32;
        let schedule_changed = args["remind_at"].is_string() || args["due_at"].is_string();
        let timezone = if schedule_changed {
            Some(crate::proactive::utils::reminder_timezone(
                ctx.state,
                ctx.user_id,
                now,
            )?)
        } else {
            None
        };
        let remind_at = args["remind_at"]
            .as_str()
            .map(|value| {
                crate::proactive::utils::parse_reminder_time_in_zone(
                    value,
                    timezone.as_deref().expect("timezone loaded"),
                    now,
                )
            })
            .transpose()?;
        let due_at = args["due_at"]
            .as_str()
            .map(|value| {
                crate::proactive::utils::parse_reminder_time_in_zone(
                    value,
                    timezone.as_deref().expect("timezone loaded"),
                    now,
                )
            })
            .transpose()?;

        let mut event = ctx
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
        if remind_at.is_some() {
            event = ctx
                .state
                .ontology_repository
                .reset_event_reminder_delivery(
                    ctx.user_id,
                    event_id,
                    timezone.as_deref().expect("timezone loaded"),
                )
                .map_err(|e| format!("Failed to persist reminder timezone: {}", e))?;
        }

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

        if let (Some(remind_at), Some(timezone)) =
            (event.remind_at, event.reminder_timezone.as_deref())
        {
            let rendered =
                crate::proactive::utils::format_persisted_local_time(remind_at, timezone)?;
            Ok(direct_reminder_response(
                &ctx,
                format!(
                    "Event {} updated. Persisted reminder: {}. Status: {}, description: '{}'.",
                    event.id, rendered, event.status, event.description
                ),
            ))
        } else {
            Ok(ToolResult::Answer(format!(
                "Event {} updated. Status: {}, description: '{}'.",
                event.id, event.status, event.description
            )))
        }
    }
}
