use crate::UserCoreOps;

/// Unified item creation tool for scheduled reminders and tracking items.
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
                "oneshot: fire once then delete. recurring: scheduled repeating report, always notifies. tracking: watches for a condition, only notifies when condition is met."
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
                "How to notify: sms (default), call (wake-ups, urgent), silent (background only)."
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
                "Required for recurring items. Format: 'daily HH:MM', 'weekdays HH:MM', 'weekly DAY HH:MM'."
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
                "Data sources to poll, or 'none'. For recurring: what data to include in reports. For tracking external data (prices, weather, news): set to 'internet'. Sources: email, chat, calendar, weather, items, internet, none."
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
                "What to remind or watch for. Use third person ('the user'). Include all relevant context from the user's request."
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
                "ISO datetime YYYY-MM-DDTHH:MM in user's timezone. Required for oneshot (trigger time). Optional for tracking (expiration deadline, defaults to 30 days). Optional for recurring (auto-computed from repeat pattern). For actions ('do X at 3pm') use exact time. For events ('meeting at 3pm') set before: 5min same-location, 45min appointments, 60min travel. Explicit time overrides ('remind at 1pm about 2pm meeting' = 13:00). Include actual event time in description."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    properties.insert(
        "platform".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Platform to match incoming messages against. Set 'none' for oneshot/recurring or data tracking via fetch."
                    .to_string(),
            ),
            enum_values: Some(vec![
                "email".to_string(),
                "whatsapp".to_string(),
                "telegram".to_string(),
                "signal".to_string(),
                "chat".to_string(),
                "any".to_string(),
                "none".to_string(),
            ]),
            ..Default::default()
        }),
    );

    properties.insert(
        "sender".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Person or entity name to match incoming messages against, or 'any'. Set 'none' for oneshot/recurring or data tracking via fetch."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    properties.insert(
        "topic".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Topic keywords to match incoming messages, or 'any' to match all messages from sender regardless of content. Set 'none' for oneshot/recurring or data tracking via fetch."
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
                "Create a scheduled item for reminders, recurring reports, or monitoring. Use for anything that should happen on a schedule (daily briefings, weekly summaries, periodic checks) or trigger on a condition (notify when X happens). Tracking items have two modes: message tracking (set platform/sender/topic to filter incoming messages) or data tracking (set fetch to poll external sources like internet, and platform/sender/topic to 'none').",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    String::from("item_type"),
                    String::from("notify"),
                    String::from("description"),
                    String::from("fetch"),
                    String::from("platform"),
                    String::from("sender"),
                    String::from("topic"),
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
    #[serde(default)]
    pub due_at: Option<String>,
    #[serde(default)]
    pub repeat: Option<String>,
    #[serde(default)]
    pub fetch: Option<String>,
    // Tracking tag params
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub sender: Option<String>,
    #[serde(default)]
    pub topic: Option<String>,
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
    let mut args: CreateItemArgs = serde_json::from_str(args)?;

    // Normalize "none" values to None (params are required in schema but "none" means not applicable)
    if args.fetch.as_deref() == Some("none") {
        args.fetch = None;
    }
    if args.platform.as_deref() == Some("none") {
        args.platform = None;
    }
    if args.sender.as_deref() == Some("none") {
        args.sender = None;
    }
    if args.topic.as_deref() == Some("none") {
        args.topic = None;
    }

    // Gate tracking items to Autopilot/BYOT plans only
    if args.item_type == "tracking" {
        let user_plan = state.user_repository.get_plan_type(user_id).unwrap_or(None);
        if !crate::utils::plan_features::has_auto_features(user_plan.as_deref()) {
            return Err("Tracking items is an Autopilot plan feature. Upgrade to Autopilot to have Lightfriend automatically watch for updates on this item.".into());
        }

        // Tracking items must have a data source: either fetch or communication filters
        let has_fetch = args.fetch.as_ref().is_some_and(|f| !f.trim().is_empty());
        let has_filters = args.platform.is_some() || args.sender.is_some() || args.topic.is_some();
        if !has_fetch && !has_filters {
            return Err("Tracking items need a data source. Set fetch (e.g. internet, email) or platform/sender/topic filters.".into());
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let user_info = state
        .user_core
        .get_user_info(user_id)
        .map_err(|e| format!("Failed to get user info: {:?}", e))?;
    let tz_str = user_info.timezone.unwrap_or_else(|| "UTC".to_string());
    let tz: Tz = tz_str.parse().unwrap_or(chrono_tz::UTC);

    // Parse due_at: required for oneshot/tracking, optional for recurring (computed from repeat)
    let ts = if let Some(ref due_at) = args.due_at {
        parse_datetime_to_timestamp(due_at, &tz)?
    } else if args.item_type == "recurring" {
        if let Some(ref repeat) = args.repeat {
            let tags = crate::proactive::utils::ParsedTags {
                repeat: Some(repeat.clone()),
                ..Default::default()
            };
            let next = crate::proactive::utils::compute_next_check_at(&tags, &tz_str)
                .ok_or("Could not compute first due_at from repeat pattern")?;
            parse_datetime_to_timestamp(&next, &tz)?
        } else {
            return Err("Recurring items need a repeat pattern.".into());
        }
    } else if args.item_type == "tracking" {
        // Tracking without explicit deadline: default to 30 days from now
        let default_expiry = chrono::Utc::now().with_timezone(&tz) + chrono::Duration::days(30);
        parse_datetime_to_timestamp(&default_expiry.format("%Y-%m-%dT%H:%M").to_string(), &tz)?
    } else {
        let user_now = chrono::Utc::now().with_timezone(&tz);
        return Err(format!(
            "due_at is required for oneshot items. Current time in user timezone: {}. Provide due_at as YYYY-MM-DDTHH:MM.",
            user_now.format("%Y-%m-%dT%H:%M")
        ).into());
    };

    // Auto-infer fetch sources if AI omitted them for a non-tracking summary/digest item
    if args.fetch.is_none() && args.item_type != "tracking" {
        let lower = args.description.to_lowercase();
        let is_summary = lower.contains("summarize")
            || lower.contains("summary")
            || lower.contains("digest")
            || lower.contains("check")
            || lower.contains("report")
            || lower.contains("recap");
        if is_summary {
            let mut sources = Vec::new();
            if lower.contains("email") || lower.contains("mail") {
                sources.push("email");
            }
            if lower.contains("whatsapp")
                || lower.contains("telegram")
                || lower.contains("signal")
                || lower.contains("message")
                || lower.contains("chat")
            {
                sources.push("chat");
            }
            if lower.contains("calendar") || lower.contains("event") || lower.contains("schedule") {
                sources.push("calendar");
            }
            if lower.contains("weather")
                || lower.contains("forecast")
                || lower.contains("temperature")
            {
                sources.push("weather");
            }
            if lower.contains("tracked item") || lower.contains("tracked things") {
                sources.push("items");
            }
            if lower.contains("price")
                || lower.contains("stock")
                || lower.contains("bitcoin")
                || lower.contains("btc")
                || lower.contains("crypto")
                || lower.contains("market")
                || lower.contains("news")
                || lower.contains("score")
            {
                sources.push("internet");
            }
            if !sources.is_empty() {
                tracing::info!(
                    "Auto-inferred fetch sources [{}] from description: {}",
                    sources.join(","),
                    args.description
                );
                args.fetch = Some(sources.join(","));
            }
        }
    }

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
        due_at: Some(ts),
        priority,
        source_id: None,
        created_at: now,
    };

    let item_id = state
        .item_repository
        .create_item(&new_item)
        .map_err(|e| format!("Failed to create item: {:?}", e))?;

    let time_label = {
        use chrono::TimeZone;
        chrono::Utc
            .timestamp_opt(ts as i64, 0)
            .single()
            .map(|t| t.with_timezone(&tz).format("%b %d %I:%M%p").to_string())
            .unwrap_or_default()
    };

    let confirmation = match args.item_type.as_str() {
        "tracking" => format!("Tracking: {} (expires {})", args.description, time_label),
        "recurring" => {
            let repeat = args.repeat.as_deref().unwrap_or("scheduled");
            format!(
                "Scheduled {}: {} (next: {})",
                repeat, args.description, time_label
            )
        }
        _ => format!("Reminder set for {}: {}", time_label, args.description),
    };

    Ok(CreateItemResult {
        message: confirmation,
        task_id: item_id,
    })
}

/// Build summary string from structured CreateItemArgs.
/// Format: "[type:X] [notify:Y] [optional tags...]\nDescription text"
pub fn build_summary_from_params(args: &CreateItemArgs) -> String {
    let mut tags = Vec::new();

    tags.push(format!("[type:{}]", args.item_type));
    tags.push(format!("[notify:{}]", args.notify));

    if let Some(ref repeat) = args.repeat {
        tags.push(format!("[repeat:{}]", repeat));
    }
    if let Some(ref fetch) = args.fetch {
        tags.push(format!("[fetch:{}]", fetch));
    }

    // Tracking tags (skip "none" values - means not applicable)
    if let Some(ref platform) = args.platform {
        if platform != "none" {
            tags.push(format!("[platform:{}]", platform));
        }
    }
    if let Some(ref sender) = args.sender {
        if sender != "none" {
            tags.push(format!("[sender:{}]", sender));
        }
    }
    if let Some(ref topic) = args.topic {
        if topic.to_lowercase() == "any" {
            tags.push("[scope:any]".to_string());
        } else if topic != "none" {
            tags.push(format!("[topic:{}]", topic));
        }
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
