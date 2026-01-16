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
    task_properties.insert(
        "action_spec".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Detailed step-by-step instructions for what to do when triggered. \
                Another AI will execute these instructions using tools, so be specific and unambiguous.\n\n\
                Available runtime tools:\n\
                - send_reminder(message): Send notification to user\n\
                - control_tesla(command): Control Tesla (climate_on, climate_off, lock, unlock)\n\
                - get_weather(location): Get weather info\n\
                - send_chat_message(platform, contact, message): Send message via WhatsApp/Telegram/Signal\n\
                - send_email(to, subject, body): Send email\n\
                - fetch_calendar_events(): Check calendar\n\n\
                Examples:\n\
                - 'Use send_reminder to notify user about picking up the package'\n\
                - 'Use control_tesla to turn on climate control'\n\
                - 'Step 1: Use get_weather to check temperature. Step 2: If below 10C, use control_tesla to turn on climate. Otherwise use send_reminder to inform user weather is warm.'\n\
                - 'Use send_chat_message to send \"just checking in\" to mom on WhatsApp'".to_string()
            ),
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
                USE CASES:\n\
                1. Time-based reminders: 'remind me at 1pm about the package'\n\
                2. Scheduled actions: 'turn on tesla climate in 3 hours'\n\
                3. Message monitoring: 'notify me when mom messages'\n\
                4. Email monitoring: 'let me know when I get an email about my job application'\n\
                5. Conditional actions: 'if mom hasn't replied by 8pm, send her a follow up'\n\n\
                For monitoring tasks, the condition describes what to look for.\n\
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

/// Tool definition for send_reminder - ONLY available to task runtime AI, not SMS conversation
pub fn get_send_reminder_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut reminder_properties = HashMap::new();
    reminder_properties.insert(
        "message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "The reminder message to send to the user via SMS or call.".to_string(),
            ),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("send_reminder"),
            description: Some(String::from(
                "Sends a notification/reminder message to the user via SMS or phone call. \
                Use this tool when the task requires notifying or reminding the user about something. \
                The message will be delivered immediately to the user's phone."
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(reminder_properties),
                required: Some(vec![String::from("message")]),
            },
        },
    }
}

#[derive(Deserialize)]
pub struct SendReminderArgs {
    pub message: String,
}

/// Handle the send_reminder tool call - sends notification to user
pub async fn handle_send_reminder(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
    notification_type: &str,
) -> Result<String, Box<dyn Error>> {
    let args: SendReminderArgs = serde_json::from_str(args)?;

    let noti_type = format!("task_reminder_{}", notification_type);
    let first_message = Some("Hey, here's your scheduled reminder.".to_string());

    crate::proactive::utils::send_notification(
        state,
        user_id,
        &args.message,
        noti_type,
        first_message,
    )
    .await;

    Ok(format!("Reminder sent: {}", args.message))
}
