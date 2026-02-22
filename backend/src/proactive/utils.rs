use crate::models::user_models::ContactProfile;
use crate::repositories::user_repository::LogUsageParams;
use crate::AppState;
use crate::UserCoreOps;
use openai_api_rs::v1::{chat_completion, types};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::context::ContextBuilder;
use chrono::Timelike;
use chrono::{Duration, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

/// Parse an ISO datetime string to a unix timestamp (seconds).
///
/// Handles:
/// - `2026-02-28T09:00:00Z` (full datetime with Z)
/// - `2026-02-28T09:00:00` (full datetime, assumed UTC)
/// - `2026-02-28` (date only, noon UTC)
///
/// Returns `None` on parse failure.
pub fn parse_iso_to_timestamp(s: &str) -> Option<i32> {
    let s = s.trim();
    // Try full datetime with Z suffix
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp() as i32);
    }
    // Try with Z appended (input like "2026-02-28T09:00:00")
    if s.contains('T') && !s.ends_with('Z') {
        let with_z = format!("{}Z", s);
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&with_z) {
            return Some(dt.timestamp() as i32);
        }
        // Try NaiveDateTime parse
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
            return Some(ndt.and_utc().timestamp() as i32);
        }
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
            return Some(ndt.and_utc().timestamp() as i32);
        }
    }
    // Try date-only (noon UTC)
    if let Ok(nd) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return nd
            .and_hms_opt(12, 0, 0)
            .map(|ndt| ndt.and_utc().timestamp() as i32);
    }
    None
}

/// Definition of a **critical** message: something that will cause human‑safety risk,
/// major financial/data loss, legal breach, or production outage if it waits >2 h.
/// The model must default to *non‑critical* when uncertain.
/// Prompt for matching incoming messages against the user’s *waiting checks*.
/// A waiting check represents something the user explicitly asked to be notified
/// about (e.g. \"Tell me when the shipment arrives\").
const WAITING_CHECK_PROMPT: &str = r#"You are an AI that determines whether an incoming message matches **one** of the user's monitor items listed below. Each item's 'Content' describes what the user wants to track.

    **Match rules**
    - Interpret the item 'Content' as the user's condition or instruction for matching.
    - If the content is descriptive (>5 words), use semantic reasoning. Translate non-English text internally.
    - If the content is short (<=5 words), require the message to contain *all* those words (case-insensitive).
    - A match must be *unambiguous*. Ambiguous, partial, or sender-only matches DO NOT count.
    - If multiple items could match, choose the single *best* match. Return `null` if none match.

    **Resolution rules**
    Set `is_resolved=true` when the tracked thing is clearly complete:
    - Payment confirmed, invoice paid
    - Package delivered (not just shipped or in transit)
    - Question answered definitively
    - Appointment completed
    - Document signed/approved
    Use common sense for `should_notify` on resolution: only notify if the user would actually
    want to hear about it. A routine invoice being paid (user probably paid it themselves) does
    NOT need a notification. A package delivered after delays, or an important question answered,
    might warrant one. When in doubt on resolution, do NOT notify - just silently resolve.

    **Lifecycle rules**
    - `next_check_at`: Set an ISO datetime for the next scheduled check if the item needs follow-up.
      Example: package shipped today, set next_check_at to 2 days from now. Leave null if no follow-up needed.
    - `priority`: 0 = background (digest only, no standalone notification), 1 = SMS, 2 = phone call.
      Only escalate if the situation warrants it (e.g. approaching deadline, overdue payment).

    **Notification decision**
    When a match is found, decide whether to **notify** the user or just **silently update** the item:
    - Set `should_notify=true` if the match represents something the user would want to know RIGHT NOW
      (e.g., "mom replied", "package delivered", "flight delayed", "job application response").
    - Set `should_notify=false` if this is just a background tracking update that doesn't need immediate attention
      (e.g., routine shipping status "in transit", background price tracking, periodic status update).
    - When in doubt, notify.

    **Updated summary**
    Provide an `updated_summary` that incorporates what was matched. This replaces the item's current summary.
    Keep it concise (max 120 chars). Include key details from the match.
    Example: "Watch for emails from mom" -> "Mom replied: 'Sure, dinner at 7 works!'"

    If a match is found AND should_notify=true, craft a short notification:
    `sms_message` (<=160 chars) - concise SMS describing the event.
"#;

const CRITICAL_PROMPT: &str = r#"You are an AI that decides whether an incoming user message is **critical** — i.e. it must be surfaced within **two hours** and cannot wait for the next scheduled summary.
A message is **critical** if delaying action beyond 2 h risks:
• Direct harm to people
• Severe data loss or major financial loss
• Production system outage or security breach
• Hard legal/compliance deadline expiring in ≤ 2 h
• The sender explicitly says it must be handled immediately (e.g. “ASAP”, “emergency”, “right now”) or gives a ≤ 2 h deadline.
• Time-sensitive personal or social requests/opportunities with an implied or stated window of ≤2 hours (e.g., invitations for immediate events like lunch, or quick decisions needed right now).
Everything else — vague urgency, routine updates, or unclear requests — is **NOT** critical.
If unsure, choose **not critical**.
---
### Process
1. Detect the message language; translate internally to English before reasoning.
2. Identify any explicit or implied time windows (e.g., "now," "soon," "today at noon," or contexts like being at a location requiring immediate input).
3. Apply the criteria **strictly**.
4. Produce JSON with these fields (do **not** add others):
| Field | Required? | Max chars | Content rules |
|-------|-----------|-----------|---------------|
| `is_critical` | always | — | Boolean. |
| `what_to_inform` | *only when* `is_critical==true` | 160 | **One SMS sentence** that:<br> • Briefly summarizes the core problem/ask (who/what/when).<br> • States the single most urgent next action the recipient must take within 2 h. Remember to include the sender or the Chat the message is from. |
| `first_message` | *only when* `is_critical==true` | 100 | **Voice-assistant opener** that grabs attention and repeats the required action in imperative form. |
If `is_critical` is false, leave the other two fields empty strings.
---
#### Examples
**Incoming:**
“I'm at the store should I buy eggs or do we have some still?”
**Output:**
{
  "is_critical": true,
  "what_to_inform": "Rasmus is asking on WhatsApp if he needs to buy eggs as well",
  "first_message": "Hey, Rasmus needs more information about the shopping list"
}

**Incoming**:
"Hey, want to grab lunch? I'm free until 1 PM."
**Output**:
{
  "is_critical": true,
  "what_to_inform": "Alex is inviting you to lunch on WhatsApp",
  "first_message": "Alex is asking if you want to crab lunch!"
}

**Incoming**:
"Weekly team update: Project is on track."
**Output**:
{
  "is_critical": false,
  "what_to_inform": "",
  "first_message": ""
}
"#;

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemMatchResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<i32>, // actually item_id - kept as task_id for LLM schema compat

    #[serde(default)]
    pub should_notify: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_summary: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sms_message: Option<String>,

    // Lifecycle fields - LLM-controlled monitor management
    #[serde(default)]
    pub is_resolved: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_check_at: Option<String>, // ISO datetime string, parsed at call site

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>, // 0/1/2 escalation
}

/// Determine whether `message` matches **one** of the supplied monitor items.
/// Uses the item's summary as the matching condition.
/// Returns the full `ItemMatchResponse` with match decision, notification decision,
/// updated summary, and optional notification messages.
pub async fn check_item_monitor_match(
    state: &Arc<AppState>,
    user_id: i32,
    message: &str,
    items: &[crate::models::user_models::Item],
) -> Result<Option<ItemMatchResponse>, Box<dyn std::error::Error>> {
    let ctx = ContextBuilder::for_user(state, user_id).build().await?;

    let items_str = items
        .iter()
        .map(|item| format!("ID: {}, Content: {}", item.id.unwrap_or(-1), item.summary))
        .collect::<Vec<_>>()
        .join("\n");

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(WAITING_CHECK_PROMPT.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(format!(
                "Current time: {}\n\nIncoming message:\n\n{}\n\nMonitor items:\n\n{}\n\nReturn the best match or null.",
                chrono::Utc::now().to_rfc3339(), message, items_str
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let mut properties = std::collections::HashMap::new();
    properties.insert(
        "task_id".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some("ID of the matched item, or null if no match".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "should_notify".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "Whether to notify the user now. true for actionable/important matches, \
                false for background tracking updates that don't need immediate attention."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );
    properties.insert(
        "updated_summary".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Updated summary incorporating match details (max 120 chars). \
                Replaces the item's current summary."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );
    properties.insert(
        "sms_message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Concise SMS (max 160 chars) when matched AND should_notify=true, else empty"
                    .to_string(),
            ),
            ..Default::default()
        }),
    );
    properties.insert(
        "is_resolved".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "Set true when the tracked thing is clearly complete (delivered, paid, answered). \
                Item will be deleted. Default false."
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
                "ISO datetime for next scheduled check, e.g. '2026-03-01T09:00:00Z'. \
                Set if the item needs follow-up later. Null if no follow-up needed."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );
    properties.insert(
        "priority".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some(
                "Escalate importance: 0 = background/digest-only, 1 = standalone notification, \
                2 = urgent. Only change if situation warrants."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );
    let tools = vec![chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "analyze_item_match".to_string(),
            description: Some(
                "Determines whether the message matches a monitor item, evaluates lifecycle \
                (resolution, next check, priority escalation), and provides an updated summary."
                    .to_string(),
            ),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec!["task_id".to_string(), "should_notify".to_string()]),
            },
        },
    }];

    let request = chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages)
        .tools(tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .temperature(0.0)
        .max_tokens(500);

    let result = ctx.client.chat_completion(request).await?;
    let tool_call = result.choices[0]
        .message
        .tool_calls
        .as_ref()
        .and_then(|tc| tc.first())
        .ok_or("No tool call in item monitor response")?;

    let args = tool_call
        .function
        .arguments
        .as_ref()
        .ok_or("No arguments in item monitor tool call")?;

    let response: ItemMatchResponse = serde_json::from_str(args)?;

    if response.task_id.is_some() {
        Ok(Some(response))
    } else {
        Ok(None)
    }
}

/// Apply monitor lifecycle changes from an `ItemMatchResponse` to an item.
///
/// Handles:
/// - Resolution: deletes the item, optionally notifies
/// - Update: applies summary, next_check_at, priority changes
/// - Priority gate: only sends standalone notification if effective priority >= 1
///
/// Safety guards on next_check_at:
/// - LLM returns null + item has due_at: fall back to heuristic (check in 1-3 days)
/// - LLM returns date >30 days out: cap at 30 days
/// - LLM returns date in the past: clamp to now + 3600
pub async fn apply_monitor_lifecycle(
    state: &Arc<AppState>,
    item: &crate::models::user_models::Item,
    response: &ItemMatchResponse,
) {
    let item_id = item.id.unwrap_or(0);
    let user_id = item.user_id;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Resolution: delete item
    if response.is_resolved {
        let _ = state.item_repository.delete_item(item_id, user_id);
        tracing::debug!("Monitor item {} resolved and deleted", item_id);

        // Notify about resolution if should_notify is true and priority >= 1
        let effective_priority = response.priority.unwrap_or(item.priority);
        if response.should_notify && effective_priority >= 1 {
            let notification_type = if effective_priority >= 2 {
                "call"
            } else {
                "sms"
            };
            let message = response
                .sms_message
                .clone()
                .unwrap_or_else(|| format!("Resolved: {}", item.summary));
            let opener = "Hey, you have a notification!".to_string();

            let state_clone = state.clone();
            tokio::spawn(async move {
                send_notification(
                    &state_clone,
                    user_id,
                    &message,
                    format!("monitor_{}", notification_type),
                    Some(opener),
                )
                .await;
            });
        }
        return;
    }

    // Update item fields
    let updated_summary = response
        .updated_summary
        .clone()
        .unwrap_or_else(|| item.summary.clone());
    let effective_priority = response.priority.unwrap_or(item.priority);

    // Parse and apply safety guards to next_check_at
    let max_future = now + 30 * 86400; // 30 days cap
    let parsed_next_check = response
        .next_check_at
        .as_deref()
        .and_then(parse_iso_to_timestamp);

    let safe_next_check = match parsed_next_check {
        Some(ts) if ts <= now => Some(now + 3600), // Past: clamp to now + 1 hour
        Some(ts) if ts > max_future => Some(max_future), // >30 days: cap
        Some(ts) => Some(ts),
        None => {
            // LLM returned null: if item has due_at, fall back to heuristic
            if let Some(due) = item.due_at {
                let days_until = (due - now) / 86400;
                let check_in = if days_until <= 2 {
                    86400 // 1 day
                } else if days_until <= 7 {
                    2 * 86400 // 2 days
                } else {
                    3 * 86400 // 3 days
                };
                Some(now + check_in)
            } else {
                item.next_check_at // Keep existing (likely None for match-only monitors)
            }
        }
    };

    let _ = state.item_repository.update_item(
        item_id,
        user_id,
        &updated_summary,
        safe_next_check,
        effective_priority,
    );

    // Notification: only send standalone if effective priority >= 1
    if response.should_notify && effective_priority >= 1 {
        let notification_type = if effective_priority >= 2 {
            "call"
        } else {
            "sms"
        };
        let message = response
            .sms_message
            .clone()
            .unwrap_or_else(|| format!("Monitor matched: {}", item.summary));
        let opener = "Hey, you have a notification!".to_string();

        let state_clone = state.clone();
        tokio::spawn(async move {
            send_notification(
                &state_clone,
                user_id,
                &message,
                format!("monitor_{}", notification_type),
                Some(opener),
            )
            .await;
        });
    } else if response.should_notify {
        tracing::debug!(
            "Monitor item {} matched with should_notify but priority {} < 1, digest only",
            item_id,
            effective_priority
        );
    }
}

