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
pub fn get_create_item_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut properties = HashMap::new();

    properties.insert(
        "summary".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "A concise description of what to remind or watch for.\n\
                At notification time, a separate LLM reads ONLY this summary to craft an SMS (max 160 chars)\n\
                and a voice opener (max 100 chars), and to decide delivery method (SMS or voice call).\n\n\
                RULES:\n\
                - Use third person: 'the user', never 'you' or 'me'\n\
                - Include all relevant context: names, times, locations, what to watch for\n\
                - If the user wants a PHONE CALL (e.g. 'call me', 'give me a call'), append ' [VIA CALL]'\n\
                  at the end. Otherwise omit it - SMS is the default.\n\n\
                EXAMPLES:\n\
                - 'remind me to call mom at 10pm' -> \"Remind the user to call mom\"\n\
                - 'call me at midnight' -> \"Scheduled check-in for the user [VIA CALL]\"\n\
                - 'let me know if mom emails' -> \"Watch for emails from mom. Notify the user when one arrives.\"\n\
                - 'tell me when John messages about the project' -> \"Watch for messages from John about the project. Remind the user when a match arrives.\"\n\
                - 'remind me about my 2pm dentist appointment' -> \"Dentist appointment at 2pm\""
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    properties.insert(
        "monitor".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "true to watch incoming emails/messages for matches, false if item only fires at next_check_at (can be rescheduled)."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    properties.insert(
        "next_check_at".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "ISO datetime 'YYYY-MM-DDTHH:MM' in the user's timezone. ALWAYS REQUIRED.\n\
                For reminders (monitor=false): when to fire.\n\
                For monitors (monitor=true): review/expiration date. If user doesn't specify one,\n\
                infer a reasonable default:\n\
                - General 'notify me when X messages': 2 weeks\n\
                - Time-bounded events (flights, deliveries, payments): match the event timeframe\n\
                - Ongoing watches (price drops, job postings): 1 month\n\
                At this date, the item fires as a check-in: 'still want to watch for X?'\n\n\
                SMART TIMING for reminders (monitor=false):\n\
                - Virtual/same-location events (meeting, call): 5 minutes before\n\
                - Appointments (doctor, interview): 45 minutes before\n\
                - Travel needed (restaurant, airport): 60 minutes before\n\
                - OVERRIDE: If user specifies exact time ('remind me at 1pm'), use that time exactly"
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("create_item"),
            description: Some(String::from(
                "Creates a tracked item: a scheduled reminder or a monitor for incoming emails/messages.\n\
                - monitor=false: fires at next_check_at. Can be rescheduled with a new next_check_at.\n\
                  Provide next_check_at as ISO datetime in the user's timezone.\n\
                - monitor=true: watches every incoming email/message for matches.\n\
                  Optionally set next_check_at as a safety-net deadline to follow up if nothing is caught.\n\n\
                EXAMPLES:\n\
                - 'remind me to call mom at 10pm' ->\n\
                    summary=\"Remind the user to call mom\", monitor=false, next_check_at=\"2026-02-28T22:00\"\n\
                - 'call me at midnight' ->\n\
                    summary=\"Scheduled check-in [VIA CALL]\", monitor=false, next_check_at=\"2026-02-29T00:00\"\n\
                - 'let me know if mom emails' ->\n\
                    summary=\"Watch for emails from mom. Notify the user when one arrives.\", monitor=true\n\
                - 'watch for invoice payment, check March 1 if unpaid' ->\n\
                    summary=\"Watch for invoice payment confirmation. Remind user if still unpaid.\", monitor=true, next_check_at=\"2026-03-01T09:00\"\n\
                - 'tell me when John messages' ->\n\
                    summary=\"Watch for messages from John. Notify the user when a match arrives.\", monitor=true",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    String::from("summary"),
                    String::from("monitor"),
                    String::from("next_check_at"),
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
pub struct CreateItemArgs {
    pub summary: String,
    pub monitor: bool,
    pub next_check_at: String,
}

/// Result from handle_create_item containing the confirmation message and item ID
pub struct CreateItemResult {
    pub message: String,
    pub task_id: i32, // item_id
}

pub async fn handle_create_item(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> Result<CreateItemResult, Box<dyn Error>> {
    let args: CreateItemArgs = serde_json::from_str(args)?;

    // Gate monitor items to Autopilot/BYOT plans only
    if args.monitor {
        let user_plan = state.user_repository.get_plan_type(user_id).unwrap_or(None);
        if !crate::utils::plan_features::has_auto_features(user_plan.as_deref()) {
            return Err("Monitoring items is an Autopilot plan feature. Upgrade to Autopilot to have Lightfriend automatically watch your messages for updates on this item.".into());
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Parse next_check_at (always required)
    let user_info = state
        .user_core
        .get_user_info(user_id)
        .map_err(|e| format!("Failed to get user info: {:?}", e))?;
    let tz_str = user_info.timezone.unwrap_or_else(|| "UTC".to_string());
    let tz: Tz = tz_str.parse().unwrap_or(chrono_tz::UTC);
    let ts = parse_datetime_to_timestamp(&args.next_check_at, &tz)?;
    let (due_at, next_check_at_ts) = (Some(ts), Some(ts));

    let summary = args.summary.clone();

    let new_item = crate::models::user_models::NewItem {
        user_id,
        summary,
        monitor: args.monitor,
        due_at,
        next_check_at: next_check_at_ts,
        priority: 1,
        source_id: None,
        created_at: now,
    };

    let item_id = state
        .item_repository
        .create_item(&new_item)
        .map_err(|e| format!("Failed to create item: {:?}", e))?;

    let confirmation = if !args.monitor {
        format!("Got it! I'll remind you: {}", args.summary)
    } else {
        format!("Got it! I'll watch for: {}", args.summary)
    };

    Ok(CreateItemResult {
        message: confirmation,
        task_id: item_id,
    })
}

/// Wrapper around handle_create_item that retries once via LLM if the first attempt fails.
/// On first failure, sends the error back to the LLM asking it to fix the arguments,
/// then executes the corrected create_item call.
#[allow(clippy::too_many_arguments)]
pub async fn handle_create_item_with_retry(
    state: &Arc<AppState>,
    user_id: i32,
    arguments: &str,
    client: &openai_api_rs::v1::api::OpenAIClient,
    model: &str,
    tools: &[openai_api_rs::v1::chat_completion::Tool],
    completion_messages: &[openai_api_rs::v1::chat_completion::ChatCompletionMessage],
    failed_tool_call: &openai_api_rs::v1::chat_completion::ToolCall,
    assistant_content: Option<&str>,
) -> Result<CreateItemResult, Box<dyn Error>> {
    use openai_api_rs::v1::chat_completion;

    // Attempt 1: try with original arguments
    // Convert error to String immediately so Box<dyn Error> (non-Send) doesn't live across .await
    let first_err_msg = match handle_create_item(state, user_id, arguments).await {
        Ok(result) => return Ok(result),
        Err(e) => e.to_string(),
    };

    tracing::warn!("create_item attempt 1/2 failed: {}", first_err_msg);

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
            "Error: {}. Please fix the arguments and call create_item again.",
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
        .map_err(|e| format!("create_item retry API call failed: {}", e))?;

    // Find create_item in the retry response
    let retry_task_call = retry_result
        .choices
        .first()
        .and_then(|c| c.message.tool_calls.as_ref())
        .and_then(|calls| {
            calls
                .iter()
                .find(|c| c.function.name.as_deref() == Some("create_item"))
        });

    match retry_task_call {
        Some(retry_call) => {
            let retry_args = retry_call.function.arguments.as_deref().unwrap_or("{}");
            match handle_create_item(state, user_id, retry_args).await {
                Ok(result) => {
                    tracing::info!("create_item succeeded on retry (attempt 2/2)");
                    Ok(result)
                }
                Err(second_err) => {
                    tracing::error!("create_item attempt 2/2 also failed: {}", second_err);
                    Err(second_err)
                }
            }
        }
        None => {
            // LLM didn't call create_item on retry
            tracing::error!("LLM did not retry create_item after error feedback");
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
