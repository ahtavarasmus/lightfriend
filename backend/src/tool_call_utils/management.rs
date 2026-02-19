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
                to this format based on current time.\n\
                SMART TIMING: When the user describes an upcoming event, set trigger_time BEFORE the event \
                so the reminder arrives with enough preparation time:\n\
                - Virtual/same-location (meeting, call, class, standup): 5 minutes before\n\
                - Appointments (doctor, dentist, interview, haircut): 45 minutes before\n\
                - Travel needed (restaurant, friend's place, airport, gym): 60 minutes before\n\
                - OVERRIDE: If the user specifies an explicit reminder time (e.g. 'remind me at 1pm about \
                my 2pm meeting'), use their exact time, not the smart offset.\n\
                - Include the actual event time in the reminder message so the user knows when it starts \
                (e.g. 'Meeting at 2:00 PM - starting in 5 min').".to_string()
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

    // action_tool: enum of available runtime tools
    let tool_names = crate::tool_call_utils::utils::get_runtime_tool_names();
    let tools_prompt = crate::tool_call_utils::utils::get_runtime_tools_prompt();

    task_properties.insert(
        "action_tool".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(format!(
                "The tool to execute when this task fires. Pick from the enum list.\n\n\
                CRITICAL - REMINDER vs ACTION:\n\
                - User says 'remind me to X' -> action_tool: \"send_reminder\" (just NOTIFY, don't do X)\n\
                - User says 'do X' / 'turn on X' -> action_tool: \"control_tesla\" (actually EXECUTE X)\n\n\
                {}\n\
                CORRECT:\n\
                - 'remind me to call mom' -> action_tool: \"send_reminder\"\n\
                - 'turn on tesla climate' -> action_tool: \"control_tesla\"\n\
                - 'call me at midnight' -> action_tool: \"send_reminder\"\n\n\
                WRONG:\n\
                - Calling send_reminder as a standalone tool - it is only valid here inside create_task\n\
                - 'remind me to X' -> action_tool: \"control_tesla\" - user wants reminder, not action",
                tools_prompt
            )),
            enum_values: Some(tool_names),
            ..Default::default()
        }),
    );

    // action_params: JSON object with tool-specific parameters
    task_properties.insert(
        "action_params".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "A JSON object string with parameters for the chosen action_tool.\n\
                Each tool has its own params shape:\n\
                - send_reminder: {\"message\": \"Call mom\"}\n\
                - control_tesla: {\"command\": \"climate_on\"}\n\
                - send_chat_message: {\"platform\": \"whatsapp\", \"contact\": \"Mom\", \"message\": \"Hi\"}\n\
                  (platform is optional - defaults to user's first connected platform;\n\
                   contact is REQUIRED - the person or group to message.\n\
                   If the user doesn't specify who to message, ask them.)\n\
                - send_email: {\"to\": \"a@b.com\", \"subject\": \"Hi\", \"body\": \"...\"}\n\
                - fetch_calendar_events: {}\n\
                - get_weather: {\"location\": \"New York\"}\n\n\
                LANGUAGE: Always use third person ('the user'), NEVER 'you'.".to_string()
            ),
            ..Default::default()
        }),
    );

    // sources: data sources to fetch before evaluating condition
    task_properties.insert(
        "sources".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Comma-separated data sources to fetch before evaluating the condition.\n\
                Use 'weather' for weather/temperature conditions (uses user's default location).\n\
                Use 'email' for email-based conditions. Use 'calendar' for calendar-based conditions.\n\
                Only set this when the task has a condition that needs external data to evaluate.\n\
                Leave empty for unconditional tasks like simple reminders.".to_string()
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
                "Creates a scheduled task or sets up monitoring for future events. \
                You MUST use this tool for all reminders and scheduled actions. \
                Do NOT attempt to call send_reminder directly - it is not a tool, it is only valid as action_tool inside create_task.\n\n\
                NOTIFICATION METHOD (how to notify user):\n\
                - 'call me at X' = action_tool=\"send_reminder\", action_params={\"message\":\"Scheduled check-in\"}, notification_type='call'\n\
                - 'text me at X' = action_tool=\"send_reminder\", notification_type='sms'\n\
                - Default is SMS if not specified\n\n\
                REMINDER vs ACTION (what to do):\n\
                - 'remind me to X' = action_tool=\"send_reminder\", action_params={\"message\":\"X\"}\n\
                - 'do X' / 'turn on X' = action_tool=\"control_tesla\", action_params={\"command\":\"climate_on\"}\n\n\
                LANGUAGE: Always use 'the user' (third person), NEVER 'you' or 'me'.\n\n\
                EXAMPLES:\n\
                - 'call me at midnight' -> action_tool=\"send_reminder\", action_params={\"message\":\"Scheduled check-in\"}, notification_type='call'\n\
                - 'remind me at 10pm to turn on tesla' -> action_tool=\"send_reminder\", action_params={\"message\":\"Turn on Tesla climate\"}\n\
                - 'turn on tesla at 10pm' -> action_tool=\"control_tesla\", action_params={\"command\":\"climate_on\"}\n\
                - 'remind me to call mom' -> action_tool=\"send_reminder\", action_params={\"message\":\"Call mom\"}\n\n\
                For scheduled tasks, use trigger_type='once' with trigger_time.",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(task_properties),
                required: Some(vec![
                    String::from("trigger_type"),
                    String::from("action_tool"),
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
    pub action_tool: String, // Tool name from runtime tools registry
    pub action_params: Option<serde_json::Value>, // JSON object with tool-specific params
    pub notification_type: Option<String>,
    pub sources: Option<String>, // comma-separated source types: "weather", "email", "calendar", etc.
}

/// Result from handle_create_task containing the confirmation message and task ID
pub struct CreateTaskResult {
    pub message: String,
    pub task_id: i32,
}

pub async fn handle_create_task(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> Result<CreateTaskResult, Box<dyn Error>> {
    let args: CreateTaskArgs = serde_json::from_str(args)?;

    // Validate action_tool against known tools
    let known_tools = crate::tool_call_utils::utils::get_runtime_tool_names();
    if !known_tools.contains(&args.action_tool) {
        return Err(format!(
            "Unknown action_tool '{}'. Valid tools: {}",
            args.action_tool,
            known_tools.join(", ")
        )
        .into());
    }

    // Normalize action_params to a JSON object
    // AI might send a string instead of an object - wrap it based on tool name
    let mut params: Option<serde_json::Value> = if let Some(ref val) = args.action_params {
        if val.is_object() {
            Some(val.clone())
        } else if let Some(s) = val.as_str() {
            // AI sent a string - it might be serialized JSON or a plain string
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                if parsed.is_object() {
                    // String was serialized JSON object like "{\"message\":\"Call mom\"}"
                    Some(parsed)
                } else if let Some(inner) = parsed.as_str() {
                    // String was double-quoted like "\"Call mom\""
                    let key = match args.action_tool.as_str() {
                        "send_reminder" => "message",
                        "control_tesla" => "command",
                        "get_weather" => "location",
                        _ => "raw",
                    };
                    Some(serde_json::json!({ key: inner }))
                } else {
                    Some(serde_json::json!({ "raw": s }))
                }
            } else {
                // Plain string like "Call mom" - wrap in expected param key
                let key = match args.action_tool.as_str() {
                    "send_reminder" => "message",
                    "control_tesla" => "command",
                    "get_weather" => "location",
                    _ => "raw",
                };
                Some(serde_json::json!({ key: s }))
            }
        } else {
            Some(val.clone())
        }
    } else {
        None
    };

    // Resolve invalid platform for send_chat_message to the user's first connected bridge
    if args.action_tool == "send_chat_message" {
        if let Some(ref mut p) = params {
            let valid_platforms = ["telegram", "whatsapp", "signal"];
            let needs_default = match p.get("platform") {
                Some(serde_json::Value::String(s)) => {
                    !valid_platforms.contains(&s.to_lowercase().as_str())
                }
                None => true,
                _ => true,
            };
            if needs_default {
                if let Ok(Some(bridge)) = state.user_repository.get_first_connected_bridge(user_id)
                {
                    if let Some(obj) = p.as_object_mut() {
                        obj.insert(
                            "platform".to_string(),
                            serde_json::Value::String(bridge.bridge_type),
                        );
                    }
                }
            }
        }
    }

    // Resolve contact name for send_chat_message via contact profiles + Matrix bridge
    if args.action_tool == "send_chat_message" {
        if let Some(ref mut p) = params {
            // Normalize: LLM sometimes uses "chat_name" instead of "contact"
            if p.get("contact").is_none() {
                if let Some(chat_name_val) =
                    p.as_object_mut().and_then(|obj| obj.remove("chat_name"))
                {
                    if let Some(obj) = p.as_object_mut() {
                        obj.insert("contact".to_string(), chat_name_val);
                    }
                }
            }

            if let Some(contact_val) = p
                .get("contact")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
            {
                if !contact_val.is_empty() {
                    let platform = p
                        .get("platform")
                        .and_then(|v| v.as_str())
                        .unwrap_or("whatsapp")
                        .to_string();

                    // Try resolving via contact profile nickname
                    let profiles = state
                        .user_repository
                        .get_contact_profiles(user_id)
                        .unwrap_or_default();
                    let profile_chat = profiles.iter().find_map(|prof| {
                        let nickname_lower = prof.nickname.to_lowercase();
                        let contact_lower = contact_val.to_lowercase();
                        if nickname_lower.contains(&contact_lower)
                            || contact_lower.contains(&nickname_lower)
                        {
                            match platform.as_str() {
                                "whatsapp" => prof.whatsapp_chat.clone(),
                                "telegram" => prof.telegram_chat.clone(),
                                "signal" => prof.signal_chat.clone(),
                                _ => None,
                            }
                        } else {
                            None
                        }
                    });

                    let search_term = profile_chat.unwrap_or_else(|| contact_val.clone());

                    // Try to resolve via Matrix bridge rooms
                    match crate::utils::matrix_auth::get_cached_client(user_id, state).await {
                        Ok(client) => {
                            match crate::utils::bridge::get_service_rooms(&client, &platform).await
                            {
                                Ok(rooms) => {
                                    if let Some(best_match) =
                                        crate::utils::bridge::search_best_match(
                                            &rooms,
                                            &search_term,
                                        )
                                    {
                                        let resolved_name =
                                            crate::utils::bridge::remove_bridge_suffix(
                                                &best_match.display_name,
                                            );
                                        if let Some(obj) = p.as_object_mut() {
                                            obj.insert(
                                                "contact".to_string(),
                                                serde_json::Value::String(resolved_name),
                                            );
                                        }
                                    } else {
                                        return Err(format!(
                                            "Could not find contact '{}' on {}. Please check the name and try again.",
                                            contact_val,
                                            platform.chars().next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default() + &platform[1..]
                                        ).into());
                                    }
                                }
                                Err(e) => {
                                    return Err(format!(
                                        "Failed to verify contact on {}: {}. Please make sure your {} bridge is connected.",
                                        platform, e, platform
                                    ).into());
                                }
                            }
                        }
                        Err(e) => {
                            return Err(format!(
                                "Could not connect to messaging bridge: {}. Please make sure your {} bridge is connected.",
                                e, platform
                            ).into());
                        }
                    }
                }
            }
        }
    }

    // Build StructuredAction and serialize to JSON for the action column
    let structured = crate::utils::action_executor::StructuredAction {
        tool: args.action_tool.clone(),
        params,
    };
    let action_json = serde_json::to_string(&structured)?;

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
        action: action_json,
        notification_type: args.notification_type.or(Some(default_noti_type)),
        status: "active".to_string(),
        created_at: now,
        is_permanent: None,
        recurrence_rule: None,
        recurrence_time: None,
        sources: args.sources.clone(),
        end_time: None,
    };

    let task_id = state
        .user_repository
        .create_task(&new_task)
        .map_err(|e| format!("Failed to create task: {:?}", e))?;

    // Build human-friendly confirmation using format_action_description
    let action_display = crate::handlers::dashboard_handlers::format_action_description(
        &structured.to_action_string(),
    );

    let confirmation = match args.trigger_type.as_str() {
        "once" => {
            format!(
                "Got it! I'll handle '{}' at the scheduled time.",
                action_display
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
                    action_display
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
                    action_display
                )
            }
        }
        _ => format!("Task created: {}", action_display),
    };

    Ok(CreateTaskResult {
        message: confirmation,
        task_id,
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