/// Result from processing a triggered item via LLM.
#[derive(Debug, Deserialize)]
pub struct TriggeredItemResult {
    /// Updated summary for the item (return unchanged for recurring items)
    pub summary: String,
    /// If present, notify the user with this message (max 480 chars for digest, 160 for simple)
    #[serde(default)]
    pub sms_message: Option<String>,
    /// ISO datetime for next trigger; omit if item is done (will be deleted)
    #[serde(default)]
    pub next_check_at: Option<String>,
    /// 0 = background/digest-only, 1 = SMS notification, 2 = phone call
    #[serde(default)]
    pub priority: Option<i32>,
}

const PROCESS_TRIGGERED_ITEM_PROMPT: &str = r#"You are an AI assistant that processes triggered items for a user. An item just fired at its scheduled time. Your job:

1. Read the item summary carefully. It describes what to do.
2. If the summary mentions fetching data (emails, messages, calendar, weather), use the available fetch tools to gather that information first.
3. Once you have all needed information, call `process_item_result` with your result.

**Available fetch tools:**
- `fetch_emails` - Fetch the user's recent emails. No parameters needed.
- `fetch_recent_messages` - Fetch recent chat messages. Parameters: `hours_back` (int, default 8), `room_id` (optional string to filter by room).
- `fetch_calendar_events` - Fetch upcoming calendar events. Parameters: `days_ahead` (int, default 1).
- `get_weather` - Fetch weather data. Parameters: `location` (string), `units` ("metric"/"imperial"), `forecast_type` ("current"/"forecast").

**Timezone:** The user message includes their local time and timezone. Produce any `next_check_at` datetimes in the user's timezone as ISO format (e.g. "2026-03-01T09:00").

**RULES:**
- `summary`: For recurring items with rescheduling instructions, keep the original instructions intact but you may append useful updates. For one-shot items, update freely.
- `sms_message`: Craft a concise notification for the user. Max 480 chars. Use second person ('you'). Be direct and informative. Omit if no notification needed.
- `next_check_at`: If the summary contains rescheduling instructions (e.g. "Reschedule for tomorrow 9am"), follow them - set next_check_at to the appropriate ISO datetime in the user's timezone. If there are no rescheduling instructions, this is a one-time item - omit next_check_at (the item will be deleted).
- `priority`: 0 = background only (no notification sent), 1 = SMS, 2 = phone call. Default to the item's current priority unless the summary says [VIA CALL] (use 2) or explicitly says no notification needed (use 0).

**EXAMPLES:**
- Simple reminder: summary="Remind the user to call mom" -> sms_message="Time to call mom!", no next_check_at (one-shot).
- Call reminder: summary="Scheduled check-in [VIA CALL]" -> sms_message="Your scheduled check-in.", priority=2, no next_check_at.
- Recurring digest: summary="Daily email digest. Reschedule for tomorrow at 9am" -> fetch_emails, sms_message="You got 3 emails: ...", next_check_at="2026-03-02T09:00", summary keeps rescheduling instructions.
- Data-fetching item: summary="Check weather in Helsinki and notify the user" -> get_weather, sms_message="Helsinki: 5C, light rain.", no next_check_at."#;

/// Process a triggered item using LLM with optional tool-calling loop.
/// Replaces the old `generate_item_notification` with a unified approach
/// that supports both simple reminders and digest/recurring items.
pub async fn process_triggered_item(
    state: &Arc<AppState>,
    user_id: i32,
    item: &crate::models::user_models::Item,
) -> Result<TriggeredItemResult, Box<dyn std::error::Error>> {
    let ctx = ContextBuilder::for_user(state, user_id)
        .with_user_context()
        .build()
        .await?;

    let (time_str, tz_label) = if let Some(ref tz) = ctx.timezone {
        (tz.formatted_now.clone(), tz.tz_str.clone())
    } else {
        (chrono::Utc::now().to_rfc3339(), "UTC".to_string())
    };

    let mut messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(PROCESS_TRIGGERED_ITEM_PROMPT.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(format!(
                "Current time: {} (timezone: {})\nItem summary: {}\nItem priority: {}",
                time_str, tz_label, item.summary, item.priority
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    // Build the final result tool
    let mut result_properties = std::collections::HashMap::new();
    result_properties.insert(
        "summary".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Updated item summary. For recurring items, keep rescheduling instructions but may append updates."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );
    result_properties.insert(
        "sms_message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Notification text for the user (max 480 chars). Omit if no notification needed."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );
    result_properties.insert(
        "next_check_at".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "ISO datetime for next trigger (e.g. '2026-03-01T09:00'). Omit if item is done."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );
    result_properties.insert(
        "priority".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some(
                "0 = background (no notification), 1 = SMS notification, 2 = phone call"
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    let result_tool = chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "process_item_result".to_string(),
            description: Some("Return the processing result for this triggered item.".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(result_properties),
                required: Some(vec!["summary".to_string()]),
            },
        },
    };

    // Assemble fetch tools + result tool
    let tools = vec![
        crate::tool_call_utils::email::get_fetch_emails_tool(),
        crate::tool_call_utils::bridge::get_fetch_recent_messages_tool(),
        crate::tool_call_utils::calendar::get_fetch_calendar_event_tool(),
        crate::tool_call_utils::internet::get_weather_tool(),
        result_tool,
    ];

    // Tool-calling loop (max 5 iterations)
    for iteration in 0..5 {
        let request =
            chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages.clone())
                .tools(tools.clone())
                .tool_choice(chat_completion::ToolChoiceType::Required)
                .temperature(0.0)
                .max_tokens(800);

        let result = ctx.client.chat_completion(request).await?;
        let choice = result.choices.first().ok_or("No choices in LLM response")?;

        let tool_calls = choice
            .message
            .tool_calls
            .as_ref()
            .ok_or("No tool calls in response")?;

        // Process each tool call
        let mut found_result = None;
        let assistant_content = choice.message.content.clone().unwrap_or_default();

        // Add assistant message with tool calls
        messages.push(chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::assistant,
            content: chat_completion::Content::Text(assistant_content),
            name: None,
            tool_calls: Some(tool_calls.clone()),
            tool_call_id: None,
        });

        for tc in tool_calls {
            let fn_name = tc.function.name.as_deref().unwrap_or("");
            let fn_args = tc.function.arguments.as_deref().unwrap_or("{}");
            let tc_id = tc.id.clone();

            if fn_name == "process_item_result" {
                // Parse and return
                let parsed: TriggeredItemResult = serde_json::from_str(fn_args)?;
                found_result = Some(parsed);
                // Add tool response to keep message structure valid
                messages.push(chat_completion::ChatCompletionMessage {
                    role: chat_completion::MessageRole::tool,
                    content: chat_completion::Content::Text("OK".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: Some(tc_id),
                });
                break;
            }

            // Execute fetch tool
            let tool_output = match fn_name {
                "fetch_emails" => {
                    crate::tool_call_utils::email::handle_fetch_emails(state, user_id).await
                }
                "fetch_recent_messages" => {
                    crate::tool_call_utils::bridge::handle_fetch_recent_messages(
                        state, user_id, fn_args,
                    )
                    .await
                }
                "fetch_calendar_events" => {
                    crate::tool_call_utils::calendar::handle_fetch_calendar_events(
                        state, user_id, fn_args,
                    )
                    .await
                }
                "get_weather" => {
                    let weather_args: serde_json::Value =
                        serde_json::from_str(fn_args).unwrap_or_default();
                    let location = weather_args
                        .get("location")
                        .and_then(|v| v.as_str())
                        .unwrap_or("current location");
                    let units = weather_args
                        .get("units")
                        .and_then(|v| v.as_str())
                        .unwrap_or("metric");
                    let forecast_type = weather_args
                        .get("forecast_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("current");
                    match crate::utils::tool_exec::get_weather(
                        state,
                        location,
                        units,
                        forecast_type,
                        user_id,
                    )
                    .await
                    {
                        Ok(answer) => answer,
                        Err(e) => format!("Weather fetch failed: {}", e),
                    }
                }
                _ => format!("Unknown tool: {}", fn_name),
            };

            tracing::debug!(
                "process_triggered_item iteration {}: tool={} output_len={}",
                iteration,
                fn_name,
                tool_output.len()
            );

            messages.push(chat_completion::ChatCompletionMessage {
                role: chat_completion::MessageRole::tool,
                content: chat_completion::Content::Text(tool_output),
                name: None,
                tool_calls: None,
                tool_call_id: Some(tc_id),
            });
        }

        if let Some(result) = found_result {
            return Ok(result);
        }
    }

    Err("process_triggered_item: LLM did not call process_item_result after 5 iterations".into())
}

#[derive(Debug, Serialize)]
pub struct DigestData {
    pub messages: Vec<MessageInfo>,
    pub calendar_events: Vec<CalendarEvent>,
    pub time_period_hours: u32,
    pub current_datetime_local: String, // Current date/time in user's timezone for relative timestamp calculation
}

#[derive(Debug, Serialize)]
pub struct MessageInfo {
    pub sender: String,
    pub content: String,
    pub timestamp_rfc: String,
    pub platform: String, // e.g., "email", "whatsapp", "telegram", "signal" etc.
    pub room_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CalendarEvent {
    pub title: String,
    pub start_time_rfc: String,
    pub duration_minutes: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchResponse {
    pub is_critical: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub what_to_inform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_message: Option<String>,
}

/// Result of processing contact profiles for digest filtering
pub struct DigestContactMaps {
    pub priority_map: HashMap<String, HashSet<String>>,
}

/// Builds priority and ignore maps from contact profiles, then filters messages
/// Returns the maps and mutates the messages vec to remove ignored contacts
fn build_contact_maps_and_filter_messages(
    state: &Arc<AppState>,
    user_id: i32,
    messages: &mut Vec<MessageInfo>,
) -> DigestContactMaps {
    let mut priority_map: HashMap<String, HashSet<String>> = HashMap::new();
    let mut ignore_map: HashMap<String, HashSet<String>> = HashMap::new();
    // Room ID-based maps for stable matching
    let mut room_id_ignore_map: HashMap<String, HashSet<String>> = HashMap::new();
    let mut room_id_priority_map: HashMap<String, HashSet<String>> = HashMap::new();
    let profiles = state
        .user_repository
        .get_contact_profiles(user_id)
        .unwrap_or_default();

    for profile in &profiles {
        let profile_id = profile.id.unwrap_or(0);
        // Get all exceptions for this profile
        let exceptions = state
            .user_repository
            .get_profile_exceptions(profile_id)
            .unwrap_or_default();

        // Helper to check if a platform has an exception and get its mode
        let get_effective_mode = |platform: &str| -> String {
            exceptions
                .iter()
                .find(|e| e.platform == platform)
                .map(|e| e.notification_mode.clone())
                .unwrap_or_else(|| profile.notification_mode.clone())
        };

        // Helper to insert into room_id maps
        let mut insert_room_id = |platform: &str, room_id: &Option<String>, mode: &str| {
            if let Some(ref rid) = room_id {
                if mode == "ignore" {
                    room_id_ignore_map
                        .entry(platform.to_string())
                        .or_default()
                        .insert(rid.clone());
                } else {
                    room_id_priority_map
                        .entry(platform.to_string())
                        .or_default()
                        .insert(rid.clone());
                }
            }
        };

        if let Some(ref wa) = profile.whatsapp_chat {
            let mode = get_effective_mode("whatsapp");
            insert_room_id("whatsapp", &profile.whatsapp_room_id, &mode);
            if mode == "ignore" {
                ignore_map
                    .entry("whatsapp".to_string())
                    .or_default()
                    .insert(wa.to_lowercase());
            } else {
                priority_map
                    .entry("whatsapp".to_string())
                    .or_default()
                    .insert(wa.to_lowercase());
            }
        }
        if let Some(ref tg) = profile.telegram_chat {
            let mode = get_effective_mode("telegram");
            insert_room_id("telegram", &profile.telegram_room_id, &mode);
            if mode == "ignore" {
                ignore_map
                    .entry("telegram".to_string())
                    .or_default()
                    .insert(tg.to_lowercase());
            } else {
                priority_map
                    .entry("telegram".to_string())
                    .or_default()
                    .insert(tg.to_lowercase());
            }
        }
        if let Some(ref sig) = profile.signal_chat {
            let mode = get_effective_mode("signal");
            insert_room_id("signal", &profile.signal_room_id, &mode);
            if mode == "ignore" {
                ignore_map
                    .entry("signal".to_string())
                    .or_default()
                    .insert(sig.to_lowercase());
            } else {
                priority_map
                    .entry("signal".to_string())
                    .or_default()
                    .insert(sig.to_lowercase());
            }
        }
        if let Some(ref emails) = profile.email_addresses {
            let mode = get_effective_mode("email");
            for email in emails.split(',') {
                if mode == "ignore" {
                    ignore_map
                        .entry("email".to_string())
                        .or_default()
                        .insert(email.trim().to_lowercase());
                } else {
                    priority_map
                        .entry("email".to_string())
                        .or_default()
                        .insert(email.trim().to_lowercase());
                }
            }
        }
    }

    if !profiles.is_empty() {
        tracing::debug!("Loaded {} contact profiles for digest", profiles.len());
    }

    // Filter out messages from ignored contacts
    // Check room_id first (stable), then fall back to display name matching
    let original_count = messages.len();
    messages.retain(|msg| {
        // Check room_id-based ignore first
        if let Some(ref rid) = msg.room_id {
            if room_id_ignore_map
                .get(&msg.platform)
                .is_some_and(|set| set.contains(rid))
            {
                return false;
            }
            // If room_id matches a priority profile, keep it
            if room_id_priority_map
                .get(&msg.platform)
                .is_some_and(|set| set.contains(rid))
            {
                return true;
            }
        }
        // Fall back to display name matching for legacy profiles
        let sender_lower = msg.sender.to_lowercase();
        !ignore_map.get(&msg.platform).is_some_and(|set| {
            set.iter()
                .any(|s| sender_lower.contains(s) || s.contains(&sender_lower))
        })
    });
    if messages.len() < original_count {
        tracing::debug!(
            "Filtered out {} messages from ignored contacts",
            original_count - messages.len()
        );
    }

    DigestContactMaps { priority_map }
}

/// Resolves a sender name to a contact profile nickname if one exists.
/// Tries room_id match first (exact, stable), then falls back to display name substring match.
fn resolve_sender_name(
    profiles: &[ContactProfile],
    platform: &str,
    chat_name: &str,
    room_id: Option<&str>,
) -> String {
    // Try room_id match first (stable identifier)
    if let Some(rid) = room_id {
        if let Some(p) = profiles.iter().find(|p| {
            let profile_room_id = match platform {
                "whatsapp" => p.whatsapp_room_id.as_deref(),
                "telegram" => p.telegram_room_id.as_deref(),
                "signal" => p.signal_room_id.as_deref(),
                _ => None,
            };
            profile_room_id == Some(rid)
        }) {
            return p.nickname.clone();
        }
    }

    // Fall back to display name substring match (legacy profiles without room_id)
    let chat_lower = chat_name.to_lowercase();
    profiles
        .iter()
        .find_map(|p| {
            let profile_chat = match platform {
                "whatsapp" => p.whatsapp_chat.as_ref(),
                "telegram" => p.telegram_chat.as_ref(),
                "signal" => p.signal_chat.as_ref(),
                "email" => p.email_addresses.as_ref(),
                _ => None,
            }?;
            let profile_lower = profile_chat.to_lowercase();
            if chat_lower.contains(&profile_lower) || profile_lower.contains(&chat_lower) {
                Some(p.nickname.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| chat_name.to_string())
}

/// Formats disconnection events into a notice string for digest inclusion.
/// Reads from items table (system disconnection items).
/// Does NOT delete the items - they persist in the dashboard for user action.
fn format_disconnection_notice(
    state: &Arc<AppState>,
    user_id: i32,
    timezone: &str,
) -> Option<String> {
    let all_items = match state.item_repository.get_items(user_id) {
        Ok(items) => items,
        Err(e) => {
            tracing::error!(
                "Failed to get items for user {} (disconnection notice): {}",
                user_id,
                e
            );
            return None;
        }
    };
    let items: Vec<_> = all_items
        .into_iter()
        .filter(|item| item.summary.starts_with("System:") && item.summary.contains("disconnected"))
        .collect();

    if items.is_empty() {
        return None;
    }

    // Parse the timezone
    let tz: chrono_tz::Tz = match timezone.parse() {
        Ok(tz) => tz,
        Err(_) => {
            tracing::warn!("Invalid timezone for user {}, using UTC", user_id);
            chrono_tz::UTC
        }
    };

    // Format each disconnection triage item
    let notices: Vec<String> = items
        .iter()
        .map(|item| {
            // Convert timestamp to user's timezone
            let datetime = chrono::DateTime::from_timestamp(item.created_at as i64, 0)
                .unwrap_or_else(Utc::now)
                .with_timezone(&tz);

            // Format time as "2pm" or "10am"
            let hour = datetime.hour();
            let (hour12, ampm) = if hour == 0 {
                (12, "am")
            } else if hour < 12 {
                (hour, "am")
            } else if hour == 12 {
                (12, "pm")
            } else {
                (hour - 12, "pm")
            };

            format!("NOTICE: {} at {}{}", item.summary, hour12, ampm)
        })
        .collect();

    Some(notices.join(". "))
}

/// Formats tracked items (monitors, reminders) into a TRACKING line for digest inclusion.
fn format_tracking_notice(state: &Arc<AppState>, user_id: i32) -> Option<String> {
    let items = match state.item_repository.get_items(user_id) {
        Ok(items) => items,
        Err(e) => {
            tracing::error!("Failed to get tracked items for user {}: {}", user_id, e);
            return None;
        }
    };

    if items.is_empty() {
        return None;
    }

    let summaries: Vec<String> = items.iter().map(|item| item.summary.clone()).collect();
    Some(format!("TRACKING: {}", summaries.join(", ")))
}

/// Checks whether a single message is critical.
/// Returns `(is_critical, what_to_inform, first_message)`.
#[allow(clippy::too_many_arguments)]
pub async fn check_message_importance(
    state: &Arc<AppState>,
    user_id: i32,
    message: &str,
    service: &str,
    chat_name: &str,
    raw_content: &str,
    contact_notes: Option<&str>,
    conversation_context: &str,
) -> Result<(bool, Option<String>, Option<String>), Box<dyn std::error::Error>> {
    // Special case for WhatsApp incoming calls
    if raw_content.contains("Incoming call") || raw_content.contains("Missed call") {
        let call_notify = state.user_core.get_call_notify(user_id).unwrap_or(true);
        if call_notify {
            // Trim for SMS
            let what_to_inform =
                format!("You have an incoming {} call from {}", service, chat_name);
            let first_message = format!(
                "Hello, you have an incoming WhatsApp call from {}.",
                chat_name
            );
            return Ok((true, Some(what_to_inform), Some(first_message)));
        } else {
            return Ok((false, None, None));
        }
    }
    // Build the chat payload ----------------------------------------------
    let ctx = ContextBuilder::for_user(state, user_id).build().await?;
    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(CRITICAL_PROMPT.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text({
                let mut prompt =
                    String::from("Analyze this message and decide if it is critical:\n\n");
                if let Some(notes) = contact_notes {
                    if !notes.is_empty() {
                        prompt.push_str(&format!("Contact notes from the user: \"{}\"\n\n", notes));
                    }
                }
                if !conversation_context.is_empty() {
                    prompt.push_str(&format!(
                        "Recent conversation:\n{}\n\n",
                        conversation_context
                    ));
                }
                prompt.push_str(&format!("New message: {}", message));
                prompt
            }),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];
    // JSON schema for the structured output -------------------------------
    let mut properties = std::collections::HashMap::new();
    properties.insert(
        "is_critical".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "Whether the message is critical and requires immediate attention".to_string(),
            ),
            ..Default::default()
        }),
    );
    properties.insert(
        "what_to_inform".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Concise SMS (≤160 chars) to send if the message is critical".to_string(),
            ),
            ..Default::default()
        }),
    );
    properties.insert(
        "first_message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Brief voice‑assistant opening line (≤100 chars) if critical".to_string(),
            ),
            ..Default::default()
        }),
    );
    let tools = vec![chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "analyze_message".to_string(),
            description: Some("Analyzes if a message is critical".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    "is_critical".to_string(),
                    "what_to_inform".to_string(),
                    "first_message".to_string(),
                ]),
            },
        },
    }];
    let request = chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages)
        .tools(tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        // Lower temperature for more deterministic classification
        .temperature(0.2)
        .max_tokens(200);
    // ---------------------------------------------------------------------
    match ctx.client.chat_completion(request).await {
        Ok(result) => {
            if let Some(tool_calls) = result.choices[0].message.tool_calls.as_ref() {
                if let Some(first_call) = tool_calls.first() {
                    if let Some(args) = &first_call.function.arguments {
                        match serde_json::from_str::<MatchResponse>(args) {
                            Ok(response) => {
                                tracing::debug!(target: "critical_check", ?response, "Message analysis result");
                                Ok((
                                    response.is_critical,
                                    response.what_to_inform,
                                    response.first_message,
                                ))
                            }
                            Err(e) => {
                                tracing::error!("Failed to parse message analysis response: {}", e);
                                Ok((false, None, None))
                            }
                        }
                    } else {
                        tracing::error!("No arguments found in tool call");
                        Ok((false, None, None))
                    }
                } else {
                    tracing::error!("No tool calls found");
                    Ok((false, None, None))
                }
            } else {
                tracing::error!("No tool calls section in response");
                Ok((false, None, None))
            }
        }
        Err(e) => {
            tracing::error!("Failed to get message analysis: {}", e);
            Err(e.into())
        }
    }
}

