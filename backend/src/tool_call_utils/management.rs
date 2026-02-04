use crate::UserCoreOps;
pub fn get_update_monitoring_status_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut proactive_agent_properties = HashMap::new();
    proactive_agent_properties.insert(
        "enabled".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "Set to true to turn the notifications on, false to turn it off.".to_string(),
            ),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("update_notifications_status"),
            description: Some(String::from(
                "Turns the notifications system on or off globally for the user's incoming messages. Use this tool when the user explicitly requests to enable or disable all monitoring or notifications entirely, such as 'turn on notifications' or 'turn off notifications'. Do not use this for setting up notifications/monitoring for specific content; instead, use the create_waiting_check tool for targeted notifications of particular events or content in incoming messages or emails.",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(proactive_agent_properties),
                required: Some(vec![String::from("enabled")]),
            },
        },
    }
}

#[derive(Deserialize)]
pub struct ProactiveAgentArgs {
    pub enabled: bool,
}

pub async fn handle_set_proactive_agent(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> Result<String, Box<dyn Error>> {
    let args: ProactiveAgentArgs = serde_json::from_str(args)?;

    // Assuming there's a method to update the proactive agent status for the user
    state
        .user_core
        .update_proactive_agent_on(user_id, args.enabled)
        .map_err(|e| Box::new(e) as Box<dyn Error>)?;

    let status = if args.enabled { "on" } else { "off" };
    Ok(format!("Proactive agent turned {}.", status))
}

/// Unified task creation tool for scheduled reminders, actions, and message monitoring.
/// This replaces the old waiting_check tool and adds support for time-triggered tasks.
pub fn get_create_task_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut task_properties = HashMap::new();

    // trigger_type: enum for type of trigger
    task_properties.insert(
        "trigger_type".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "How the task should be triggered. Options:\n\
                - \"once\": Execute at a specific time (requires trigger_time)\n\
                - \"recurring_email\": Check each incoming email until condition is met\n\
                - \"recurring_messaging\": Check each incoming message (WhatsApp, Telegram, etc.) until condition is met".to_string()
            ),
            enum_values: Some(vec![
                "once".to_string(),
                "recurring_email".to_string(),
                "recurring_messaging".to_string(),
            ]),
            ..Default::default()
        }),
    );

    // trigger_time: ISO datetime for once triggers
    task_properties.insert(
        "trigger_time".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "When to execute the task. Required for trigger_type='once'. \
                Format: ISO datetime 'YYYY-MM-DDTHH:MM' in the user's timezone. \
                Convert relative times like 'in 3 hours', 'tomorrow at 9am', 'this afternoon at 2pm' \
                to this format based on current time.".to_string()
            ),
            ..Default::default()
        }),
    );

    // condition: optional natural language condition
    task_properties.insert(
        "condition".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Optional condition to check before executing the action. \
                Use natural language that will be evaluated by AI at execution time.\n\
                Examples:\n\
                - 'about job application'\n\
                - 'from mom'\n\
                - 'if mom hasn't replied since I asked'\n\
                - 'contains a link'\n\
                - 'meeting rescheduled'\n\
                For recurring_email/recurring_messaging: this is what to look for in incoming messages.\n\
                For once: this is an optional condition to check before executing the action.".to_string()
            ),
            ..Default::default()
        }),
    );

    // action_spec: detailed step-by-step instructions for runtime AI
    // Get the dynamic tools list
    let tools_prompt = crate::tool_call_utils::utils::get_runtime_tools_prompt();

    task_properties.insert(
        "action_spec".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(format!(
                "The action to execute when triggered. Use tool call format.\n\n\
                CRITICAL - REMINDER vs ACTION:\n\
                - User says 'remind me to X' -> use send_reminder(X) - just NOTIFY, don't do X\n\
                - User says 'do X' or 'turn on X' -> use control_tesla(X) - actually EXECUTE X\n\n\
                LANGUAGE: Always use third person ('the user'), NEVER 'you'. These are AI instructions.\n\n\
                {}\n\
                CORRECT EXAMPLES:\n\
                - 'call me at midnight' -> send_reminder(Scheduled check-in)\n\
                - 'remind me to turn on tesla' -> send_reminder(Turn on Tesla climate)\n\
                - 'turn on tesla climate' -> control_tesla(climate_on)\n\
                - 'remind me to call mom' -> send_reminder(Call mom)\n\
                - 'remind me about the meeting' -> send_reminder(Meeting reminder)\n\
                - 'remind me to lock my tesla' -> send_reminder(Lock the Tesla)\n\n\
                WRONG (don't do this):\n\
                - send_reminder(Call you) - WRONG! Never use 'you', use descriptive text like 'Scheduled check-in'\n\
                - 'remind me to X' -> control_tesla(...) WRONG! User wants reminder, not action",
                tools_prompt
            )),
            ..Default::default()
        }),
    );

    // notification_type: how to notify
    task_properties.insert(
        "notification_type".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "How to notify the user. If not specified, uses user's default preference from settings. \
                Only set this if user explicitly asks for a specific method (e.g., 'call me', 'text me'). \
                Options: 'sms', 'call', or 'call_sms' (loud phone ring + SMS backup, only charged for call if answered).".to_string()
            ),
            enum_values: Some(vec!["sms".to_string(), "call".to_string(), "call_sms".to_string()]),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("create_task"),
            description: Some(String::from(
                "Creates a scheduled task or sets up monitoring for future events.\n\n\
                NOTIFICATION METHOD (how to notify user):\n\
                - 'call me at X' = notify via phone CALL at X -> send_reminder(Scheduled check-in) + notification_type='call'\n\
                - 'text me at X' = notify via SMS at X -> send_reminder(...) + notification_type='sms'\n\
                - Default is SMS if not specified\n\n\
                REMINDER vs ACTION (what to do):\n\
                - 'remind me to X' = ONLY notify about X (use send_reminder) - user does X themselves\n\
                - 'do X' / 'turn on X' = EXECUTE action automatically (use control_tesla, etc.)\n\n\
                LANGUAGE: Always use 'the user' (third person), NEVER 'you' or 'me'. These are instructions for an AI.\n\n\
                EXAMPLES:\n\
                - 'call me at midnight' -> send_reminder(Scheduled check-in) + notification_type='call'\n\
                - 'remind me at 10pm to turn on tesla' -> send_reminder(Turn on Tesla climate)\n\
                - 'turn on tesla at 10pm' -> control_tesla(climate_on)\n\
                - 'text me at 3pm about the meeting' -> send_reminder(Meeting reminder) + notification_type='sms'\n\n\
                USE CASES:\n\
                1. Phone call check-in: 'call me at midnight' -> send_reminder + notification_type='call'\n\
                2. Reminders: 'remind me at 1pm about the package' -> send_reminder\n\
                3. Scheduled actions: 'turn on tesla in 3 hours' -> control_tesla\n\
                4. Message monitoring: 'notify me when mom messages'\n\n\
                For scheduled tasks, use trigger_type='once' with trigger_time.",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(task_properties),
                required: Some(vec![
                    String::from("trigger_type"),
                    String::from("action_spec"),
                ]),
            },
        },
    }
}

