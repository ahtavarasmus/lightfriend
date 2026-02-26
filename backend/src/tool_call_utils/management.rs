use crate::UserCoreOps;

/// Unified item creation tool for scheduled reminders and message monitoring.
/// Uses structured params (enums, not freeform) so behavioral decisions are deterministic.
pub fn get_create_item_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut properties = HashMap::new();

    properties.insert(
        "item_type".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Item lifecycle type:\n\
                - 'oneshot': fire once, notify, delete. For simple reminders, one-shot monitors.\n\
                - 'recurring': fire, notify, auto-reschedule via repeat pattern. Summary never changes.\n\
                - 'tracking': AI-managed background item. AI decides when to surface, reschedule, or update summary.\n\
                  Example: 'track this package delivery', 'watch Bitcoin price and tell me when it hits 100k'."
                    .to_string(),
            ),
            enum_values: Some(vec![
                "oneshot".to_string(),
                "recurring".to_string(),
                "tracking".to_string(),
            ]),
            ..Default::default()
        }),
    );

    properties.insert(
        "notify".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "How to notify the user when this item fires:\n\
                - 'sms': send an SMS (default for most items)\n\
                - 'call': phone call (for wake-up alarms, urgent alerts)\n\
                - 'silent': no notification (background tracking only)"
                    .to_string(),
            ),
            enum_values: Some(vec![
                "sms".to_string(),
                "call".to_string(),
                "silent".to_string(),
            ]),
            ..Default::default()
        }),
    );

    properties.insert(
        "repeat".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Repeat pattern for recurring items. Required when item_type='recurring'.\n\
                Formats: 'daily HH:MM', 'weekdays HH:MM', 'weekly DAY HH:MM'\n\
                Examples: 'daily 09:00', 'weekdays 08:30', 'weekly Monday 09:00'"
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    properties.insert(
        "fetch".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Comma-separated list of data sources to fetch at trigger time.\n\
                Available: email, chat, calendar, weather, items\n\
                Example: 'email,chat,calendar,items'\n\
                Omit if no data fetching needed."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    properties.insert(
        "description".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Natural language description of what to remind or watch for.\n\
                RULES:\n\
                - Use third person: 'the user', never 'you' or 'me'\n\
                - Include all relevant context: names, times, locations, what to watch for\n\n\
                EXAMPLES:\n\
                - 'Remind the user to call mom.'\n\
                - 'Summarize recent emails, messages, calendar events, and tracked items for the user.'\n\
                - 'Check weather in Tampere. If below freezing, remind the user to warm up the car. If not, no notification needed.'\n\
                - 'Watch for emails from mom.'\n\
                - 'Watch for messages from John about the project.'"
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
                "true to watch incoming emails/messages for matches, false if item only fires at next_check_at."
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
                For monitors (monitor=true): safety-net review/expiration date. If user doesn't specify one,\n\
                infer a reasonable default:\n\
                - General 'notify me when X messages': 2 weeks\n\
                - Time-bounded events (flights, deliveries, payments): match the event timeframe\n\
                - Ongoing watches (price drops, job postings): 1 month\n\
                At this date, the item fires as a check-in: 'still want to watch for X?'"
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    // Monitor-specific tag params (optional, for monitor=true items)
    properties.insert(
        "platform".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "For monitors: which platform to watch. Options: email, whatsapp, telegram, signal, chat (any chat), any"
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
                "For monitors: person/entity name to watch for, or 'any'.".to_string(),
            ),
            ..Default::default()
        }),
    );

    properties.insert(
        "topic".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "For monitors: key topic words to match. Omit if using scope='any'.".to_string(),
            ),
            ..Default::default()
        }),
    );

    properties.insert(
        "scope".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "For monitors: set to 'any' to match ANY message from the sender (skips topic check)."
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
                "Creates a tracked item: a scheduled reminder, recurring digest, background tracker, or a monitor for incoming messages.\n\n\
                ITEM TYPES:\n\
                - oneshot: fire once, notify, delete (simple reminders, one-shot monitors)\n\
                - recurring: fire, notify, auto-reschedule (daily digests, recurring reminders). Requires 'repeat' param.\n\
                - tracking: AI-managed. AI decides when to surface, reschedule, or update (package tracking, price watches)\n\n\
                EXAMPLES:\n\
                - 'remind me to call mom at 10pm' ->\n\
                    item_type='oneshot', notify='sms', description='Remind the user to call mom.', monitor=false, next_check_at='2026-02-28T22:00'\n\
                - 'call me at midnight' ->\n\
                    item_type='oneshot', notify='call', description='Scheduled check-in for the user.', monitor=false, next_check_at='2026-02-29T00:00'\n\
                - 'every morning at 9am summarize my messages and email' ->\n\
                    item_type='recurring', notify='sms', repeat='daily 09:00', fetch='email,chat,calendar,items', description='Summarize recent emails, messages, calendar events, and tracked items.', monitor=false, next_check_at='2026-02-25T09:00'\n\
                - 'let me know if mom emails' ->\n\
                    item_type='oneshot', notify='sms', description='Watch for emails from mom.', monitor=true, platform='email', sender='mom', scope='any', next_check_at='2026-03-09T09:00'\n\
                - 'call me when John messages about the project' ->\n\
                    item_type='oneshot', notify='call', description='Watch for messages from John about the project.', monitor=true, platform='chat', sender='John', topic='project', next_check_at='2026-03-09T09:00'\n\
                - 'track my Amazon package delivery' ->\n\
                    item_type='tracking', notify='sms', description='Track Amazon package delivery. Notify user when shipped or delivered.', monitor=true, platform='email', sender='Amazon', topic='shipping delivery', next_check_at='2026-03-07T09:00'",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    String::from("item_type"),
                    String::from("notify"),
                    String::from("description"),
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
    pub item_type: String,
    pub notify: String,
    pub description: String,
    pub monitor: bool,
    pub next_check_at: String,
    #[serde(default)]
    pub repeat: Option<String>,
    #[serde(default)]
    pub fetch: Option<String>,
    // Monitor tag params
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub sender: Option<String>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
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

    // Build summary from structured params: tags line + "\n" + description
    let summary = build_summary_from_params(&args);

    // Derive priority from notify param
    let priority = match args.notify.as_str() {
        "call" => 2,
        "silent" => 0,
        _ => 1, // sms or unknown defaults to 1
    };

    let new_item = crate::models::user_models::NewItem {
        user_id,
        summary: summary.clone(),
        monitor: args.monitor,
        next_check_at: Some(ts),
        priority,
        source_id: None,
        created_at: now,
    };

    let item_id = state
        .item_repository
        .create_item(&new_item)
        .map_err(|e| format!("Failed to create item: {:?}", e))?;

    let confirmation = if !args.monitor {
        format!("Got it! I'll remind you: {}", args.description)
    } else {
        format!("Got it! I'll watch for: {}", args.description)
    };

    Ok(CreateItemResult {
        message: confirmation,
        task_id: item_id,
    })
}