// ---------------------------------------------------------------------------
// Trackable item detection for auto-triage
// ---------------------------------------------------------------------------

const TRACKABLE_PROMPT: &str = r#"You are an AI that detects trackable items from emails. A "trackable item" is something the user might need to follow up on or track.

Use common sense. Only flag things with concrete actionable details. Skip:
- Marketing emails, newsletters, promotional content
- Social notifications (likes, follows, comments)
- Routine status updates without action needed
- If uncertain, say not trackable

Return JSON with:
- `is_trackable` (boolean): whether this email contains something worth tracking
- `summary` (string, max 80 chars): concise description e.g. "AWS invoice $45.20 due Feb 20" or "" if not trackable
- `due_date` (string): ISO date if a due/delivery date is mentioned, or "" if none
- `needs_monitoring` (boolean): true if the item should be actively watched for resolution (e.g. invoice awaiting payment, package in transit, pending approval). false for one-time info items.
"#;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TrackableResponse {
    is_trackable: bool,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    due_date: String,
    #[serde(default)]
    needs_monitoring: bool, // Included in LLM schema for future use
}

/// Checks whether an email contains a trackable item and silently creates
/// a monitor item if detected. All trackable items become monitors.
/// Uses source_id for dedup. Priority is based on due_at urgency.
pub async fn check_trackable_items(
    state: &Arc<AppState>,
    user_id: i32,
    email_uid: &str,
    email_content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Dedup: skip if we already have an item for this email
    let source_id = format!("email_{}", email_uid);
    if state
        .item_repository
        .item_exists_by_source(user_id, &source_id)?
    {
        return Ok(());
    }

    let ctx = ContextBuilder::for_user(state, user_id).build().await?;

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(TRACKABLE_PROMPT.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(format!(
                "Analyze this email for trackable items:\n\n{}",
                email_content
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let mut properties = std::collections::HashMap::new();
    properties.insert(
        "is_trackable".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("Whether the email contains something worth tracking".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "summary".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Concise summary of the trackable item (max 80 chars)".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "due_date".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("ISO date if a due/delivery date is mentioned".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "needs_monitoring".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "true if item should be actively watched for resolution (invoice, package, approval)"
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    let tools = vec![chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "analyze_trackable".to_string(),
            description: Some("Analyzes if an email contains a trackable item".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    "is_trackable".to_string(),
                    "summary".to_string(),
                    "due_date".to_string(),
                    "needs_monitoring".to_string(),
                ]),
            },
        },
    }];

    let request = chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages)
        .tools(tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .temperature(0.1)
        .max_tokens(200);

    let result = ctx.client.chat_completion(request).await?;

    let response = result.choices[0]
        .message
        .tool_calls
        .as_ref()
        .and_then(|tc| tc.first())
        .and_then(|tc| tc.function.arguments.as_ref())
        .and_then(|args| serde_json::from_str::<TrackableResponse>(args).ok());

    if let Some(resp) = response {
        if resp.is_trackable && !resp.summary.is_empty() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            // Parse due_date to unix timestamp
            let due_at = parse_iso_to_timestamp(&resp.due_date);

            // All trackable items become monitors.
            // Priority based on due_at urgency:
            // - No due_at or >7 days away: priority 0 (digest-only)
            // - 2-7 days away: priority 1 (standalone notifications)
            // - <2 days or overdue: priority 2 (urgent)
            let priority = match due_at {
                Some(due) => {
                    let days_until = (due - now) / 86400;
                    if days_until < 2 {
                        2
                    } else if days_until <= 7 {
                        1
                    } else {
                        0
                    }
                }
                None => 0,
            };

            // Initial next_check_at based on due_at proximity
            let next_check_at = match due_at {
                Some(due) => {
                    let days_until = (due - now) / 86400;
                    if days_until <= 2 {
                        Some(now + 86400) // Check in 1 day
                    } else if days_until <= 7 {
                        Some(now + 2 * 86400) // Check in 2 days
                    } else {
                        Some(now + 3 * 86400) // Check in 3 days
                    }
                }
                None => None, // No due_at: match-only, no scheduled checks
            };

            let new_item = crate::models::user_models::NewItem {
                user_id,
                summary: resp.summary,
                monitor: true,
                due_at,
                next_check_at,
                priority,
                source_id: Some(source_id),
                created_at: now,
            };

            if let Err(e) = state.item_repository.create_item(&new_item) {
                tracing::error!(
                    "Failed to create trackable item for user {}: {}",
                    user_id,
                    e
                );
            } else {
                tracing::debug!(
                    "Created monitor item for user {} (priority {})",
                    user_id,
                    priority
                );
            }
        }
    }

    Ok(())
}

