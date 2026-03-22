use openai_api_rs::v1::{chat_completion, types};
use std::collections::HashMap;

use crate::models::ontology_models::{NewOntEvent, NewOntRule};
use crate::proactive::rules::{compute_next_fire_at, ActionConfig, TriggerConfig};
use crate::tools::registry::{ToolContext, ToolHandler, ToolResult};
use crate::UserCoreOps;

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
                    "When to fire. ISO datetime for one-time (e.g. '2026-03-19T14:30'), or pattern for recurring: 'daily HH:MM', 'weekdays HH:MM', 'weekly DAY HH:MM', 'hourly'."
                        .to_string(),
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
                    "Set a reminder or one-time notification. Use for: 'remind me to X', 'wake me at Y', 'notify me at Z'. For recurring schedules, event triggers, or complex automations use create_rule instead."
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

        // Auto-detect once vs recurring
        let recurring_keywords = ["daily", "weekdays", "weekly", "hourly", "every"];
        let is_recurring = recurring_keywords
            .iter()
            .any(|kw| when.to_lowercase().starts_with(kw));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32;

        if !is_recurring {
            // One-time reminder: create an Event with notify_at == expires_at
            let notify_at = crate::proactive::utils::parse_iso_to_timestamp(&when)
                .ok_or_else(|| {
                    format!(
                        "Invalid one-time reminder timestamp '{}'. Use an ISO datetime.",
                        when
                    )
                })?;

            let new_event = NewOntEvent {
                user_id: ctx.user_id,
                description: message.clone(),
                notify_at: Some(notify_at as i32),
                expires_at: Some(notify_at as i32),
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
            };

            match ctx.state.ontology_repository.create_event(&new_event) {
                Ok(event) => Ok(ToolResult::Answer(format!(
                    "Reminder '{}' set (event id={}). Will fire once.",
                    name, event.id,
                ))),
                Err(e) => Err(format!("Failed to create reminder: {}", e)),
            }
        } else {
            // Recurring reminder: create a Rule as before
            let trigger_config_str = serde_json::json!({
                "schedule": "recurring",
                "pattern": when
            })
            .to_string();

            let action_config_str = serde_json::json!({
                "method": "sms",
                "message": message
            })
            .to_string();

            let next_fire_at = {
                let trigger: TriggerConfig = serde_json::from_str(&trigger_config_str).unwrap();
                if let Some(ref pattern) = trigger.pattern {
                    let user_tz = ctx
                        .state
                        .user_core
                        .get_user_info(ctx.user_id)
                        .ok()
                        .and_then(|info| info.timezone)
                        .unwrap_or_else(|| "UTC".to_string());
                    compute_next_fire_at(pattern, &user_tz)
                } else {
                    None
                }
            };

            let flow_config = serde_json::json!({
                "type": "action",
                "action_type": "notify",
                "config": serde_json::from_str::<serde_json::Value>(&action_config_str).unwrap_or_default()
            });

            let new_rule = NewOntRule {
                user_id: ctx.user_id,
                name: name.clone(),
                trigger_type: "schedule".to_string(),
                trigger_config: trigger_config_str,
                logic_type: "passthrough".to_string(),
                logic_prompt: None,
                logic_fetch: None,
                action_type: "notify".to_string(),
                action_config: action_config_str,
                status: "active".to_string(),
                next_fire_at,
                expires_at: None,
                created_at: now,
                updated_at: now,
                flow_config: Some(flow_config.to_string()),
            };

            match ctx.state.ontology_repository.create_rule(&new_rule) {
                Ok(rule) => Ok(ToolResult::AnswerWithTask {
                    answer: format!(
                        "Recurring reminder '{}' set (rule id={}). Pattern: {}",
                        name, rule.id, when
                    ),
                    task_id: rule.id,
                }),
                Err(e) => Err(format!("Failed to create reminder: {}", e)),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CreateRuleHandler - complex automations with triggers, logic, and actions
// ---------------------------------------------------------------------------

pub struct CreateRuleHandler;

#[async_trait::async_trait]
impl ToolHandler for CreateRuleHandler {
    fn name(&self) -> &'static str {
        "create_rule"
    }

    fn definition(&self) -> chat_completion::Tool {
        let mut properties = HashMap::new();

        properties.insert(
            "name".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Short descriptive name for the rule (e.g. 'Track Amazon package', 'Daily email briefing', 'Remind about meeting')."
                        .to_string(),
                ),
                ..Default::default()
            }),
        );

        properties.insert(
            "trigger_type".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "What triggers this rule. 'schedule' for time-based (reminders, recurring). 'ontology_change' for event-based (when a message arrives, when something changes)."
                        .to_string(),
                ),
                enum_values: Some(vec![
                    "schedule".to_string(),
                    "ontology_change".to_string(),
                ]),
                ..Default::default()
            }),
        );

        properties.insert(
            "trigger_config".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    r#"JSON trigger configuration.
For schedule once: {"schedule":"once","at":"YYYY-MM-DDTHH:MM"} (datetime in user's timezone)
For schedule recurring: {"schedule":"recurring","pattern":"daily HH:MM"} or "weekdays HH:MM" or "weekly DAY HH:MM" or "hourly"
For ontology_change: {"entity_type":"Message","change":"created","filters":{"sender":"name","topic":"keyword"}} - filters are optional, use substring matching"#
                        .to_string(),
                ),
                ..Default::default()
            }),
        );

        properties.insert(
            "logic_type".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "'passthrough' for simple reminders/notifications (no LLM evaluation needed). 'llm' when the trigger needs intelligent evaluation (e.g. checking if a condition is met, summarizing content)."
                        .to_string(),
                ),
                enum_values: Some(vec![
                    "passthrough".to_string(),
                    "llm".to_string(),
                ]),
                ..Default::default()
            }),
        );

        properties.insert(
            "logic_prompt".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Instructions for the LLM evaluator (only for logic_type='llm'). Describe what to check or summarize. E.g. 'Check if this message is about a package delay' or 'Summarize the user\\'s recent emails into a brief morning briefing'."
                        .to_string(),
                ),
                ..Default::default()
            }),
        );

        properties.insert(
            "logic_fetch".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Comma-separated data sources to pre-fetch for LLM context. Options: email, chat, internet. Only needed for logic_type='llm'. E.g. 'email' or 'email,chat'."
                        .to_string(),
                ),
                ..Default::default()
            }),
        );

        properties.insert(
            "action_type".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "'notify' to send SMS/call to user. 'tool_call' to execute a specific tool."
                        .to_string(),
                ),
                enum_values: Some(vec!["notify".to_string(), "tool_call".to_string()]),
                ..Default::default()
            }),
        );

        properties.insert(
            "action_config".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    r#"JSON action configuration.
For notify: {"method":"sms"} or {"method":"call"} or {"method":"sms","message":"fixed message text"}
For tool_call: {"tool":"tool_name","params":{...}}"#
                        .to_string(),
                ),
                ..Default::default()
            }),
        );

        properties.insert(
            "expires_in_days".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::Number),
                description: Some(
                    "Optional: number of days until this rule expires. Use for tracking rules that have a natural end date. Omit for permanent rules like recurring briefings."
                        .to_string(),
                ),
                ..Default::default()
            }),
        );

        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: "create_rule".to_string(),
                description: Some(
                    "Create a complex automation rule with event triggers, AI logic, or tool actions. For simple reminders and notifications, use set_reminder instead."
                        .to_string(),
                ),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(properties),
                    required: Some(vec![
                        "name".to_string(),
                        "trigger_type".to_string(),
                        "trigger_config".to_string(),
                        "logic_type".to_string(),
                        "action_type".to_string(),
                        "action_config".to_string(),
                    ]),
                },
            },
        }
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        let args: serde_json::Value =
            serde_json::from_str(ctx.arguments).map_err(|e| format!("Invalid JSON: {}", e))?;

        let name = args["name"].as_str().unwrap_or("Unnamed rule").to_string();
        let trigger_type = args["trigger_type"]
            .as_str()
            .unwrap_or("schedule")
            .to_string();
        let trigger_config_str = args["trigger_config"].as_str().unwrap_or("{}").to_string();
        let logic_type = args["logic_type"]
            .as_str()
            .unwrap_or("passthrough")
            .to_string();
        let logic_prompt = args["logic_prompt"].as_str().map(|s| s.to_string());
        let logic_fetch = args["logic_fetch"].as_str().map(|s| s.to_string());
        let action_type = args["action_type"].as_str().unwrap_or("notify").to_string();
        let action_config_str = args["action_config"]
            .as_str()
            .unwrap_or(r#"{"method":"sms"}"#)
            .to_string();
        let expires_in_days = args["expires_in_days"].as_f64();

        // Validate trigger_config is valid JSON
        let _trigger: TriggerConfig = serde_json::from_str(&trigger_config_str)
            .map_err(|e| format!("Invalid trigger_config JSON: {}", e))?;

        // Validate action_config is valid JSON
        let _action: ActionConfig = serde_json::from_str(&action_config_str)
            .map_err(|e| format!("Invalid action_config JSON: {}", e))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32;

        // Compute next_fire_at for schedule rules
        let next_fire_at = if trigger_type == "schedule" {
            let trigger: TriggerConfig = serde_json::from_str(&trigger_config_str).unwrap();
            match trigger.schedule.as_deref() {
                Some("once") => {
                    // Parse the "at" field as ISO datetime in user timezone
                    trigger
                        .at
                        .as_ref()
                        .and_then(|at| crate::proactive::utils::parse_iso_to_timestamp(at))
                }
                Some("recurring") => {
                    // Compute next occurrence from pattern
                    if let Some(ref pattern) = trigger.pattern {
                        // Get user timezone from user_info
                        let user_tz = ctx
                            .state
                            .user_core
                            .get_user_info(ctx.user_id)
                            .ok()
                            .and_then(|info| info.timezone)
                            .unwrap_or_else(|| "UTC".to_string());
                        compute_next_fire_at(pattern, &user_tz)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        // Compute expiry
        let expires_at = expires_in_days.map(|days| now + (days * 86400.0) as i32);

        // Build flow_config from logic + action
        let action_json =
            serde_json::from_str::<serde_json::Value>(&action_config_str).unwrap_or_default();
        let action_node = serde_json::json!({
            "type": "action",
            "action_type": &action_type,
            "config": action_json
        });
        let flow_config = match logic_type.as_str() {
            "llm" => {
                let fetch_sources: Vec<serde_json::Value> = logic_fetch
                    .as_deref()
                    .and_then(|f| serde_json::from_str(f).ok())
                    .unwrap_or_default();
                serde_json::json!({
                    "type": "llm_condition",
                    "prompt": logic_prompt.as_deref().unwrap_or("Evaluate and decide"),
                    "fetch": fetch_sources,
                    "true_branch": action_node,
                    "false_branch": null
                })
            }
            "keyword" => {
                serde_json::json!({
                    "type": "keyword_condition",
                    "keyword": logic_prompt.as_deref().unwrap_or(""),
                    "true_branch": action_node,
                    "false_branch": null
                })
            }
            _ => action_node, // passthrough: just the action at root
        };

        let new_rule = NewOntRule {
            user_id: ctx.user_id,
            name: name.clone(),
            trigger_type,
            trigger_config: trigger_config_str,
            logic_type,
            logic_prompt,
            logic_fetch,
            action_type,
            action_config: action_config_str,
            status: "active".to_string(),
            next_fire_at,
            expires_at,
            created_at: now,
            updated_at: now,
            flow_config: Some(flow_config.to_string()),
        };

        match ctx.state.ontology_repository.create_rule(&new_rule) {
            Ok(rule) => Ok(ToolResult::AnswerWithTask {
                answer: format!("Rule '{}' created (id={}). Status: active.", name, rule.id),
                task_id: rule.id,
            }),
            Err(e) => Err(format!("Failed to create rule: {}", e)),
        }
    }
}

// ---------------------------------------------------------------------------
// CreateEventHandler - creates a tracked event linked to the triggering message
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
                    "Short description of what to track (e.g. 'Amazon package delivery', 'Invoice payment due')."
                        .to_string(),
                ),
                ..Default::default()
            }),
        );
        properties.insert(
            "notify_at_days".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::Number),
                description: Some("Days from now until notification. Default 7.".to_string()),
                ..Default::default()
            }),
        );
        properties.insert(
            "expires_at_days".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::Number),
                description: Some(
                    "Days from now until expiration. Defaults to notify_at_days.".to_string(),
                ),
                ..Default::default()
            }),
        );

        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: "create_event".to_string(),
                description: Some(
                    "Create a tracked event on the user's dashboard. Links to the triggering message."
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

        let notify_at_days = args["notify_at_days"].as_i64().unwrap_or(7).clamp(1, 90) as i32;
        let expires_at_days = args["expires_at_days"]
            .as_i64()
            .unwrap_or(notify_at_days as i64)
            .clamp(1, 90) as i32;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32;

        let new_event = NewOntEvent {
            user_id: ctx.user_id,
            description: description.clone(),
            notify_at: Some(now + notify_at_days * 86400),
            expires_at: Some(now + expires_at_days * 86400),
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
            "Event '{}' created (id={}). Notify in {} days, expires in {} days.",
            description, event.id, notify_at_days, expires_at_days
        )))
    }
}

// ---------------------------------------------------------------------------
// UpdateEventHandler - updates a tracked event's status/description
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
            "description".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some("Updated description for the event".to_string()),
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
            "extend_days".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::Number),
                description: Some("Extend deadline by this many days from now".to_string()),
                ..Default::default()
            }),
        );

        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: "update_event".to_string(),
                description: Some(
                    "Update a tracked event's status, description, or deadline.".to_string(),
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
        let description = args["description"].as_str();
        let status = args["status"].as_str();
        let extend_days = args["extend_days"].as_i64().map(|d| d as i32);

        let event = ctx
            .state
            .ontology_repository
            .update_event(ctx.user_id, event_id, description, status, extend_days)
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