/// Build summary string from structured CreateItemArgs.
/// Format: "[type:X] [notify:Y] [optional tags...]\nDescription text"
fn build_summary_from_params(args: &CreateItemArgs) -> String {
    let mut tags = Vec::new();

    tags.push(format!("[type:{}]", args.item_type));
    tags.push(format!("[notify:{}]", args.notify));

    if let Some(ref repeat) = args.repeat {
        tags.push(format!("[repeat:{}]", repeat));
    }
    if let Some(ref fetch) = args.fetch {
        tags.push(format!("[fetch:{}]", fetch));
    }

    // Monitor tags
    if let Some(ref platform) = args.platform {
        tags.push(format!("[platform:{}]", platform));
    }
    if let Some(ref sender) = args.sender {
        tags.push(format!("[sender:{}]", sender));
    }
    if let Some(ref topic) = args.topic {
        tags.push(format!("[topic:{}]", topic));
    }
    if let Some(ref scope) = args.scope {
        tags.push(format!("[scope:{}]", scope));
    }

    format!("{}\n{}", tags.join(" "), args.description)
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
        .tool_choice(chat_completion::ToolChoiceType::Required);

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

/// Parse ISO datetime string (in user's timezone) to UTC unix timestamp.
/// Delegates to the shared `parse_user_datetime_to_utc` and converts to i32 timestamp.
fn parse_datetime_to_timestamp(time_str: &str, tz: &Tz) -> Result<i32, Box<dyn Error>> {
    let utc_dt = crate::tool_call_utils::utils::parse_user_datetime_to_utc(time_str, tz)?;
    Ok(utc_dt.timestamp() as i32)
}