use crate::AppState;
use chrono_tz::Tz;
use serde::Deserialize;
use std::error::Error;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct CreateTaskArgs {
    pub trigger_type: String, // "once" | "recurring_email" | "recurring_messaging"
    pub trigger_time: Option<String>, // "2025-12-30T13:00" (required for "once")
    pub condition: Option<String>,
    pub action_spec: String, // Detailed step-by-step instructions for runtime AI
    pub notification_type: Option<String>,
}

pub async fn handle_create_task(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> Result<String, Box<dyn Error>> {
    let args: CreateTaskArgs = serde_json::from_str(args)?;

    // Build trigger field
    let trigger = match args.trigger_type.as_str() {
        "once" => {
            let time_str = args
                .trigger_time
                .as_ref()
                .ok_or("trigger_time is required for 'once' trigger type")?;

            // Get user's timezone
            let user_info = state
                .user_core
                .get_user_info(user_id)
                .map_err(|e| format!("Failed to get user info: {:?}", e))?;
            let tz_str = user_info.timezone.unwrap_or_else(|| "UTC".to_string());
            let tz: Tz = tz_str.parse().unwrap_or(chrono_tz::UTC);

            // Parse the datetime in user's timezone and convert to UTC timestamp
            let timestamp = parse_datetime_to_timestamp(time_str, &tz)?;
            format!("once_{}", timestamp)
        }
        "recurring_email" => "recurring_email".to_string(),
        "recurring_messaging" => "recurring_messaging".to_string(),
        _ => return Err(format!("Invalid trigger_type: {}", args.trigger_type).into()),
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Get user's default notification type from settings
    let user_settings = state
        .user_core
        .get_user_settings(user_id)
        .map_err(|e| format!("Failed to get user settings: {:?}", e))?;
    let default_noti_type = user_settings
        .notification_type
        .unwrap_or_else(|| "sms".to_string());

    let new_task = crate::models::user_models::NewTask {
        user_id,
        trigger: trigger.clone(),
        condition: args.condition.clone(),
        action: args.action_spec.clone(),
        notification_type: args.notification_type.or(Some(default_noti_type)),
        status: "active".to_string(),
        created_at: now,
        is_permanent: None,
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
    };

    state
        .user_repository
        .create_task(&new_task)
        .map_err(|e| format!("Failed to create task: {:?}", e))?;

    // Build confirmation message (use a short summary, not full action_spec)
    let action_summary = if args.action_spec.len() > 50 {
        format!("{}...", &args.action_spec[..50])
    } else {
        args.action_spec.clone()
    };

    let confirmation = match args.trigger_type.as_str() {
        "once" => {
            format!(
                "Got it! I'll handle '{}' at the scheduled time.",
                action_summary
            )
        }
        "recurring_email" => {
            if let Some(cond) = &args.condition {
                format!(
                    "Got it! I'll watch for emails matching '{}' and execute the task.",
                    cond
                )
            } else {
                format!(
                    "Got it! I'll monitor your emails and execute: {}",
                    action_summary
                )
            }
        }
        "recurring_messaging" => {
            if let Some(cond) = &args.condition {
                format!(
                    "Got it! I'll watch for messages matching '{}' and execute the task.",
                    cond
                )
            } else {
                format!(
                    "Got it! I'll monitor your messages and execute: {}",
                    action_summary
                )
            }
        }
        _ => format!("Task created: {}", action_summary),
    };

    Ok(confirmation)
}

/// Parse ISO datetime string (in user's timezone) to UTC unix timestamp
fn parse_datetime_to_timestamp(time_str: &str, tz: &Tz) -> Result<i32, Box<dyn Error>> {
    use chrono::{NaiveDateTime, TimeZone};

    // Try parsing as ISO datetime (YYYY-MM-DDTHH:MM or YYYY-MM-DDTHH:MM:SS)
    let naive = if time_str.len() == 16 {
        // YYYY-MM-DDTHH:MM format
        NaiveDateTime::parse_from_str(&format!("{}:00", time_str), "%Y-%m-%dT%H:%M:%S")?
    } else {
        NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%S")?
    };

    // Interpret naive datetime in user's timezone, then get UTC timestamp
    let local_dt = tz
        .from_local_datetime(&naive)
        .single()
        .ok_or("Ambiguous or invalid local time")?;

    Ok(local_dt.timestamp() as i32)
}

/// Tool definition for generate_digest - processes source data into digest format
/// This is a task runtime tool only (not available in SMS conversation)
pub fn get_generate_digest_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};

    // No parameters needed - uses source data passed via context
    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("generate_digest"),
            description: Some(String::from(
                "Generates a concise digest summary from the source data provided in context. \
                Use this when the task needs to summarize emails, messages, and calendar events \
                into a brief SMS-friendly notification. The source data is automatically fetched \
                before this tool is called.",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: None,
                required: None,
            },
        },
    }
}