/// Generates a suggested reply for a bridge message.
/// Returns (needs_reply, suggested_reply, reasoning) where:
/// - needs_reply: whether this message warrants a response
/// - suggested_reply: the AI-drafted reply text matching the user's style
/// - reasoning: why the AI suggested this reply (shown as helper text)
#[allow(clippy::too_many_arguments)]
pub async fn generate_suggested_reply(
    state: &Arc<AppState>,
    user_id: i32,
    service: &str,
    chat_name: &str,
    message_content: &str,
    recent_user_messages: &[String],
    conversation_history: &[(String, String, i64)],
    user_context: Option<&str>,
    contact_notes: Option<&str>,
) -> Result<(bool, Option<String>, Option<String>), Box<dyn std::error::Error>> {
    let ctx = ContextBuilder::for_user(state, user_id).build().await?;

    // Build context about the user's messaging style from their recent messages
    let style_examples = if recent_user_messages.is_empty() {
        "No previous messages available - use a casual, friendly tone.".to_string()
    } else {
        let examples: Vec<String> = recent_user_messages
            .iter()
            .take(5)
            .map(|m| format!("- {}", m))
            .collect();
        format!(
            "Examples of how this user writes messages:\n{}",
            examples.join("\n")
        )
    };

    let conversation_section = if conversation_history.is_empty() {
        String::new()
    } else {
        let lines: Vec<String> = conversation_history
            .iter()
            .map(|(sender, msg, ts)| {
                let dt = chrono::DateTime::from_timestamp(*ts, 0)
                    .map(|d| d.format("%b %d %H:%M").to_string())
                    .unwrap_or_default();
                format!("[{}] {}: {}", dt, sender, msg)
            })
            .collect();
        format!("\n\nRecent conversation history:\n{}", lines.join("\n"))
    };

    let context_section = match user_context {
        Some(ctx) => format!(
            "\n\nRelevant context about the user's schedule/tasks:\n{}",
            ctx
        ),
        None => String::new(),
    };

    let notes_section = match contact_notes {
        Some(notes) if !notes.is_empty() => {
            format!("\n\nUser's notes about this contact: \"{}\"", notes)
        }
        _ => String::new(),
    };

    let system_prompt = format!(
        r#"You are drafting a reply on behalf of a user to a message they received on {service}.
The reply will be sent from the user's account, so it must sound like them - not like an AI.

{style_examples}{conversation_section}{context_section}{notes_section}

Rules:
1. First decide: does this message NEED a reply? Messages like "love you", "ok", "thumbs up", memes, or broadcast messages do NOT need replies. Questions, requests, invitations, and time-sensitive asks DO.
2. If a reply is needed, draft one that matches the user's writing style (abbreviations, emoji usage, tone, language).
3. Keep it short and natural - how a real person would reply on {service}.
4. If the user has schedule conflicts or relevant context, incorporate that naturally.
5. Provide brief reasoning for your suggestion."#,
        service = service,
        style_examples = style_examples,
        conversation_section = conversation_section,
        context_section = context_section,
        notes_section = notes_section,
    );

    let user_prompt = format!(
        "Message from {} on {}:\n\"{}\"\n\nShould the user reply? If so, draft a reply.",
        chat_name, service, message_content
    );

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(system_prompt),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(user_prompt),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let mut properties = std::collections::HashMap::new();
    properties.insert(
        "needs_reply".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("Whether this message needs a reply from the user".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "suggested_reply".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "The drafted reply matching the user's writing style. Empty if no reply needed."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );
    properties.insert(
        "reasoning".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Brief explanation of why this reply was suggested or why no reply is needed"
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    let tools = vec![chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "draft_reply".to_string(),
            description: Some("Analyzes if a message needs a reply and drafts one".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    "needs_reply".to_string(),
                    "suggested_reply".to_string(),
                    "reasoning".to_string(),
                ]),
            },
        },
    }];

    let request = chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages)
        .tools(tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .temperature(0.7)
        .max_tokens(300);

    #[derive(Deserialize, Debug)]
    struct ReplyResponse {
        needs_reply: bool,
        suggested_reply: Option<String>,
        reasoning: Option<String>,
    }

    match ctx.client.chat_completion(request).await {
        Ok(result) => {
            if let Some(tool_calls) = result.choices[0].message.tool_calls.as_ref() {
                if let Some(first_call) = tool_calls.first() {
                    if let Some(args) = &first_call.function.arguments {
                        match serde_json::from_str::<ReplyResponse>(args) {
                            Ok(response) => {
                                tracing::debug!(target: "triage_reply", ?response, "Reply analysis result");
                                let reply = if response.needs_reply {
                                    response.suggested_reply.filter(|s| !s.is_empty())
                                } else {
                                    None
                                };
                                Ok((response.needs_reply, reply, response.reasoning))
                            }
                            Err(e) => {
                                tracing::error!("Failed to parse reply analysis response: {}", e);
                                Ok((false, None, None))
                            }
                        }
                    } else {
                        Ok((false, None, None))
                    }
                } else {
                    Ok((false, None, None))
                }
            } else {
                Ok((false, None, None))
            }
        }
        Err(e) => {
            tracing::error!("Failed to get reply analysis: {}", e);
            Err(e.into())
        }
    }
}

// Helper function to calculate hours until a target hour
fn hours_until(current_hour: u32, target_hour: u32) -> u32 {
    if current_hour <= target_hour {
        target_hour - current_hour
    } else {
        24 - (current_hour - target_hour)
    }
}

// Helper function to calculate hours since a previous hour
fn hours_since(current_hour: u32, previous_hour: u32) -> u32 {
    if current_hour >= previous_hour {
        current_hour - previous_hour
    } else {
        current_hour + (24 - previous_hour)
    }
}

