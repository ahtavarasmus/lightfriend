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

/// Unified item creation tool for scheduled reminders and message monitoring.
pub fn get_create_task_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut properties = HashMap::new();

    properties.insert(
        "summary".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "A concise, natural language description of the item. This is what the user will see \
                and what the system uses to generate notifications.\n\
                LANGUAGE: Always use third person ('the user'), NEVER 'you' or 'me'.\n\
                EXAMPLES:\n\
                - 'remind me to call mom' -> \"Remind the user to call mom\"\n\
                - 'call me at midnight' -> \"Scheduled check-in call for the user\"\n\
                - 'remind me at 10pm to turn on tesla' -> \"Remind the user to turn on Tesla climate\"\n\
                - 'let me know if mom emails' -> \"Watch for emails from mom\"\n\
                - 'notify me when my package ships' -> \"Watch for package shipping notification\"\n\
                Include relevant details like event times, contact names, or what to watch for."
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
                "How the item should be triggered. Options:\n\
                - \"once\": Fire at a specific time (requires trigger_time)\n\
                - \"recurring_email\": Watch each incoming email for a match\n\
                - \"recurring_messaging\": Watch each incoming message for a match"
                    .to_string(),
            ),
            enum_values: Some(vec![
                "once".to_string(),
                "recurring_email".to_string(),
                "recurring_messaging".to_string(),
            ]),
            ..Default::default()
        }),
    );

    properties.insert(
        "trigger_time".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "When to fire the item. Required for trigger_type='once'. \
                Format: ISO datetime 'YYYY-MM-DDTHH:MM' in the user's timezone. \
                Convert relative times like 'in 3 hours', 'tomorrow at 9am' to this format.\n\
                SMART TIMING: Set trigger_time BEFORE events so reminders arrive early:\n\
                - Virtual/same-location (meeting, call): 5 minutes before\n\
                - Appointments (doctor, interview): 45 minutes before\n\
                - Travel needed (restaurant, airport): 60 minutes before\n\
                - OVERRIDE: If user specifies exact time ('remind me at 1pm'), use that time."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    properties.insert(
        "notification_type".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "How to notify the user. If not specified, uses user's default preference. \
                Only set this if user explicitly asks (e.g., 'call me', 'text me'). \
                Options: 'sms', 'call', or 'call_sms'."
                    .to_string(),
            ),
            enum_values: Some(vec![
                "sms".to_string(),
                "call".to_string(),
                "call_sms".to_string(),
            ]),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("create_task"),
            description: Some(String::from(
                "Creates a scheduled reminder or sets up monitoring for incoming emails/messages. \
                Items are informational - they track things and notify the user.\n\n\
                EXAMPLES:\n\
                - 'call me at midnight' -> summary=\"Scheduled check-in\", trigger_type=\"once\", notification_type=\"call\"\n\
                - 'remind me to call mom at 10pm' -> summary=\"Remind the user to call mom\", trigger_type=\"once\"\n\
                - 'let me know if mom emails' -> summary=\"Watch for emails from mom\", trigger_type=\"recurring_email\"\n\
                - 'tell me when John messages' -> summary=\"Watch for messages from John\", trigger_type=\"recurring_messaging\"",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    String::from("summary"),
                    String::from("trigger_type"),
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
    pub summary: String,
    pub trigger_type: String, // "once" | "recurring_email" | "recurring_messaging"
    pub trigger_time: Option<String>, // "2025-12-30T13:00" (required for "once")
    pub notification_type: Option<String>,
}

/// Result from handle_create_task containing the confirmation message and item ID
pub struct CreateTaskResult {
    pub message: String,
    pub task_id: i32, // actually item_id now
}

pub async fn handle_create_task(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> Result<CreateTaskResult, Box<dyn Error>> {
    let args: CreateTaskArgs = serde_json::from_str(args)?;

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

    let notification_type = args.notification_type.unwrap_or(default_noti_type);

    // Determine item fields based on trigger type
    let (kind, due_at, next_check_at, priority) = match args.trigger_type.as_str() {
        "once" => {
            let time_str = args
                .trigger_time
                .as_ref()
                .ok_or("trigger_time is required for 'once' trigger type")?;

            let user_info = state
                .user_core
                .get_user_info(user_id)
                .map_err(|e| format!("Failed to get user info: {:?}", e))?;
            let tz_str = user_info.timezone.unwrap_or_else(|| "UTC".to_string());
            let tz: Tz = tz_str.parse().unwrap_or(chrono_tz::UTC);

            let timestamp = parse_datetime_to_timestamp(time_str, &tz)?;
            ("reminder", Some(timestamp), Some(timestamp), 1)
        }
        "recurring_email" | "recurring_messaging" => {
            // User explicitly asked: priority 1 (more important than auto-detected)
            ("monitor", None, None, 1)
        }
        _ => return Err(format!("Invalid trigger_type: {}", args.trigger_type).into()),
    };

    // For reminders (trigger_type="once"), format summary as structured reminder
    // Encode notification_type into summary text when it's "call" or "call_sms"
    let via_call_tag = if notification_type == "call" || notification_type == "call_sms" {
        " [VIA CALL]"
    } else {
        ""
    };

    let summary = if kind == "reminder" {
        if let Some(ref time_str) = args.trigger_time {
            let user_info = state
                .user_core
                .get_user_info(user_id)
                .map_err(|e| format!("Failed to get user info: {:?}", e))?;
            let tz_str = user_info.timezone.unwrap_or_else(|| "UTC".to_string());
            format!(
                "REMINDER {} {}{}: {}",
                time_str, tz_str, via_call_tag, args.summary
            )
        } else if via_call_tag.is_empty() {
            args.summary.clone()
        } else {
            format!("{}{}", args.summary, via_call_tag)
        }
    } else if via_call_tag.is_empty() {
        args.summary.clone()
    } else {
        format!("{}{}", args.summary, via_call_tag)
    };

    let new_item = crate::models::user_models::NewItem {
        user_id,
        summary,
        kind: kind.to_string(),
        due_at,
        next_check_at,
        priority,
        source_id: None,
        created_at: now,
    };

    let item_id = state
        .item_repository
        .create_item(&new_item)
        .map_err(|e| format!("Failed to create item: {:?}", e))?;

    let confirmation = match args.trigger_type.as_str() {
        "once" => format!("Got it! I'll remind you: {}", args.summary),
        _ => format!("Got it! I'll watch for: {}", args.summary),
    };

    Ok(CreateTaskResult {
        message: confirmation,
        task_id: item_id,
    })
}

/// Wrapper around handle_create_task that retries once via LLM if the first attempt fails.
/// On first failure, sends the error back to the LLM asking it to fix the arguments,
/// then executes the corrected create_task call.
#[allow(clippy::too_many_arguments)]
pub async fn handle_create_task_with_retry(
    state: &Arc<AppState>,
    user_id: i32,
    arguments: &str,
    client: &openai_api_rs::v1::api::OpenAIClient,
    model: &str,
    tools: &[openai_api_rs::v1::chat_completion::Tool],
    completion_messages: &[openai_api_rs::v1::chat_completion::ChatCompletionMessage],
    failed_tool_call: &openai_api_rs::v1::chat_completion::ToolCall,
    assistant_content: Option<&str>,
) -> Result<CreateTaskResult, Box<dyn Error>> {
    use openai_api_rs::v1::chat_completion;

    // Attempt 1: try with original arguments
    // Convert error to String immediately so Box<dyn Error> (non-Send) doesn't live across .await
    let first_err_msg = match handle_create_task(state, user_id, arguments).await {
        Ok(result) => return Ok(result),
        Err(e) => e.to_string(),
    };

    tracing::warn!("create_task attempt 1/2 failed: {}", first_err_msg);

    // Build retry messages: conversation + failed assistant call + error feedback
    let mut retry_msgs = completion_messages.to_vec();
    retry_msgs.push(chat_completion::ChatCompletionMessage {
        role: chat_completion::MessageRole::assistant,
        content: chat_completion::Content::Text(assistant_content.unwrap_or_default().to_string()),
        name: None,
        tool_calls: Some(vec![failed_tool_call.clone()]),
        tool_call_id: None,
    });
    retry_msgs.push(chat_completion::ChatCompletionMessage {
        role: chat_completion::MessageRole::tool,
        content: chat_completion::Content::Text(format!(
            "Error: {}. Please fix the arguments and call create_task again.",
            first_err_msg
        )),
        name: None,
        tool_calls: None,
        tool_call_id: Some(failed_tool_call.id.clone()),
    });

    // Ask LLM to retry with fixed arguments
    let retry_req = chat_completion::ChatCompletionRequest::new(model.to_string(), retry_msgs)
        .tools(tools.to_vec())
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .max_tokens(500);

    let retry_result = client
        .chat_completion(retry_req)
        .await
        .map_err(|e| format!("create_task retry API call failed: {}", e))?;

    // Find create_task in the retry response
    let retry_task_call = retry_result
        .choices
        .first()
        .and_then(|c| c.message.tool_calls.as_ref())
        .and_then(|calls| {
            calls
                .iter()
                .find(|c| c.function.name.as_deref() == Some("create_task"))
        });

    match retry_task_call {
        Some(retry_call) => {
            let retry_args = retry_call.function.arguments.as_deref().unwrap_or("{}");
            match handle_create_task(state, user_id, retry_args).await {
                Ok(result) => {
                    tracing::info!("create_task succeeded on retry (attempt 2/2)");
                    Ok(result)
                }
                Err(second_err) => {
                    tracing::error!("create_task attempt 2/2 also failed: {}", second_err);
                    Err(second_err)
                }
            }
        }
        None => {
            // LLM didn't call create_task on retry
            tracing::error!("LLM did not retry create_task after error feedback");
            Err(first_err_msg.into())
        }
    }
}

/// Parse ISO datetime string (in user's timezone) to UTC unix timestamp
fn parse_datetime_to_timestamp(time_str: &str, tz: &Tz) -> Result<i32, Box<dyn Error>> {
    use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone};

    // Try parsing as full ISO datetime with timezone offset first (e.g. "2026-02-08T07:00:00+02:00")
    // AI often includes timezone offsets - use the offset directly for accurate conversion
    if let Ok(dt) = DateTime::<FixedOffset>::parse_from_rfc3339(time_str) {
        return Ok(dt.timestamp() as i32);
    }
    // Also try common offset format without seconds (e.g. "2026-02-08T07:00+02:00")
    if let Ok(dt) = DateTime::<FixedOffset>::parse_from_rfc3339(&format!(
        "{}:00{}",
        &time_str[..16],
        &time_str[16..]
    )) {
        return Ok(dt.timestamp() as i32);
    }

    // Fall back to naive datetime parsing (no timezone offset in string)
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