/// Handle the generate_digest tool call - creates a digest from source data
/// The source_data should already be fetched and passed in by the action executor
pub async fn handle_generate_digest(
    state: &Arc<AppState>,
    user_id: i32,
    source_data: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    use crate::tool_call_utils::utils::create_openai_client_for_user;
    use crate::ModelPurpose;
    use openai_api_rs::v1::{chat_completion, types};

    if source_data.is_empty() {
        return Ok("No source data available to generate digest.".to_string());
    }

    let (client, provider) = create_openai_client_for_user(state, user_id).map_err(
        |e| -> Box<dyn std::error::Error + Send + Sync> {
            Box::new(std::io::Error::other(e.to_string()))
        },
    )?;

    // Use a simplified digest prompt for the tool
    let digest_prompt = r#"You are creating a brief SMS digest of messages and events.

Rules:
- Maximum 480 characters total
- Plain text only (no markdown, emojis, or links)
- Start each platform group on a new line (e.g., "WhatsApp: ...")
- Include timestamps using relative terms (today 3pm, yesterday 8am)
- Put important/urgent items first
- Be concise - tease content, don't fully summarize

Return JSON with a single field:
- `digest` - the plain-text SMS message
"#;

    let user_content = format!(
        "Current datetime: {}\n\nSource data to summarize:\n{}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
        source_data
    );

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(digest_prompt.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(user_content),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let mut properties = std::collections::HashMap::new();
    properties.insert(
        "digest".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The SMS-friendly digest message".to_string()),
            ..Default::default()
        }),
    );

    let tools = vec![chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("create_digest"),
            description: Some(String::from(
                "Creates a concise digest of messages and calendar events",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![String::from("digest")]),
            },
        },
    }];

    let model = state
        .ai_config
        .model(provider, ModelPurpose::Default)
        .to_string();

    let request = chat_completion::ChatCompletionRequest::new(model, messages)
        .tools(tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .max_tokens(350);

    match client.chat_completion(request).await {
        Ok(result) => {
            if let Some(tool_calls) = result.choices[0].message.tool_calls.as_ref() {
                if let Some(first_call) = tool_calls.first() {
                    if let Some(args) = &first_call.function.arguments {
                        #[derive(serde::Deserialize)]
                        struct DigestResponse {
                            digest: String,
                        }
                        if let Ok(resp) = serde_json::from_str::<DigestResponse>(args) {
                            return Ok(resp.digest);
                        }
                    }
                }
            }
            Ok("Failed to generate digest format.".to_string())
        }
        Err(e) => {
            tracing::error!("Failed to generate digest: {}", e);
            Err(Box::new(std::io::Error::other(format!(
                "AI call failed: {}",
                e
            ))))
        }
    }
}