pub async fn check_morning_digest(
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get the user's digest settings and timezone
    let (morning_digest, day_digest, evening_digest) = state.user_core.get_digests(user_id)?;
    let user_info = state.user_core.get_user_info(user_id)?;

    // If morning digest is enabled (Some value) and we have a timezone, check the time
    if let (Some(digest_hour_str), Some(timezone)) = (morning_digest.clone(), user_info.timezone) {
        // Parse the timezone
        let tz: chrono_tz::Tz = timezone
            .parse()
            .map_err(|e| format!("Invalid timezone: {}", e))?;

        // Get current time in user's timezone
        let now = chrono::Utc::now().with_timezone(&tz);

        // Parse the digest hour (expected format: "HH:00" like "00:00", "23:00")
        let digest_hour: u32 = digest_hour_str
            .split(':')
            .next()
            .ok_or("Invalid time format")?
            .parse()
            .map_err(|e| format!("Invalid hour in digest time: {}", e))?;

        // Validate hour is between 0-23
        if digest_hour > 23 {
            tracing::error!("Invalid hour value (must be 0-23): {}", digest_hour);
            return Ok(());
        }

        // Compare current hour with digest hour
        if now.hour() == digest_hour {
            // Calculate hours until next digest
            let hours_to_next = match (day_digest.as_ref(), evening_digest.as_ref()) {
                (Some(day), _) => {
                    let day_hour: u32 = day.split(':').next().unwrap_or("12").parse().unwrap_or(12);
                    hours_until(digest_hour, day_hour)
                }
                (None, Some(evening)) => {
                    let evening_hour: u32 = evening
                        .split(':')
                        .next()
                        .unwrap_or("18")
                        .parse()
                        .unwrap_or(18);
                    hours_until(digest_hour, evening_hour)
                }
                (None, None) => {
                    // If no other digests, calculate hours until midnight
                    hours_until(digest_hour, 0)
                }
            };

            // Calculate hours since previous digest
            let hours_since_prev = match evening_digest.as_ref() {
                Some(evening) => {
                    let evening_hour: u32 = evening
                        .split(':')
                        .next()
                        .unwrap_or("18")
                        .parse()
                        .unwrap_or(18);
                    hours_since(digest_hour, evening_hour)
                }
                None => {
                    // If no evening digest, calculate hours since midnight
                    hours_since(digest_hour, 0)
                }
            };

            // Format start time (now) and end time (now + hours_to_next) in RFC3339
            let start_time = now.with_timezone(&Utc).to_rfc3339();
            let end_time = (now + Duration::hours(hours_to_next as i64))
                .with_timezone(&Utc)
                .to_rfc3339();

            // Check if user has active Google Calendar before fetching events
            let calendar_events = match state.user_repository.has_active_google_calendar(user_id) {
                Ok(true) => {
                    match crate::handlers::google_calendar::handle_calendar_fetching(
                        state.as_ref(),
                        user_id,
                        &start_time,
                        &end_time,
                    )
                    .await
                    {
                        Ok(axum::Json(value)) => {
                            if let Some(events) = value.get("events").and_then(|e| e.as_array()) {
                                events
                                    .iter()
                                    .filter_map(|event| {
                                        let summary = event.get("summary")?.as_str()?.to_string();
                                        let start = event.get("start")?.as_str()?.parse().ok()?;
                                        let duration_minutes = event
                                            .get("duration_minutes")?
                                            .as_str()?
                                            .parse()
                                            .ok()?;
                                        Some(CalendarEvent {
                                            title: summary,
                                            start_time_rfc: start,
                                            duration_minutes,
                                        })
                                    })
                                    .collect()
                            } else {
                                Vec::new()
                            }
                        }
                        Err(_) => Vec::new(),
                    }
                }
                Ok(false) => {
                    tracing::debug!("User {} has no active Google Calendar", user_id);
                    Vec::new()
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to check Google Calendar status for user {}: {}",
                        user_id,
                        e
                    );
                    Vec::new()
                }
            };

            // Calculate the time range for message fetching
            let now = Utc::now();
            let cutoff_time = now - Duration::hours(hours_since_prev as i64);
            let start_timestamp = cutoff_time.timestamp();

            // Fetch contact profiles for resolving sender nicknames
            let contact_profiles = state
                .user_repository
                .get_contact_profiles(user_id)
                .unwrap_or_default();

            // Check if user has IMAP credentials before fetching emails
            let mut messages = match state.user_repository.get_imap_credentials(user_id) {
                Ok(Some(_)) => {
                    // Fetch and filter emails
                    match crate::handlers::imap_handlers::fetch_emails_imap(
                        state,
                        user_id,
                        false,
                        Some(50),
                        false,
                        true,
                    )
                    .await
                    {
                        Ok(emails) => {
                            emails
                                .into_iter()
                                .filter(|email| {
                                    // Filter emails based on timestamp
                                    if let Some(date) = email.date {
                                        date >= cutoff_time
                                    } else {
                                        false // Exclude emails without a timestamp
                                    }
                                })
                                .map(|email| MessageInfo {
                                    sender: email
                                        .from
                                        .unwrap_or_else(|| "Unknown sender".to_string()),
                                    content: email
                                        .snippet
                                        .unwrap_or_else(|| "No content".to_string()),
                                    timestamp_rfc: email
                                        .date_formatted
                                        .unwrap_or_else(|| "No Timestamp".to_string()),
                                    platform: "email".to_string(),
                                    room_id: None,
                                })
                                .collect::<Vec<MessageInfo>>()
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch emails for digest: {:#?}", e);
                            Vec::new()
                        }
                    }
                }
                Ok(None) => {
                    tracing::debug!(
                        "Skipping email fetch - user {} has no IMAP credentials configured",
                        user_id
                    );
                    Vec::new()
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to check IMAP credentials for user {}: {}",
                        user_id,
                        e
                    );
                    Vec::new()
                }
            };

            // Log the number of filtered email messages
            tracing::debug!(
                "Filtered {} email messages from the last {} hours for digest",
                messages.len(),
                hours_since_prev
            );

            // Fetch WhatsApp messages
            match state.user_repository.get_bridge(user_id, "whatsapp") {
                Ok(Some(_bridge)) => {
                    match crate::utils::bridge::fetch_bridge_messages(
                        "whatsapp",
                        state,
                        user_id,
                        start_timestamp,
                        true,
                    )
                    .await
                    {
                        Ok(whatsapp_messages) => {
                            // Convert WhatsAppMessage to MessageInfo and add to messages
                            let whatsapp_infos: Vec<MessageInfo> = whatsapp_messages
                                .into_iter()
                                .map(|msg| MessageInfo {
                                    sender: resolve_sender_name(
                                        &contact_profiles,
                                        "whatsapp",
                                        &msg.room_name,
                                        msg.room_id.as_deref(),
                                    ),
                                    content: msg.content,
                                    timestamp_rfc: msg.formatted_timestamp,
                                    platform: "whatsapp".to_string(),
                                    room_id: msg.room_id.clone(),
                                })
                                .collect();

                            tracing::debug!(
                                "Fetched {} WhatsApp messages from the last {} hours for digest",
                                whatsapp_infos.len(),
                                hours_since_prev
                            );

                            // Extend messages with WhatsApp messages
                            messages.extend(whatsapp_infos);

                            // Sort all messages by timestamp (most recent first)
                            messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch WhatsApp messages for digest: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    tracing::debug!("WhatsApp not connected for user {}", user_id);
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to check WhatsApp connection for user {}: {}",
                        user_id,
                        e
                    );

                    // Send admin alert (non-blocking)
                    let state_clone = state.clone();
                    let error_str = e.to_string();
                    tokio::spawn(async move {
                        let subject = "Bridge Check Failed - WhatsApp";
                        let message = format!(
                            "Failed to check WhatsApp bridge connection during digest generation.\n\n\
                            User ID: {}\n\
                            Error: {}\n\
                            Timestamp: {}",
                            user_id, error_str, chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        if let Err(e) = crate::utils::notification_utils::send_admin_alert(
                            &state_clone,
                            subject,
                            &message,
                        )
                        .await
                        {
                            tracing::error!("Failed to send admin alert: {}", e);
                        }
                    });
                }
            }

            // Fetch Telegram messages
            match state.user_repository.get_bridge(user_id, "telegram") {
                Ok(Some(_bridge)) => {
                    match crate::utils::bridge::fetch_bridge_messages(
                        "telegram",
                        state,
                        user_id,
                        start_timestamp,
                        true,
                    )
                    .await
                    {
                        Ok(telegram_messages) => {
                            // Convert TelegramMessage to MessageInfo and add to messages
                            let telegram_infos: Vec<MessageInfo> = telegram_messages
                                .into_iter()
                                .map(|msg| MessageInfo {
                                    sender: resolve_sender_name(
                                        &contact_profiles,
                                        "telegram",
                                        &msg.room_name,
                                        msg.room_id.as_deref(),
                                    ),
                                    content: msg.content,
                                    timestamp_rfc: msg.formatted_timestamp,
                                    platform: "telegram".to_string(),
                                    room_id: msg.room_id.clone(),
                                })
                                .collect();

                            tracing::debug!(
                                "Fetched {} Telegram messages from the last {} hours for digest",
                                telegram_infos.len(),
                                hours_since_prev
                            );

                            // Extend messages with Telegram messages
                            messages.extend(telegram_infos);

                            // Sort all messages by timestamp (most recent first)
                            messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch Telegram messages for digest: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    tracing::debug!("Telegram not connected for user {}", user_id);
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to check Telegram connection for user {}: {}",
                        user_id,
                        e
                    );

                    // Send admin alert (non-blocking)
                    let state_clone = state.clone();
                    let error_str = e.to_string();
                    tokio::spawn(async move {
                        let subject = "Bridge Check Failed - Telegram";
                        let message = format!(
                            "Failed to check Telegram bridge connection during digest generation.\n\n\
                            User ID: {}\n\
                            Error: {}\n\
                            Timestamp: {}",
                            user_id, error_str, chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        if let Err(e) = crate::utils::notification_utils::send_admin_alert(
                            &state_clone,
                            subject,
                            &message,
                        )
                        .await
                        {
                            tracing::error!("Failed to send admin alert: {}", e);
                        }
                    });
                }
            }

            // Fetch Signal messages
            match state.user_repository.get_bridge(user_id, "signal") {
                Ok(Some(_bridge)) => {
                    match crate::utils::bridge::fetch_bridge_messages(
                        "signal",
                        state,
                        user_id,
                        start_timestamp,
                        true,
                    )
                    .await
                    {
                        Ok(signal_messages) => {
                            // Convert Signal Message to MessageInfo and add to messages
                            let signal_infos: Vec<MessageInfo> = signal_messages
                                .into_iter()
                                .map(|msg| MessageInfo {
                                    sender: resolve_sender_name(
                                        &contact_profiles,
                                        "signal",
                                        &msg.room_name,
                                        msg.room_id.as_deref(),
                                    ),
                                    content: msg.content,
                                    timestamp_rfc: msg.formatted_timestamp,
                                    platform: "signal".to_string(),
                                    room_id: msg.room_id.clone(),
                                })
                                .collect();

                            tracing::debug!(
                                "Fetched {} Signal messages from the last {} hours for digest",
                                signal_infos.len(),
                                hours_since_prev
                            );

                            // Extend messages with Signal messages
                            messages.extend(signal_infos);

                            // Sort all messages by timestamp (most recent first)
                            messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch Signal messages for digest: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    tracing::debug!("Signal not connected for user {}", user_id);
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to check Signal connection for user {}: {}",
                        user_id,
                        e
                    );

                    // Send admin alert (non-blocking)
                    let state_clone = state.clone();
                    let error_str = e.to_string();
                    tokio::spawn(async move {
                        let subject = "Bridge Check Failed - Signal";
                        let message = format!(
                            "Failed to check Signal bridge connection during digest generation.\n\n\
                            User ID: {}\n\
                            Error: {}\n\
                            Timestamp: {}",
                            user_id, error_str, chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        if let Err(e) = crate::utils::notification_utils::send_admin_alert(
                            &state_clone,
                            subject,
                            &message,
                        )
                        .await
                        {
                            tracing::error!("Failed to send admin alert: {}", e);
                        }
                    });
                }
            }

            // Log total number of messages
            tracing::debug!("Total {} messages collected for digest", messages.len());

            // Build contact maps and filter out ignored contacts
            let DigestContactMaps { priority_map } =
                build_contact_maps_and_filter_messages(state, user_id, &mut messages);

            // return if no new nothing (after filtering ignored contacts)
            if messages.is_empty() && calendar_events.is_empty() {
                return Ok(());
            }

            messages.sort_by(|a, b| {
                let plat_cmp = a.platform.cmp(&b.platform);
                if plat_cmp == std::cmp::Ordering::Equal {
                    let sender_lower = a.sender.to_lowercase();
                    let a_pri = priority_map.get(&a.platform).is_some_and(|set| {
                        set.iter()
                            .any(|s| sender_lower.contains(s) || s.contains(&sender_lower))
                    });
                    let sender_lower_b = b.sender.to_lowercase();
                    let b_pri = priority_map.get(&b.platform).is_some_and(|set| {
                        set.iter()
                            .any(|s| sender_lower_b.contains(s) || s.contains(&sender_lower_b))
                    });
                    b_pri
                        .cmp(&a_pri)
                        .then_with(|| b.timestamp_rfc.cmp(&a.timestamp_rfc))
                } else {
                    plat_cmp
                }
            });

            // Get current datetime in user's timezone for AI context
            let now_local = chrono::Utc::now().with_timezone(&tz);
            let current_datetime_local = now_local.format("%Y-%m-%d %H:%M:%S").to_string();

            // Prepare digest data
            let digest_data = DigestData {
                messages,
                calendar_events,
                time_period_hours: hours_to_next,
                current_datetime_local,
            };

            // Generate the digest
            let mut digest_message = match generate_digest(state, user_id, digest_data, priority_map).await {
                Ok(digest) => format!("Good morning! {}",digest),
                Err(_) => format!(
                    "Good morning! Here's your morning digest covering the last {} hours. Next digest in {} hours.",
                    hours_since_prev, hours_to_next
                ),
            };

            // Append disconnection notices if any
            if let Some(notice) = format_disconnection_notice(state, user_id, &timezone) {
                digest_message = format!("{} {}", digest_message, notice);
            }

            // Append trackable item notices (invoices, shipments, deadlines)
            if let Some(tracking) = format_tracking_notice(state, user_id) {
                digest_message = format!("{} {}", digest_message, tracking);
            }

            tracing::info!(
                "Sending morning digest for user {} at {}:00 in timezone {}",
                user_id,
                digest_hour,
                timezone
            );

            send_notification(
                state,
                user_id,
                &digest_message,
                "morning_digest".to_string(),
                Some("Good morning! Want to hear your morning digest?".to_string()),
            )
            .await;
        }
    }

    Ok(())
}

pub async fn check_day_digest(
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get the user's digest settings and timezone
    let (morning_digest, day_digest, evening_digest) = state.user_core.get_digests(user_id)?;
    let user_info = state.user_core.get_user_info(user_id)?;

    // If day digest is enabled (Some value) and we have a timezone, check the time
    if let (Some(digest_hour_str), Some(timezone)) = (day_digest.clone(), user_info.timezone) {
        // Parse the timezone
        let tz: chrono_tz::Tz = timezone
            .parse()
            .map_err(|e| format!("Invalid timezone: {}", e))?;

        // Get current time in user's timezone
        let now = chrono::Utc::now().with_timezone(&tz);

        // Parse the digest hour (expected format: "HH:00" like "00:00", "23:00")
        let digest_hour: u32 = digest_hour_str
            .split(':')
            .next()
            .ok_or("Invalid time format")?
            .parse()
            .map_err(|e| format!("Invalid hour in digest time: {}", e))?;

        // Validate hour is between 0-23
        if digest_hour > 23 {
            tracing::error!("Invalid hour value (must be 0-23): {}", digest_hour);
            return Ok(());
        }

        // Compare current hour with digest hour
        if now.hour() == digest_hour {
            // Calculate hours until next digest
            let hours_to_next = match evening_digest.as_ref() {
                Some(evening) => {
                    let evening_hour: u32 = evening
                        .split(':')
                        .next()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0);
                    hours_until(digest_hour, evening_hour)
                }
                None => {
                    // If no other digests, calculate hours until evening
                    hours_until(digest_hour, 0)
                }
            };

            // Calculate hours since previous digest
            let hours_since_prev = match morning_digest.as_ref() {
                Some(morning) => {
                    let morning_hour: u32 = morning
                        .split(':')
                        .next()
                        .unwrap_or("6")
                        .parse()
                        .unwrap_or(6);
                    hours_since(digest_hour, morning_hour)
                }
                None => {
                    // If no morning digest, calculate hours since 6 o'clock
                    hours_since(digest_hour, 6)
                }
            };

            // Format start time (now) and end time (now + hours_to_next) in RFC3339
            let start_time = now.with_timezone(&Utc).to_rfc3339();
            let end_time = (now + Duration::hours(hours_to_next as i64))
                .with_timezone(&Utc)
                .to_rfc3339();

            // Fetch calendar events for the period
            let calendar_events = if state.user_repository.has_active_google_calendar(user_id)? {
                match crate::handlers::google_calendar::handle_calendar_fetching(
                    state.as_ref(),
                    user_id,
                    &start_time,
                    &end_time,
                )
                .await
                {
                    Ok(axum::Json(value)) => {
                        if let Some(events) = value.get("events").and_then(|e| e.as_array()) {
                            events
                                .iter()
                                .filter_map(|event| {
                                    let summary = event.get("summary")?.as_str()?.to_string();
                                    let start = event.get("start")?.as_str()?.parse().ok()?;
                                    let duration_minutes =
                                        event.get("duration_minutes")?.as_str()?.parse().ok()?;
                                    Some(CalendarEvent {
                                        title: summary,
                                        start_time_rfc: start,
                                        duration_minutes,
                                    })
                                })
                                .collect()
                        } else {
                            Vec::new()
                        }
                    }
                    Err(_) => Vec::new(),
                }
            } else {
                Vec::new()
            };

            // Calculate the time range for message fetching
            let now = Utc::now();
            let cutoff_time = now - Duration::hours(hours_since_prev as i64);
            let start_timestamp = cutoff_time.timestamp();

            // Fetch contact profiles for resolving sender nicknames
            let contact_profiles = state
                .user_repository
                .get_contact_profiles(user_id)
                .unwrap_or_default();

            // Check if user has IMAP credentials before fetching emails
            let mut messages = match state.user_repository.get_imap_credentials(user_id) {
                Ok(Some(_)) => {
                    // Fetch and filter emails
                    match crate::handlers::imap_handlers::fetch_emails_imap(
                        state,
                        user_id,
                        false,
                        Some(50),
                        false,
                        true,
                    )
                    .await
                    {
                        Ok(emails) => {
                            emails
                                .into_iter()
                                .filter(|email| {
                                    // Filter emails based on timestamp
                                    if let Some(date) = email.date {
                                        date >= cutoff_time
                                    } else {
                                        false // Exclude emails without a timestamp
                                    }
                                })
                                .map(|email| MessageInfo {
                                    sender: email
                                        .from
                                        .unwrap_or_else(|| "Unknown sender".to_string()),
                                    content: email
                                        .snippet
                                        .unwrap_or_else(|| "No content".to_string()),
                                    timestamp_rfc: email
                                        .date_formatted
                                        .unwrap_or_else(|| "No Timestamp".to_string()),
                                    platform: "email".to_string(),
                                    room_id: None,
                                })
                                .collect::<Vec<MessageInfo>>()
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch emails for digest: {:#?}", e);
                            Vec::new()
                        }
                    }
                }
                Ok(None) => {
                    tracing::debug!(
                        "Skipping email fetch - user {} has no IMAP credentials configured",
                        user_id
                    );
                    Vec::new()
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to check IMAP credentials for user {}: {}",
                        user_id,
                        e
                    );
                    Vec::new()
                }
            };

            // Log the number of filtered email messages
            tracing::debug!(
                "Filtered {} email messages from the last {} hours for digest",
                messages.len(),
                hours_since_prev
            );

            // Fetch WhatsApp messages
            match state.user_repository.get_bridge(user_id, "whatsapp") {
                Ok(Some(_bridge)) => {
                    match crate::utils::bridge::fetch_bridge_messages(
                        "whatsapp",
                        state,
                        user_id,
                        start_timestamp,
                        true,
                    )
                    .await
                    {
                        Ok(whatsapp_messages) => {
                            // Convert WhatsAppMessage to MessageInfo and add to messages
                            let whatsapp_infos: Vec<MessageInfo> = whatsapp_messages
                                .into_iter()
                                .map(|msg| MessageInfo {
                                    sender: resolve_sender_name(
                                        &contact_profiles,
                                        "whatsapp",
                                        &msg.room_name,
                                        msg.room_id.as_deref(),
                                    ),
                                    content: msg.content,
                                    timestamp_rfc: msg.formatted_timestamp,
                                    platform: "whatsapp".to_string(),
                                    room_id: msg.room_id.clone(),
                                })
                                .collect();

                            tracing::debug!(
                                "Fetched {} WhatsApp messages from the last {} hours for digest",
                                whatsapp_infos.len(),
                                hours_since_prev
                            );

                            // Extend messages with WhatsApp messages
                            messages.extend(whatsapp_infos);

                            // Sort all messages by timestamp (most recent first)
                            messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch WhatsApp messages for digest: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    tracing::debug!("WhatsApp not connected for user {}", user_id);
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to check WhatsApp connection for user {}: {}",
                        user_id,
                        e
                    );

                    // Send admin alert (non-blocking)
                    let state_clone = state.clone();
                    let error_str = e.to_string();
                    tokio::spawn(async move {
                        let subject = "Bridge Check Failed - WhatsApp";
                        let message = format!(
                            "Failed to check WhatsApp bridge connection during digest generation.\n\n\
                            User ID: {}\n\
                            Error: {}\n\
                            Timestamp: {}",
                            user_id, error_str, chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        if let Err(e) = crate::utils::notification_utils::send_admin_alert(
                            &state_clone,
                            subject,
                            &message,
                        )
                        .await
                        {
                            tracing::error!("Failed to send admin alert: {}", e);
                        }
                    });
                }
            }

            // Fetch Telegram messages
            if state
                .user_repository
                .get_bridge(user_id, "telegram")?
                .is_some()
            {
                match crate::utils::bridge::fetch_bridge_messages(
                    "telegram",
                    state,
                    user_id,
                    start_timestamp,
                    true,
                )
                .await
                {
                    Ok(telegram_messages) => {
                        // Convert TelegramMessage to MessageInfo and add to messages
                        let telegram_infos: Vec<MessageInfo> = telegram_messages
                            .into_iter()
                            .map(|msg| MessageInfo {
                                sender: resolve_sender_name(
                                    &contact_profiles,
                                    "telegram",
                                    &msg.room_name,
                                    msg.room_id.as_deref(),
                                ),
                                content: msg.content,
                                timestamp_rfc: msg.formatted_timestamp,
                                platform: "telegram".to_string(),
                                room_id: msg.room_id.clone(),
                            })
                            .collect();

                        tracing::debug!(
                            "Fetched {} Telegram messages from the last {} hours for digest",
                            telegram_infos.len(),
                            hours_since_prev
                        );
                        // Extend messages with Telegram messages
                        messages.extend(telegram_infos);

                        // Sort all messages by timestamp (most recent first)
                        messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch Telegram messages for digest: {}", e);
                    }
                }
            }

            // Fetch Signal messages
            match state.user_repository.get_bridge(user_id, "signal") {
                Ok(Some(_bridge)) => {
                    match crate::utils::bridge::fetch_bridge_messages(
                        "signal",
                        state,
                        user_id,
                        start_timestamp,
                        true,
                    )
                    .await
                    {
                        Ok(signal_messages) => {
                            // Convert Signal Message to MessageInfo and add to messages
                            let signal_infos: Vec<MessageInfo> = signal_messages
                                .into_iter()
                                .map(|msg| MessageInfo {
                                    sender: resolve_sender_name(
                                        &contact_profiles,
                                        "signal",
                                        &msg.room_name,
                                        msg.room_id.as_deref(),
                                    ),
                                    content: msg.content,
                                    timestamp_rfc: msg.formatted_timestamp,
                                    platform: "signal".to_string(),
                                    room_id: msg.room_id.clone(),
                                })
                                .collect();

                            tracing::debug!(
                                "Fetched {} Signal messages from the last {} hours for digest",
                                signal_infos.len(),
                                hours_since_prev
                            );

                            // Extend messages with Signal messages
                            messages.extend(signal_infos);

                            // Sort all messages by timestamp (most recent first)
                            messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch Signal messages for digest: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    tracing::debug!("Signal not connected for user {}", user_id);
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to check Signal connection for user {}: {}",
                        user_id,
                        e
                    );

                    // Send admin alert (non-blocking)
                    let state_clone = state.clone();
                    let error_str = e.to_string();
                    tokio::spawn(async move {
                        let subject = "Bridge Check Failed - Signal";
                        let message = format!(
                            "Failed to check Signal bridge connection during digest generation.\n\n\
                            User ID: {}\n\
                            Error: {}\n\
                            Timestamp: {}",
                            user_id, error_str, chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        if let Err(e) = crate::utils::notification_utils::send_admin_alert(
                            &state_clone,
                            subject,
                            &message,
                        )
                        .await
                        {
                            tracing::error!("Failed to send admin alert: {}", e);
                        }
                    });
                }
            }

            // Log total number of messages
            tracing::debug!("Total {} messages collected for digest", messages.len());

            // Build contact maps and filter out ignored contacts
            let DigestContactMaps { priority_map } =
                build_contact_maps_and_filter_messages(state, user_id, &mut messages);

            // return if no new nothing (after filtering ignored contacts)
            if messages.is_empty() && calendar_events.is_empty() {
                return Ok(());
            }

            messages.sort_by(|a, b| {
                let plat_cmp = a.platform.cmp(&b.platform);
                if plat_cmp == std::cmp::Ordering::Equal {
                    let sender_lower = a.sender.to_lowercase();
                    let a_pri = priority_map.get(&a.platform).is_some_and(|set| {
                        set.iter()
                            .any(|s| sender_lower.contains(s) || s.contains(&sender_lower))
                    });
                    let sender_lower_b = b.sender.to_lowercase();
                    let b_pri = priority_map.get(&b.platform).is_some_and(|set| {
                        set.iter()
                            .any(|s| sender_lower_b.contains(s) || s.contains(&sender_lower_b))
                    });
                    b_pri
                        .cmp(&a_pri)
                        .then_with(|| b.timestamp_rfc.cmp(&a.timestamp_rfc))
                } else {
                    plat_cmp
                }
            });

            // Get current datetime in user's timezone for AI context
            let now_local = chrono::Utc::now().with_timezone(&tz);
            let current_datetime_local = now_local.format("%Y-%m-%d %H:%M:%S").to_string();

            // Prepare digest data
            let digest_data = DigestData {
                messages,
                calendar_events,
                time_period_hours: hours_to_next,
                current_datetime_local,
            };

            // Generate the digest
            let mut digest_message = match generate_digest(state, user_id, digest_data, priority_map).await {
                Ok(digest) => format!("Hello! {}",digest),
                Err(_) => format!(
                    "Hello! Here's your daily digest covering the last {} hours. Next digest in {} hours.",
                    hours_since_prev, hours_to_next
                ),
            };

            // Append disconnection notices if any
            if let Some(notice) = format_disconnection_notice(state, user_id, &timezone) {
                digest_message = format!("{} {}", digest_message, notice);
            }

            // Append trackable item notices (invoices, shipments, deadlines)
            if let Some(tracking) = format_tracking_notice(state, user_id) {
                digest_message = format!("{} {}", digest_message, tracking);
            }

            tracing::info!(
                "Sending day digest for user {} at {}:00 in timezone {}",
                user_id,
                digest_hour,
                timezone
            );

            send_notification(
                state,
                user_id,
                &digest_message,
                "day_digest".to_string(),
                Some("Hello! Want to hear your daily digest?".to_string()),
            )
            .await;
        }
    }

    Ok(())
}

pub async fn check_evening_digest(
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get the user's digest settings and timezone
    let (morning_digest, day_digest, evening_digest) = state.user_core.get_digests(user_id)?;
    let user_info = state.user_core.get_user_info(user_id)?;

    // If morning digest is enabled (Some value) and we have a timezone, check the time
    if let (Some(digest_hour_str), Some(timezone)) = (evening_digest.clone(), user_info.timezone) {
        // Parse the timezone
        let tz: chrono_tz::Tz = timezone
            .parse()
            .map_err(|e| format!("Invalid timezone: {}", e))?;

        // Get current time in user's timezone
        let now = chrono::Utc::now().with_timezone(&tz);

        // Parse the digest hour (expected format: "HH:00" like "00:00", "23:00")
        let digest_hour: u32 = digest_hour_str
            .split(':')
            .next()
            .ok_or("Invalid time format")?
            .parse()
            .map_err(|e| format!("Invalid hour in digest time: {}", e))?;

        // Validate hour is between 0-23
        if digest_hour > 23 {
            tracing::error!("Invalid hour value (must be 0-23): {}", digest_hour);
            return Ok(());
        }

        // Compare current hour with digest hour
        if now.hour() == digest_hour {
            // Calculate hours until next digest
            let hours_to_next = match morning_digest.as_ref() {
                Some(morning) => {
                    let morning_hour: u32 = morning
                        .split(':')
                        .next()
                        .unwrap_or("8")
                        .parse()
                        .unwrap_or(8);
                    hours_until(digest_hour, morning_hour)
                }
                None => {
                    // If no other digests, calculate hours until morning
                    hours_until(digest_hour, 8)
                }
            };

            // Calculate hours since previous digest
            let hours_since_prev = match day_digest.as_ref() {
                Some(day) => {
                    let day_hour: u32 = day.split(':').next().unwrap_or("12").parse().unwrap_or(12);
                    hours_since(digest_hour, day_hour)
                }
                None => {
                    // If no morning digest, calculate hours since 6 o'clock
                    hours_since(digest_hour, 12)
                }
            };

            let tz: chrono_tz::Tz = timezone
                .parse()
                .map_err(|e| format!("Invalid timezone: {}", e))?;

            // Format start time (now) and end time (now + hours_to_next) in RFC3339
            let start_time = now.with_timezone(&tz).to_rfc3339();

            // Calculate end of tomorrow
            let tomorrow_end = now
                .date_naive()
                .succ_opt() // Get tomorrow's date
                .unwrap_or(now.date_naive()) // Fallback to today if overflow
                .and_hms_opt(23, 59, 59) // Set to end of day
                .unwrap_or(now.naive_local()) // Fallback to now if invalid time
                .and_local_timezone(tz)
                .earliest() // Get the earliest possible time if ambiguous
                .unwrap_or(now); // Fallback to now if conversion fails

            let end_time = tomorrow_end.with_timezone(&Utc).to_rfc3339();

            // Check if user has active Google Calendar before fetching events
            let calendar_events = match state.user_repository.has_active_google_calendar(user_id) {
                Ok(true) => {
                    match crate::handlers::google_calendar::handle_calendar_fetching(
                        state.as_ref(),
                        user_id,
                        &start_time,
                        &end_time,
                    )
                    .await
                    {
                        Ok(axum::Json(value)) => {
                            if let Some(events) = value.get("events").and_then(|e| e.as_array()) {
                                events
                                    .iter()
                                    .filter_map(|event| {
                                        let summary = event.get("summary")?.as_str()?.to_string();
                                        let start = event.get("start")?.as_str()?.parse().ok()?;
                                        let duration_minutes = event
                                            .get("duration_minutes")?
                                            .as_str()?
                                            .parse()
                                            .ok()?;
                                        Some(CalendarEvent {
                                            title: summary,
                                            start_time_rfc: start,
                                            duration_minutes,
                                        })
                                    })
                                    .collect()
                            } else {
                                Vec::new()
                            }
                        }
                        Err(_) => Vec::new(),
                    }
                }
                Ok(false) => {
                    tracing::debug!("User {} has no active Google Calendar", user_id);
                    Vec::new()
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to check Google Calendar status for user {}: {}",
                        user_id,
                        e
                    );
                    Vec::new()
                }
            };

            // Calculate the time range for message fetching
            let now = Utc::now();
            let cutoff_time = now - Duration::hours(hours_since_prev as i64);
            let start_timestamp = cutoff_time.timestamp();

            // Fetch contact profiles for resolving sender nicknames
            let contact_profiles = state
                .user_repository
                .get_contact_profiles(user_id)
                .unwrap_or_default();

            // Check if user has IMAP credentials before fetching emails
            let mut messages = match state.user_repository.get_imap_credentials(user_id) {
                Ok(Some(_)) => {
                    // Fetch and filter emails
                    match crate::handlers::imap_handlers::fetch_emails_imap(
                        state,
                        user_id,
                        false,
                        Some(50),
                        false,
                        true,
                    )
                    .await
                    {
                        Ok(emails) => {
                            emails
                                .into_iter()
                                .filter(|email| {
                                    // Filter emails based on timestamp
                                    if let Some(date) = email.date {
                                        date >= cutoff_time
                                    } else {
                                        false // Exclude emails without a timestamp
                                    }
                                })
                                .map(|email| MessageInfo {
                                    sender: email
                                        .from
                                        .unwrap_or_else(|| "Unknown sender".to_string()),
                                    content: email
                                        .snippet
                                        .unwrap_or_else(|| "No content".to_string()),
                                    timestamp_rfc: email
                                        .date_formatted
                                        .unwrap_or_else(|| "No Timestamp".to_string()),
                                    platform: "email".to_string(),
                                    room_id: None,
                                })
                                .collect::<Vec<MessageInfo>>()
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch emails for digest: {:#?}", e);
                            Vec::new()
                        }
                    }
                }
                Ok(None) => {
                    tracing::debug!(
                        "Skipping email fetch - user {} has no IMAP credentials configured",
                        user_id
                    );
                    Vec::new()
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to check IMAP credentials for user {}: {}",
                        user_id,
                        e
                    );
                    Vec::new()
                }
            };

            // Log the number of filtered email messages
            tracing::debug!(
                "Filtered {} email messages from the last {} hours for digest",
                messages.len(),
                hours_since_prev
            );

            // Fetch WhatsApp messages
            match state.user_repository.get_bridge(user_id, "whatsapp") {
                Ok(Some(_bridge)) => {
                    match crate::utils::bridge::fetch_bridge_messages(
                        "whatsapp",
                        state,
                        user_id,
                        start_timestamp,
                        true,
                    )
                    .await
                    {
                        Ok(whatsapp_messages) => {
                            // Convert WhatsAppMessage to MessageInfo and add to messages
                            let whatsapp_infos: Vec<MessageInfo> = whatsapp_messages
                                .into_iter()
                                .map(|msg| MessageInfo {
                                    sender: resolve_sender_name(
                                        &contact_profiles,
                                        "whatsapp",
                                        &msg.room_name,
                                        msg.room_id.as_deref(),
                                    ),
                                    content: msg.content,
                                    timestamp_rfc: msg.formatted_timestamp,
                                    platform: "whatsapp".to_string(),
                                    room_id: msg.room_id.clone(),
                                })
                                .collect();

                            tracing::debug!(
                                "Fetched {} WhatsApp messages from the last {} hours for digest",
                                whatsapp_infos.len(),
                                hours_since_prev
                            );

                            // Extend messages with WhatsApp messages
                            messages.extend(whatsapp_infos);

                            // Sort all messages by timestamp (most recent first)
                            messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch WhatsApp messages for digest: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    tracing::debug!("WhatsApp not connected for user {}", user_id);
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to check WhatsApp connection for user {}: {}",
                        user_id,
                        e
                    );

                    // Send admin alert (non-blocking)
                    let state_clone = state.clone();
                    let error_str = e.to_string();
                    tokio::spawn(async move {
                        let subject = "Bridge Check Failed - WhatsApp";
                        let message = format!(
                            "Failed to check WhatsApp bridge connection during digest generation.\n\n\
                            User ID: {}\n\
                            Error: {}\n\
                            Timestamp: {}",
                            user_id, error_str, chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        if let Err(e) = crate::utils::notification_utils::send_admin_alert(
                            &state_clone,
                            subject,
                            &message,
                        )
                        .await
                        {
                            tracing::error!("Failed to send admin alert: {}", e);
                        }
                    });
                }
            }

            // Fetch Telegram messages
            if state
                .user_repository
                .get_bridge(user_id, "telegram")?
                .is_some()
            {
                match crate::utils::bridge::fetch_bridge_messages(
                    "telegram",
                    state,
                    user_id,
                    start_timestamp,
                    true,
                )
                .await
                {
                    Ok(telegram_messages) => {
                        // Convert Telegram to MessageInfo and add to messages
                        let telegram_infos: Vec<MessageInfo> = telegram_messages
                            .into_iter()
                            .map(|msg| MessageInfo {
                                sender: resolve_sender_name(
                                    &contact_profiles,
                                    "telegram",
                                    &msg.room_name,
                                    msg.room_id.as_deref(),
                                ),
                                content: msg.content,
                                timestamp_rfc: msg.formatted_timestamp,
                                platform: "telegram".to_string(),
                                room_id: msg.room_id.clone(),
                            })
                            .collect();

                        tracing::debug!(
                            "Fetched {} Telegram messages from the last {} hours for digest",
                            telegram_infos.len(),
                            hours_since_prev
                        );

                        // Extend messages with Telegram messages
                        messages.extend(telegram_infos);

                        // Sort all messages by timestamp (most recent first)
                        messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch Telegram messages for digest: {}", e);
                    }
                }
            }

            // Fetch Signal messages
            match state.user_repository.get_bridge(user_id, "signal") {
                Ok(Some(_bridge)) => {
                    match crate::utils::bridge::fetch_bridge_messages(
                        "signal",
                        state,
                        user_id,
                        start_timestamp,
                        true,
                    )
                    .await
                    {
                        Ok(signal_messages) => {
                            // Convert Signal Message to MessageInfo and add to messages
                            let signal_infos: Vec<MessageInfo> = signal_messages
                                .into_iter()
                                .map(|msg| MessageInfo {
                                    sender: resolve_sender_name(
                                        &contact_profiles,
                                        "signal",
                                        &msg.room_name,
                                        msg.room_id.as_deref(),
                                    ),
                                    content: msg.content,
                                    timestamp_rfc: msg.formatted_timestamp,
                                    platform: "signal".to_string(),
                                    room_id: msg.room_id.clone(),
                                })
                                .collect();

                            tracing::debug!(
                                "Fetched {} Signal messages from the last {} hours for digest",
                                signal_infos.len(),
                                hours_since_prev
                            );

                            // Extend messages with Signal messages
                            messages.extend(signal_infos);

                            // Sort all messages by timestamp (most recent first)
                            messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                        }
                        Err(e) => {
                            tracing::error!("Failed to fetch Signal messages for digest: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    tracing::debug!("Signal not connected for user {}", user_id);
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to check Signal connection for user {}: {}",
                        user_id,
                        e
                    );

                    // Send admin alert (non-blocking)
                    let state_clone = state.clone();
                    let error_str = e.to_string();
                    tokio::spawn(async move {
                        let subject = "Bridge Check Failed - Signal";
                        let message = format!(
                            "Failed to check Signal bridge connection during digest generation.\n\n\
                            User ID: {}\n\
                            Error: {}\n\
                            Timestamp: {}",
                            user_id, error_str, chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        if let Err(e) = crate::utils::notification_utils::send_admin_alert(
                            &state_clone,
                            subject,
                            &message,
                        )
                        .await
                        {
                            tracing::error!("Failed to send admin alert: {}", e);
                        }
                    });
                }
            }

            // Log total number of messages
            tracing::debug!("Total {} messages collected for digest", messages.len());

            // Build contact maps and filter out ignored contacts
            let DigestContactMaps { priority_map } =
                build_contact_maps_and_filter_messages(state, user_id, &mut messages);

            // return if no new nothing (after filtering ignored contacts)
            if messages.is_empty() && calendar_events.is_empty() {
                return Ok(());
            }

            messages.sort_by(|a, b| {
                let plat_cmp = a.platform.cmp(&b.platform);
                if plat_cmp == std::cmp::Ordering::Equal {
                    let sender_lower = a.sender.to_lowercase();
                    let a_pri = priority_map.get(&a.platform).is_some_and(|set| {
                        set.iter()
                            .any(|s| sender_lower.contains(s) || s.contains(&sender_lower))
                    });
                    let sender_lower_b = b.sender.to_lowercase();
                    let b_pri = priority_map.get(&b.platform).is_some_and(|set| {
                        set.iter()
                            .any(|s| sender_lower_b.contains(s) || s.contains(&sender_lower_b))
                    });
                    b_pri
                        .cmp(&a_pri)
                        .then_with(|| b.timestamp_rfc.cmp(&a.timestamp_rfc))
                } else {
                    plat_cmp
                }
            });

            // Get current datetime in user's timezone for AI context
            let now_local = chrono::Utc::now().with_timezone(&tz);
            let current_datetime_local = now_local.format("%Y-%m-%d %H:%M:%S").to_string();

            // Prepare digest data
            let digest_data = DigestData {
                messages,
                calendar_events,
                time_period_hours: hours_to_next,
                current_datetime_local,
            };

            // Generate the digest
            let mut digest_message = match generate_digest(state, user_id, digest_data, priority_map).await {
                Ok(digest) => format!("Good evening! {}",digest),
                Err(_) => format!(
                    "Hello! Here's your evening digest covering the last {} hours. Next digest in {} hours.",
                    hours_since_prev, hours_to_next
                ),
            };

            // Append disconnection notices if any
            if let Some(notice) = format_disconnection_notice(state, user_id, &timezone) {
                digest_message = format!("{} {}", digest_message, notice);
            }

            // Append trackable item notices (invoices, shipments, deadlines)
            if let Some(tracking) = format_tracking_notice(state, user_id) {
                digest_message = format!("{} {}", digest_message, tracking);
            }

            tracing::info!(
                "Sending evening digest for user {} at {}:00 in timezone {}",
                user_id,
                digest_hour,
                timezone
            );

            send_notification(
                state,
                user_id,
                &digest_message,
                "evening_digest".to_string(),
                Some("Good evening! Want to hear your evening digest?".to_string()),
            )
            .await;
        }
    }

    Ok(())
}

const DIGEST_PROMPT: &str = r#"You are an AI called lightfriend that creates concise SMS digests of messages and calendar events. Your goal is to help users stay on top of unread messages and upcoming calendar events without needing to open their apps. Group items by platform (e.g., WHATSAPP:, EMAIL:, CALENDAR:), starting each group on a new line. Within each group, provide clear teasers for critical or prioritized items (e.g., sender, topic hint, timestamp in parentheses), separating them with commas or '+' for brevity. Summarize less urgent or grouped items at the end of the group with '+' (e.g., '+ other routine items from xai, claude, ..'). Adjust detail based on overall content: if low volume or mostly low-criticality, expand critical items with fuller, detailed teasers (e.g., key excerpts or actions) to avoid follow-ups. For high volume or non-critical items, use minimal teasers. Highlight critical/actionable items with more specific hints to reduce follow-ups, but avoid full content. Cover all items concisely without omissions.
Rules
• Absolute length limit: 480 characters.
• Do NOT use markdown (no *, **, _, links, or backticks).
• Do NOT use emojis or emoticons.
• Plain text only.
• Start each platform group on a new line, followed by ': ' and the teasers/summaries.
• Messages marked with [PRIORITY] are from user-defined priority senders. Always put them first in their platform group, highlight them with more detailed teasers (e.g., key excerpts, actions, or urgency hints), and treat them as critical/actionable to minimize user follow-ups.
• Put critical or prioritized items first within each group.
• Include timestamps in parentheses using relative terms based on the current datetime provided. Use '(today Xpm/am)' for same-day messages and '(yesterday Xpm/am)' only for messages from the previous calendar day. Compare the message date with the current date to determine this correctly.
• For calendar, include events in the next 24 hours with start time and brief hint.
• Tease naturally, e.g., 'Mom suggested dinner in family chat (today 8pm)'.
Return JSON with a single field:
• `digest` – the plain-text SMS message, with newlines separating groups.
"#;
pub async fn generate_digest(
    state: &Arc<AppState>,
    user_id: i32,
    data: DigestData,
    priority_map: HashMap<String, HashSet<String>>,
) -> Result<String, Box<dyn std::error::Error>> {
    let ctx = ContextBuilder::for_user(state, user_id).build().await?;
    // Format messages for the prompt
    let messages_str = data
        .messages
        .iter()
        .map(|msg| {
            let priority_tag = if priority_map
                .get(&msg.platform)
                .is_some_and(|set| set.contains(&msg.sender))
            {
                " [PRIORITY]".to_string()
            } else {
                String::new()
            };
            format!(
                "- [{}] {} on {}: {}{}",
                msg.platform.to_uppercase(),
                msg.sender,
                msg.timestamp_rfc,
                msg.content,
                priority_tag,
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    // Format calendar events for the prompt
    let events_str = data
        .calendar_events
        .iter()
        .map(|event| {
            format!(
                "- {} at {} lasting {} minutes",
                event.title, event.start_time_rfc, event.duration_minutes,
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    // Conditionally include calendar section only if there are events
    let user_content = if data.calendar_events.is_empty() {
        format!(
            "Current datetime (user's local time): {}\n\nCreate a digest covering the last {} hours.\n\nMessages:\n{}",
            data.current_datetime_local, data.time_period_hours, messages_str
        )
    } else {
        format!(
            "Current datetime (user's local time): {}\n\nCreate a digest covering the last {} hours.\n\nMessages:\n{}\n\nUpcoming calendar events:\n{}",
            data.current_datetime_local, data.time_period_hours, messages_str, events_str
        )
    };
    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(DIGEST_PROMPT.to_string()),
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
    let request = chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages)
        .tools(tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .max_tokens(350);
    match ctx.client.chat_completion(request).await {
        Ok(result) => {
            if let Some(tool_calls) = result.choices[0].message.tool_calls.as_ref() {
                if let Some(first_call) = tool_calls.first() {
                    if let Some(args) = &first_call.function.arguments {
                        #[derive(Debug, Deserialize)]
                        struct DigestResponse {
                            digest: String,
                        }
                        match serde_json::from_str::<DigestResponse>(args) {
                            Ok(response) => {
                                tracing::debug!("Generated digest: {}", response.digest);
                                Ok(response.digest)
                            }
                            Err(e) => {
                                tracing::error!("Failed to parse digest response: {}", e);
                                Ok("Failed to generate digest(parse error).".to_string())
                            }
                        }
                    } else {
                        tracing::error!("No arguments found in tool call");
                        Ok("Failed to generate digest(arguments missing).".to_string())
                    }
                } else {
                    tracing::error!("No tool calls found");
                    Ok("Failed to generate digest(no first tool call).".to_string())
                }
            } else {
                tracing::error!("No tool calls section in response");
                Ok("Failed to generate digest(no tool calls).".to_string())
            }
        }
        Err(e) => {
            tracing::error!("Failed to generate digest: {}", e);
            Err(e.into())
        }
    }
}

pub async fn send_notification(
    state: &Arc<AppState>,
    user_id: i32,
    notification: &str,
    content_type: String,
    first_message: Option<String>,
) {
    // Get current timestamp for message history
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    // Get user info
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::error!("User {} not found for notification", user_id);
            return;
        }
        Err(e) => {
            tracing::error!("Failed to get user {}: {}", user_id, e);
            return;
        }
    };

    // Get user settings (assuming state has a user_settings repository or similar)
    let user_settings = match state.user_core.get_user_settings(user_id) {
        Ok(settings) => settings,
        Err(e) => {
            tracing::error!("Failed to get settings for user {}: {}", user_id, e);
            return;
        }
    };

    // Check quiet mode - skip notification if user is in quiet mode
    match state.user_core.get_quiet_mode(user_id) {
        Ok(Some(0)) => {
            tracing::debug!(
                "Skipping notification for user {} - indefinite quiet mode",
                user_id
            );
            return;
        }
        Ok(Some(ts)) if ts > current_time => {
            tracing::debug!(
                "Skipping notification for user {} - quiet mode until {}",
                user_id,
                ts
            );
            return;
        }
        _ => {
            // No quiet mode or expired - proceed with notification
        }
    }

    let user_info = match state.user_core.get_user_info(user_id) {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("Failed to get info for user {}: {}", user_id, e);
            return;
        }
    };

    // Check user's notification preference from settings
    // Note: Order matters - check "_call_sms" before "_call" to avoid false match
    // Digests are always SMS-only (not affected by user's default notification type)
    let notification_type = if content_type.contains("digest") {
        "sms"
    } else if content_type.contains("_call_sms") {
        "call_sms"
    } else if content_type.contains("critical") {
        user_settings.critical_enabled.as_deref().unwrap_or("sms")
    } else if content_type.contains("_call") {
        "call"
    } else if content_type.contains("_sms") {
        "sms"
    } else {
        user_settings.notification_type.as_deref().unwrap_or("sms")
    };

    match notification_type {
        "call" => {
            if let Err(e) =
                crate::utils::usage::check_user_credits(state, &user, "noti_call", None).await
            {
                tracing::warn!("User {} has insufficient credits: {}", user.id, e);
                return;
            }

            // Create dynamic variables (optional, can be customized based on needs)
            let dynamic_vars = std::collections::HashMap::new();

            match crate::api::elevenlabs::make_notification_call(
                &state.clone(),
                content_type.clone(), // Notification type
                first_message.clone().unwrap_or(
                    "Hello, I have a critical notification to tell you about".to_string(),
                ),
                notification.to_string(),
                user.id.to_string(),
                user_info.timezone,
            )
            .await
            {
                Ok(mut response) => {
                    // Add dynamic variables to the client data
                    if let Some(client_data) = response.get_mut("client_data") {
                        if let Some(obj) = client_data.as_object_mut() {
                            obj.extend(
                                dynamic_vars
                                    .into_iter()
                                    .map(|(k, v)| (k, serde_json::Value::String(v))),
                            );
                        }
                    }
                    tracing::debug!(
                        "Successfully initiated call notification for user {}",
                        user.id
                    );

                    // Store notification in message history
                    let assistant_notification = crate::models::user_models::NewMessageHistory {
                        user_id: user.id,
                        role: "assistant".to_string(),
                        encrypted_content: notification.to_string(),
                        tool_name: None,
                        tool_call_id: None,
                        tool_calls_json: None,
                        created_at: current_time,
                        conversation_id: "".to_string(),
                    };

                    if let Err(e) = state
                        .user_repository
                        .create_message_history(&assistant_notification)
                    {
                        tracing::error!("Failed to store notification in history: {}", e);
                    }

                    // Log successful call notification
                    if let Err(e) = state.user_repository.log_usage(LogUsageParams {
                        user_id,
                        sid: response
                            .get("sid")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        activity_type: content_type,
                        credits: None,
                        time_consumed: None,
                        success: Some(true),
                        reason: None,
                        status: Some("completed".to_string()),
                        recharge_threshold_timestamp: None,
                        zero_credits_timestamp: None,
                    }) {
                        tracing::error!("Failed to log call notification usage: {}", e);
                    }

                    // Deduct credits after successful notification
                    if let Err(e) =
                        crate::utils::usage::deduct_user_credits(state, user_id, "noti_call", None)
                    {
                        tracing::error!(
                            "Failed to deduct credits for user {} after call notification: {}",
                            user_id,
                            e
                        );
                    }
                }
                Err((_, json_err)) => {
                    tracing::error!("Failed to initiate call notification: {:?}", json_err);

                    // Log failed call notification
                    if let Err(e) = state.user_repository.log_usage(LogUsageParams {
                        user_id,
                        sid: None,
                        activity_type: content_type,
                        credits: None,
                        time_consumed: None,
                        success: Some(false),
                        reason: Some(format!("Failed to initiate call: {:?}", json_err)),
                        status: Some("failed".to_string()),
                        recharge_threshold_timestamp: None,
                        zero_credits_timestamp: None,
                    }) {
                        tracing::error!("Failed to log failed call notification: {}", e);
                    }
                }
            }
        }
        "call_sms" => {
            // Call + SMS: Send SMS first (always charged), then initiate call (conditionally charged)
            // The call acts as a loud alert; SMS contains the actual message content.
            // If the user doesn't answer the call (call_initiation_failure webhook),
            // we don't charge for the call - only for the SMS.

            // Step 1: Check credits for SMS (always required)
            if let Err(e) =
                crate::utils::usage::check_user_credits(state, &user, "noti_msg", None).await
            {
                tracing::warn!(
                    "User {} has insufficient credits for Call+SMS notification: {}",
                    user.id,
                    e
                );
                return;
            }

            // Step 2: Send SMS first (this is always charged)
            let sms_success = match state
                .twilio_message_service
                .send_sms(notification, None, &user)
                .await
            {
                Ok(response_sid) => {
                    tracing::info!("Call+SMS: SMS sent successfully for user {}", user_id);

                    // Store notification in message history
                    let assistant_notification = crate::models::user_models::NewMessageHistory {
                        user_id: user.id,
                        role: "assistant".to_string(),
                        encrypted_content: notification.to_string(),
                        tool_name: None,
                        tool_call_id: None,
                        tool_calls_json: None,
                        created_at: current_time,
                        conversation_id: "".to_string(),
                    };

                    if let Err(e) = state
                        .user_repository
                        .create_message_history(&assistant_notification)
                    {
                        tracing::error!("Failed to store Call+SMS notification in history: {}", e);
                    }

                    // Log SMS usage
                    if let Err(e) = state.user_repository.log_usage(LogUsageParams {
                        user_id,
                        sid: Some(response_sid),
                        activity_type: format!("{}_sms", content_type),
                        credits: None,
                        time_consumed: None,
                        success: Some(true),
                        reason: None,
                        status: Some("delivered".to_string()),
                        recharge_threshold_timestamp: None,
                        zero_credits_timestamp: None,
                    }) {
                        tracing::error!("Failed to log Call+SMS SMS usage: {}", e);
                    }

                    // SMS credits deducted at Twilio status callback

                    true
                }
                Err(e) => {
                    tracing::error!("Call+SMS: Failed to send SMS for user {}: {}", user_id, e);
                    false
                }
            };

            // Step 3: Initiate call as alert (only if we have credits, charged conditionally via webhook)
            // The call will only be charged if the user answers (post_call_transcription webhook)
            // If declined/no-answer (call_initiation_failure webhook), no charge for call
            if sms_success {
                // Check if user has credits for call (but don't charge yet - webhook handles it)
                if crate::utils::usage::check_user_credits(state, &user, "noti_call", None)
                    .await
                    .is_ok()
                {
                    match crate::api::elevenlabs::make_notification_call(
                        &state.clone(),
                        format!("{}_call_conditional", content_type), // Mark as conditional for webhook
                        first_message.clone().unwrap_or(
                            "Hello, you have a notification. Check your SMS for details."
                                .to_string(),
                        ),
                        notification.to_string(),
                        user.id.to_string(),
                        user_info.timezone.clone(),
                    )
                    .await
                    {
                        Ok(response) => {
                            tracing::info!("Call+SMS: Call initiated for user {} (will be charged if answered)", user_id);

                            // Log call usage as "ongoing" - it will be updated by webhook
                            // Don't deduct credits here - the webhook will handle it based on answer status
                            if let Err(e) = state.user_repository.log_usage(LogUsageParams {
                                user_id,
                                sid: response
                                    .get("sid")
                                    .and_then(|v| v.as_str())
                                    .map(String::from),
                                activity_type: format!("{}_call_conditional", content_type),
                                credits: None,
                                time_consumed: None,
                                success: None, // Don't set success yet
                                reason: None,
                                status: Some("ongoing".to_string()),
                                recharge_threshold_timestamp: None,
                                zero_credits_timestamp: None,
                            }) {
                                tracing::error!("Failed to log Call+SMS call usage: {}", e);
                            }
                        }
                        Err((_, json_err)) => {
                            tracing::error!(
                                "Call+SMS: Failed to initiate call for user {}: {:?}",
                                user_id,
                                json_err
                            );
                            // Call failed but SMS was already sent successfully, so notification is partially delivered
                        }
                    }
                } else {
                    tracing::info!("Call+SMS: Skipping call for user {} (insufficient credits), SMS already sent", user_id);
                }
            }
        }
        _ => {
            // Default to SMS notification
            if let Err(e) =
                crate::utils::usage::check_user_credits(state, &user, "noti_msg", None).await
            {
                tracing::warn!("User {} has insufficient credits: {}", user.id, e);
                return;
            }
            match state
                .twilio_message_service
                .send_sms(notification, None, &user)
                .await
            {
                Ok(response_sid) => {
                    tracing::info!("Successfully sent notification to user {}", user_id);

                    // Store notification in message history
                    let assistant_notification = crate::models::user_models::NewMessageHistory {
                        user_id: user.id,
                        role: "assistant".to_string(),
                        encrypted_content: notification.to_string(),
                        tool_name: None,
                        tool_call_id: None,
                        tool_calls_json: None,
                        created_at: current_time,
                        conversation_id: "".to_string(),
                    };

                    // Store message in history
                    if let Err(e) = state
                        .user_repository
                        .create_message_history(&assistant_notification)
                    {
                        tracing::error!("Failed to store notification in history: {}", e);
                    }

                    // Log successful SMS notification
                    if let Err(e) = state.user_repository.log_usage(LogUsageParams {
                        user_id,
                        sid: Some(response_sid),
                        activity_type: content_type,
                        credits: None,
                        time_consumed: None,
                        success: Some(true),
                        reason: None,
                        status: Some("delivered".to_string()),
                        recharge_threshold_timestamp: None,
                        zero_credits_timestamp: None,
                    }) {
                        tracing::error!("Failed to log SMS notification usage: {}", e);
                    }
                    // SMS credits deducted at Twilio status callback
                }
                Err(e) => {
                    tracing::error!("Failed to send notification: {}", e);

                    // Log failed SMS notification
                    if let Err(log_err) = state.user_repository.log_usage(LogUsageParams {
                        user_id,
                        sid: None,
                        activity_type: content_type,
                        credits: None,
                        time_consumed: None,
                        success: Some(false),
                        reason: Some(format!("Failed to send SMS: {}", e)),
                        status: Some("failed".to_string()),
                        recharge_threshold_timestamp: None,
                        zero_credits_timestamp: None,
                    }) {
                        tracing::error!("Failed to log failed SMS notification: {}", log_err);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // hours_until Tests (internal function)
    // =========================================================================

    #[test]
    fn test_hours_until_same_hour() {
        assert_eq!(hours_until(12, 12), 0);
        assert_eq!(hours_until(0, 0), 0);
        assert_eq!(hours_until(23, 23), 0);
    }

    #[test]
    fn test_hours_until_later_hour() {
        assert_eq!(hours_until(8, 12), 4); // 8am to noon
        assert_eq!(hours_until(0, 23), 23); // Midnight to 11pm
        assert_eq!(hours_until(10, 18), 8); // 10am to 6pm
    }

    #[test]
    fn test_hours_until_earlier_hour_wraps_around() {
        assert_eq!(hours_until(18, 8), 14); // 6pm to 8am next day (14 hours)
        assert_eq!(hours_until(23, 1), 2); // 11pm to 1am next day
        assert_eq!(hours_until(20, 6), 10); // 8pm to 6am next day
    }

    // =========================================================================
    // hours_since Tests
    // =========================================================================

    #[test]
    fn test_hours_since_same_hour() {
        assert_eq!(hours_since(12, 12), 0);
        assert_eq!(hours_since(0, 0), 0);
    }

    #[test]
    fn test_hours_since_later_hour() {
        assert_eq!(hours_since(12, 8), 4); // Noon since 8am
        assert_eq!(hours_since(23, 0), 23); // 11pm since midnight
        assert_eq!(hours_since(18, 10), 8); // 6pm since 10am
    }

    #[test]
    fn test_hours_since_earlier_hour_wraps_around() {
        assert_eq!(hours_since(6, 20), 10); // 6am since 8pm (10 hours ago)
        assert_eq!(hours_since(1, 23), 2); // 1am since 11pm (2 hours ago)
        assert_eq!(hours_since(8, 18), 14); // 8am since 6pm yesterday
    }

    // =========================================================================
    // Critical Message Prompt Constants Tests (internal constants)
    // =========================================================================

    #[test]
    fn test_critical_prompt_contains_key_criteria() {
        // Verify the prompt includes important classification criteria
        assert!(CRITICAL_PROMPT.contains("critical"));
        assert!(CRITICAL_PROMPT.contains("two hours") || CRITICAL_PROMPT.contains("2 h"));
        assert!(CRITICAL_PROMPT.contains("is_critical"));
        assert!(CRITICAL_PROMPT.contains("what_to_inform"));
        assert!(CRITICAL_PROMPT.contains("first_message"));
    }

    #[test]
    fn test_waiting_check_prompt_contains_key_elements() {
        // Verify the waiting check prompt has required elements
        assert!(WAITING_CHECK_PROMPT.contains("monitor"));
        assert!(WAITING_CHECK_PROMPT.contains("sms_message"));
        assert!(WAITING_CHECK_PROMPT.contains("match"));
        assert!(WAITING_CHECK_PROMPT.contains("should_notify"));
        assert!(WAITING_CHECK_PROMPT.contains("updated_summary"));
    }
}
