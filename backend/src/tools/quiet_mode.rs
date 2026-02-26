use openai_api_rs::v1::{chat_completion, types};
use serde::Deserialize;
use std::collections::HashMap;

use crate::repositories::user_core::UserCoreOps;
use crate::tools::registry::{ToolContext, ToolHandler, ToolResult};

pub struct QuietModeHandler;

#[derive(Deserialize)]
struct QuietModeArgs {
    action: String, // "enable", "disable", "add_rule"
    duration_minutes: Option<i32>,
    until: Option<String>,     // ISO datetime
    rule_type: Option<String>, // "suppress" or "allow" (for add_rule)
    platform: Option<String>,
    sender: Option<String>,
    topic: Option<String>,
}

#[async_trait::async_trait]
impl ToolHandler for QuietModeHandler {
    fn name(&self) -> &'static str {
        "set_quiet_mode"
    }

    fn definition(&self) -> chat_completion::Tool {
        let mut properties = HashMap::new();
        properties.insert(
            "action".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Action: 'enable' (global quiet), 'disable' (turn off quiet + clear rules), 'add_rule' (add a filtering rule)".to_string(),
                ),
                enum_values: Some(vec![
                    "enable".to_string(),
                    "disable".to_string(),
                    "add_rule".to_string(),
                ]),
                ..Default::default()
            }),
        );
        properties.insert(
            "duration_minutes".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::Number),
                description: Some(
                    "Duration in minutes. For 'enable': omit for indefinite. For 'add_rule': required (no indefinite rules).".to_string(),
                ),
                ..Default::default()
            }),
        );
        properties.insert(
            "until".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Alternative to duration_minutes: ISO datetime string for when quiet/rule expires (e.g. '2024-01-15T17:00:00').".to_string(),
                ),
                ..Default::default()
            }),
        );
        properties.insert(
            "rule_type".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "For add_rule: 'suppress' (block matching notifications) or 'allow' (only allow matching notifications, block rest).".to_string(),
                ),
                enum_values: Some(vec!["suppress".to_string(), "allow".to_string()]),
                ..Default::default()
            }),
        );
        properties.insert(
            "platform".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Filter by platform: whatsapp, telegram, signal, email, calendar, etc."
                        .to_string(),
                ),
                ..Default::default()
            }),
        );
        properties.insert(
            "sender".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Filter by sender name (substring match, case-insensitive).".to_string(),
                ),
                ..Default::default()
            }),
        );
        properties.insert(
            "topic".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Filter by topic/keyword in message content (substring match, case-insensitive).".to_string(),
                ),
                ..Default::default()
            }),
        );

        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: "set_quiet_mode".to_string(),
                description: Some(
                    "Control quiet mode (notification suppression). Can enable global quiet, disable it, or add filtering rules like 'suppress WhatsApp for 2 hours' or 'only notify about Mom's messages until 5pm'.".to_string(),
                ),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(properties),
                    required: Some(vec!["action".to_string()]),
                },
            },
        }
    }

    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String> {
        tracing::debug!("Executing set_quiet_mode tool call");

        let args: QuietModeArgs = serde_json::from_str(ctx.arguments).map_err(|e| e.to_string())?;

        match args.action.as_str() {
            "enable" => {
                // Global quiet mode (no rules)
                let until_ts = resolve_until(&args)?;
                ctx.state
                    .user_core
                    .set_quiet_mode(ctx.user_id, Some(until_ts))
                    .map_err(|e| format!("Failed to enable quiet mode: {}", e))?;

                let msg = if until_ts == 0 {
                    "Quiet mode enabled indefinitely. All notifications suppressed.".to_string()
                } else {
                    format!("Quiet mode enabled until {}.", format_timestamp(until_ts))
                };
                Ok(ToolResult::Answer(msg))
            }
            "disable" => {
                ctx.state
                    .user_core
                    .set_quiet_mode(ctx.user_id, None)
                    .map_err(|e| format!("Failed to disable quiet mode: {}", e))?;

                Ok(ToolResult::Answer(
                    "Quiet mode disabled. All notifications resumed. Any rules have been cleared."
                        .to_string(),
                ))
            }
            "add_rule" => {
                let rule_type = args.rule_type.as_deref().unwrap_or("suppress");

                if rule_type != "suppress" && rule_type != "allow" {
                    return Err(format!(
                        "Invalid rule_type: '{}'. Must be 'suppress' or 'allow'.",
                        rule_type
                    ));
                }

                // Rules always require a duration (no indefinite rules)
                let until_ts = resolve_until(&args)?;
                if until_ts == 0 {
                    return Err(
                        "Rules must have a duration or end time. For indefinite quiet, use action='enable' without duration.".to_string(),
                    );
                }

                // Build description
                let mut desc_parts = Vec::new();
                if let Some(ref p) = args.platform {
                    desc_parts.push(format!("platform={}", p));
                }
                if let Some(ref s) = args.sender {
                    desc_parts.push(format!("sender={}", s));
                }
                if let Some(ref t) = args.topic {
                    desc_parts.push(format!("topic={}", t));
                }
                let description = if desc_parts.is_empty() {
                    format!("Quiet rule: {} all", rule_type)
                } else {
                    format!("Quiet rule: {} {}", rule_type, desc_parts.join(", "))
                };

                let rule_id = ctx
                    .state
                    .user_core
                    .add_quiet_rule(
                        ctx.user_id,
                        until_ts,
                        rule_type,
                        args.platform.as_deref(),
                        args.sender.as_deref(),
                        args.topic.as_deref(),
                        &description,
                    )
                    .map_err(|e| format!("Failed to add quiet rule: {}", e))?;

                let msg = format!(
                    "Quiet rule added (id={}): {} until {}. {}",
                    rule_id,
                    rule_type,
                    format_timestamp(until_ts),
                    description,
                );
                Ok(ToolResult::Answer(msg))
            }
            other => Err(format!(
                "Unknown action: '{}'. Use 'enable', 'disable', or 'add_rule'.",
                other
            )),
        }
    }
}

/// Resolve the until timestamp from args. Returns 0 for indefinite.
fn resolve_until(args: &QuietModeArgs) -> Result<i32, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    if let Some(minutes) = args.duration_minutes {
        if minutes <= 0 {
            return Err("duration_minutes must be positive".to_string());
        }
        return Ok(now + minutes * 60);
    }

    if let Some(ref until_str) = args.until {
        // Try parsing ISO datetime
        let ts = chrono::NaiveDateTime::parse_from_str(until_str, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(until_str, "%Y-%m-%d %H:%M:%S"))
            .map(|dt| dt.and_utc().timestamp() as i32)
            .map_err(|e| format!("Invalid datetime '{}': {}", until_str, e))?;

        if ts <= now {
            return Err("until time must be in the future".to_string());
        }
        return Ok(ts);
    }

    // No duration specified - indefinite (0)
    Ok(0)
}

fn format_timestamp(ts: i32) -> String {
    if ts == 0 {
        return "indefinitely".to_string();
    }
    chrono::DateTime::from_timestamp(ts as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_else(|| format!("timestamp {}", ts))
}
