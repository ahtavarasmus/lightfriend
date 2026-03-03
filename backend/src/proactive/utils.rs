use crate::repositories::user_repository::LogUsageParams;
use crate::AppState;
use crate::UserCoreOps;
use openai_api_rs::v1::{chat_completion, types};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::context::ContextBuilder;
use chrono::{Datelike, NaiveDate, NaiveDateTime};
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

/// Parsed structured tags from an item summary's first line.
#[derive(Debug, Default)]
pub struct ParsedTags {
    /// Item type: "oneshot", "recurring", or "tracking"
    pub item_type: Option<String>,
    /// Notification type: "sms", "call", or "silent"
    pub notify: Option<String>,
    /// Repeat pattern: "daily HH:MM", "weekdays HH:MM", "weekly DAY HH:MM", "none"
    pub repeat: Option<String>,
    /// Fetch tools to call: list of "email", "chat", "calendar", "weather", "items"
    pub fetch: Vec<String>,
    /// Sender name from [sender:X] tag (for monitor grouping)
    pub sender: Option<String>,
    /// Platform from [platform:X] tag
    pub platform: Option<String>,
    /// Topic from [topic:X] tag
    pub topic: Option<String>,
    /// Quiet rule type from [quiet:X] tag: "suppress" or "allow"
    pub quiet: Option<String>,
    /// Whether any structured tags were found
    pub has_tags: bool,
}

/// Parse structured [key:value] tags from the first line of a summary.
/// Returns parsed tags and whether the summary has structured tags.
pub fn parse_summary_tags(summary: &str) -> ParsedTags {
    let first_line = summary.lines().next().unwrap_or("");
    let mut tags = ParsedTags::default();

    // Check if first line contains any [key:value] patterns
    if !first_line.contains('[') {
        return tags;
    }

    for cap in regex::Regex::new(r"\[(\w+):([^\]]+)\]")
        .unwrap()
        .captures_iter(first_line)
    {
        let key = cap.get(1).unwrap().as_str();
        let value = cap.get(2).unwrap().as_str();
        match key {
            "type" => {
                tags.item_type = Some(value.to_string());
                tags.has_tags = true;
            }
            "notify" => {
                tags.notify = Some(value.to_string());
                tags.has_tags = true;
            }
            "repeat" => {
                tags.repeat = Some(value.to_string());
                tags.has_tags = true;
            }
            "fetch" => {
                tags.fetch = value.split(',').map(|s| s.trim().to_string()).collect();
                tags.has_tags = true;
            }
            "sender" => {
                tags.sender = Some(value.to_string());
                tags.has_tags = true;
            }
            "platform" => {
                tags.platform = Some(value.to_string());
                tags.has_tags = true;
            }
            "topic" => {
                tags.topic = Some(value.to_string());
                tags.has_tags = true;
            }
            "quiet" => {
                tags.quiet = Some(value.to_string());
                tags.has_tags = true;
            }
            "scope" => {
                tags.has_tags = true;
            }
            _ => {}
        }
    }

    tags
}

/// Compute priority from parsed tags, falling back to legacy heuristics.
/// Returns the determined priority, or None if the LLM should decide (legacy items).
pub fn compute_priority_from_tags(
    tags: &ParsedTags,
    summary: &str,
    current_priority: i32,
) -> Option<i32> {
    if let Some(ref notify) = tags.notify {
        return match notify.as_str() {
            "call" => Some(2),
            "silent" => Some(0),
            "sms" => Some(1),
            _ => Some(current_priority),
        };
    }
    // Legacy fallback: check for [VIA CALL] in text
    if summary.contains("[VIA CALL]") {
        return Some(2);
    }
    // No tag, no legacy marker - let current priority stand
    if tags.has_tags {
        // Has other tags but no [notify] - default to sms
        Some(1)
    } else {
        // Fully legacy item - let LLM decide (but default to current)
        None
    }
}

/// Compute due_at from a [repeat:PATTERN] tag.
/// Returns ISO datetime string in the user's timezone, or None for one-shot items.
pub fn compute_next_check_at(tags: &ParsedTags, tz_str: &str) -> Option<String> {
    let pattern = tags.repeat.as_deref()?;
    if pattern == "none" {
        return None;
    }

    let tz: chrono_tz::Tz = tz_str.parse().unwrap_or(chrono_tz::UTC);
    let now = chrono::Utc::now().with_timezone(&tz);

    // Parse patterns like "daily 09:00", "weekdays 09:00", "weekly Monday 09:00"
    let parts: Vec<&str> = pattern.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return None;
    }

    let frequency = parts[0];
    let rest = parts[1];

    match frequency {
        "daily" => {
            let time_parts: Vec<&str> = rest.split(':').collect();
            if time_parts.len() >= 2 {
                let hour: u32 = time_parts[0].parse().ok()?;
                let minute: u32 = time_parts[1].parse().ok()?;
                let mut next = now.date_naive().succ_opt()?.and_hms_opt(hour, minute, 0)?;
                // If still today and time hasn't passed, use today
                if let Some(today) = now.date_naive().and_hms_opt(hour, minute, 0) {
                    if today > now.naive_local() {
                        next = today;
                    }
                }
                Some(next.format("%Y-%m-%dT%H:%M").to_string())
            } else {
                None
            }
        }
        "weekdays" => {
            let time_parts: Vec<&str> = rest.split(':').collect();
            if time_parts.len() >= 2 {
                let hour: u32 = time_parts[0].parse().ok()?;
                let minute: u32 = time_parts[1].parse().ok()?;
                // Find next weekday
                let mut candidate = now.date_naive();
                // Check if we can still do today
                if let Some(today_time) = candidate.and_hms_opt(hour, minute, 0) {
                    if today_time > now.naive_local()
                        && candidate.weekday().num_days_from_monday() < 5
                    {
                        return Some(today_time.format("%Y-%m-%dT%H:%M").to_string());
                    }
                }
                // Otherwise find next weekday
                for _ in 0..7 {
                    candidate = candidate.succ_opt()?;
                    if candidate.weekday().num_days_from_monday() < 5 {
                        let next = candidate.and_hms_opt(hour, minute, 0)?;
                        return Some(next.format("%Y-%m-%dT%H:%M").to_string());
                    }
                }
                None
            } else {
                None
            }
        }
        "weekly" => {
            // "weekly Monday 09:00"
            let sub_parts: Vec<&str> = rest.splitn(2, ' ').collect();
            if sub_parts.len() >= 2 {
                let day_name = sub_parts[0].to_lowercase();
                let time_str = sub_parts[1];
                let time_parts: Vec<&str> = time_str.split(':').collect();
                if time_parts.len() >= 2 {
                    let hour: u32 = time_parts[0].parse().ok()?;
                    let minute: u32 = time_parts[1].parse().ok()?;

                    let target_weekday = match day_name.as_str() {
                        "monday" | "mon" => chrono::Weekday::Mon,
                        "tuesday" | "tue" => chrono::Weekday::Tue,
                        "wednesday" | "wed" => chrono::Weekday::Wed,
                        "thursday" | "thu" => chrono::Weekday::Thu,
                        "friday" | "fri" => chrono::Weekday::Fri,
                        "saturday" | "sat" => chrono::Weekday::Sat,
                        "sunday" | "sun" => chrono::Weekday::Sun,
                        _ => return None,
                    };

                    let mut candidate = now.date_naive();
                    for _ in 0..8 {
                        if candidate.weekday() == target_weekday {
                            let next = candidate.and_hms_opt(hour, minute, 0)?;
                            if next > now.naive_local() {
                                return Some(next.format("%Y-%m-%dT%H:%M").to_string());
                            }
                        }
                        candidate = candidate.succ_opt()?;
                    }
                    None
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Definition of a **critical** message: something that will cause human‑safety risk,
/// major financial/data loss, legal breach, or production outage if it waits >2 h.
/// The model must default to *non‑critical* when uncertain.
/// Prompt for matching incoming messages against the user’s *waiting checks*.
/// A waiting check represents something the user explicitly asked to be notified
/// about (e.g. \"Tell me when the shipment arrives\").
const WAITING_CHECK_PROMPT: &str = r#"You are an AI that determines whether an incoming message matches one of the user's tracking items.

Each tracking item may have structured header tags on its first line: [platform:X] [sender:Y] [topic:Z] or [scope:any], followed by a natural language description. Items without tags are legacy - use pure semantic reasoning on those.

**Matching heuristics:**

1. **Sender**: Does the message From field plausibly refer to the same person/entity as [sender:X]? Name matching is fuzzy: "mom", "Mom", "Mama" are the same; "hr@company.com" matches [sender:HR]. [sender:any] skips sender check.

2. **Platform**: [platform:any] matches everything. [platform:chat] matches whatsapp/telegram/signal. Exact platform match is a strong signal. Cross-platform match is allowed when sender AND topic clearly align.

3. **Topic**: Does the message content relate to [topic:X]? Use semantic reasoning. Translate non-English internally. If [scope:any] is present, skip topic check - any message from the sender matches. If no topic tag and no [scope:any], infer topic from the natural language description.

4. **Decision**:
   - Same platform + sender match + topic match = MATCH
   - Same platform + sender match + [scope:any] = MATCH
   - Cross-platform + sender + strong topic alignment = MATCH
   - Sender match alone (no topic match, no [scope:any]) = NO MATCH
   - Topic match alone (sender specified but wrong) = NO MATCH
   - Very short messages ("ok", "thanks") = NO MATCH unless [scope:any] from the right sender

5. If NO item matches, return is_match=false and task_id=null. Most messages will NOT match.

6. If multiple items could match, pick the single best by specificity.
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
    pub is_match: bool,
}

/// Determine whether `message` matches **one** of the supplied tracking items.
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
                "Current time: {}\n\nIncoming message:\n\n{}\n\nTracking items:\n\n{}\n\nReturn the best match or null.",
                chrono::Utc::now().to_rfc3339(), message, items_str
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let mut properties = std::collections::HashMap::new();
    properties.insert(
        "is_match".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "true if a tracking item matches the message, false if no item matches."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );
    properties.insert(
        "task_id".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some(
                "ID of the matched item when is_match is true. Null when is_match is false."
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
                "Determines whether the message matches a tracking item.".to_string(),
            ),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec!["is_match".to_string(), "task_id".to_string()]),
            },
        },
    }];

    let request = chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages)
        .tools(tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .temperature(0.0);

    let result = ctx.client.chat_completion(request).await?;
    let tool_call = result.choices[0]
        .message
        .tool_calls
        .as_ref()
        .and_then(|tc| tc.first())
        .ok_or("No tool call in item tracking match response")?;

    let args = tool_call
        .function
        .arguments
        .as_ref()
        .ok_or("No arguments in item tracking match tool call")?;

    let response: ItemMatchResponse = serde_json::from_str(args)?;

    if response.is_match && response.task_id.is_some() {
        Ok(Some(response))
    } else {
        Ok(None)
    }
}

/// Deserialize a number that may be integer or float into Option<i32>.
/// LLMs sometimes return `1.0` instead of `1`.
fn deserialize_optional_int_from_float<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    use serde_json::Value;
    let v = Option::<Value>::deserialize(deserializer)?;
    match v {
        None => Ok(None),
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                Ok(Some(i as i32))
            } else if let Some(f) = n.as_f64() {
                Ok(Some(f as i32))
            } else {
                Err(de::Error::custom("invalid number for priority"))
            }
        }
        Some(Value::Null) => Ok(None),
        _ => Err(de::Error::custom("expected number or null for priority")),
    }
}

/// Result from processing a triggered item via LLM.
#[derive(Debug, Deserialize)]
pub struct TriggeredItemResult {
    /// Updated summary for the item. Defaults to empty for oneshot/recurring (overridden in Rust).
    #[serde(default)]
    pub summary: String,
    /// If present, notify the user with this message (max 480 chars for digest, 160 for simple)
    #[serde(default)]
    pub sms_message: Option<String>,
    /// ISO datetime for next trigger; omit if item is done (will be deleted)
    #[serde(default)]
    pub due_at: Option<String>,
    /// 0 = background/digest-only, 1 = SMS notification, 2 = phone call
    #[serde(default, deserialize_with = "deserialize_optional_int_from_float")]
    pub priority: Option<i32>,
    /// For tracking items: true if the tracked condition is fully met and tracking should stop
    #[serde(default)]
    pub tracking_complete: Option<bool>,
}

/// Handle the result of processing a triggered item.
///
/// Sends notification (if sms_message present and priority >= 1),
/// reschedules (if due_at present, with safety clamps),
/// or deletes the item (if due_at absent).
pub async fn handle_triggered_item_result(
    state: &Arc<AppState>,
    user_id: i32,
    item_id: i32,
    current_priority: i32,
    response: &TriggeredItemResult,
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let new_priority = response.priority.unwrap_or(current_priority);

    // Send notification if sms_message present (non-empty) and priority >= 1
    if let Some(ref sms) = response.sms_message {
        if !sms.is_empty() && new_priority >= 1 {
            let noti_type = if new_priority >= 2 { "call" } else { "sms" };
            send_notification(
                state,
                user_id,
                sms,
                format!("item_{}", noti_type),
                Some("Hey, you have a notification!".to_string()),
            )
            .await;
        }
    }

    // Reschedule or delete
    if let Some(ref next) = response.due_at {
        let next_ts = parse_iso_to_timestamp(next)
            .map(|ts| ts.max(now + 60).min(now + 30 * 86400))
            .unwrap_or(now + 86400);
        let _ = state.item_repository.update_item(
            item_id,
            user_id,
            &response.summary,
            Some(next_ts),
            new_priority,
        );
        tracing::debug!("Item {} rescheduled to {}", item_id, next_ts);
    } else {
        let _ = state.item_repository.delete_item(item_id, user_id);
        tracing::debug!("Item {} completed and deleted", item_id);
    }
}

const NOTIFICATION_PROMPT: &str = r#"Convert this item into an SMS notification for the user. Call process_item_result with your sms_message.

Rules:
- Max 480 chars, plain text, second person
- Be direct and natural, like a friend reminding them
- Include specific details (times, names, amounts)
- If pre-fetched data is provided, summarize it: group by platform, use teasers (sender + topic hint + relative time), important items first, compress low-priority with '+ N other messages'
- If the item checks a condition and the data shows the condition is NOT met, return an empty sms_message"#;

const TRACKING_PROMPT: &str = r#"Evaluate the tracking condition for this item. Context is either a matched message or fetched data.

Read the item description (below the [tags] line) to understand what's being tracked.

tracking_complete: true if the condition is fully resolved (e.g. package delivered, reply received, payment confirmed). false if still pending.
sms_message: notify if noteworthy. Empty string to skip. If [last_notified:X] is present, only notify if the situation has materially changed. Max 480 chars, plain text, lead with what happened.
summary: preserve the entire first line (all [tags]) exactly. Append new findings below.
priority: only escalate (0->1) if warranted.

Call process_item_result."#;

/// Process a triggered item using LLM with optional tool-calling loop.
/// Supports simple reminders, digest/recurring items, and tracking items
/// that matched an incoming message.
///
/// Tags on the summary's first line are parsed **before** the LLM call:
/// - `[notify:X]`  -> priority is locked (not an LLM decision)
/// - `[repeat:X]`  -> due_at is computed deterministically
/// - `[fetch:X]`   -> only matching fetch tools are provided to the LLM
///
/// For legacy items (no tags), the LLM decides priority and due_at.
///
/// `matched_message`: `None` = time-fired trigger, `Some(msg)` = an incoming
/// message matched this item (tracking match from bridge/email).
pub async fn process_triggered_item(
    state: &Arc<AppState>,
    user_id: i32,
    item: &crate::models::user_models::Item,
    matched_message: Option<&str>,
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

    // ── Pre-compute from tags ──────────────────────────────────────────
    let tags = parse_summary_tags(&item.summary);
    let item_type = tags.item_type.as_deref().unwrap_or(""); // empty = legacy
    let pre_priority = compute_priority_from_tags(&tags, &item.summary, item.priority);
    // [repeat:X] -> deterministic due_at; no tag -> None (LLM may decide for legacy/tracking)
    let pre_due_at = compute_next_check_at(&tags, &tz_label).and_then(|local_iso| {
        let tz: chrono_tz::Tz = tz_label.parse().unwrap_or(chrono_tz::UTC);
        crate::tool_call_utils::utils::parse_user_datetime_to_utc(&local_iso, &tz)
            .ok()
            .map(|utc| utc.format("%Y-%m-%dT%H:%M:%SZ").to_string())
    });

    // Scheduling is locked for oneshot (always None) and recurring (always computed).
    // Tracking and legacy items let the LLM decide.
    let next_check_locked = match item_type {
        "oneshot" => true,
        "recurring" => true,
        "tracking" => false,
        _ => {
            // Legacy: locked if has_tags (old-style tagged without [type]),
            // unlocked if no tags at all
            tags.has_tags || pre_due_at.is_some()
        }
    };

    // ── Pre-call fetch tools for tagged items ────────────────────────
    // For items with [fetch:X] tags (non-tracking), pre-call tools in Rust
    // and inject results as context. This eliminates the tool-call loop.
    // Legacy items and tracking items keep the tool-call loop.
    let is_legacy = !tags.has_tags;
    let is_tracking = item_type == "tracking";
    let mut prefetched_context = String::new();

    if !is_legacy && !is_tracking && !tags.fetch.is_empty() {
        let mut fetch_results = Vec::new();
        for f in &tags.fetch {
            let result_text = match f.as_str() {
                "email" => crate::tool_call_utils::email::handle_fetch_emails(state, user_id).await,
                "chat" => {
                    let cutoff = chrono::Utc::now().timestamp() - 12 * 3600;
                    let mut chat_parts: Vec<String> = Vec::new();
                    for platform in &["whatsapp", "telegram", "signal"] {
                        if let Ok(Some(_)) = state.user_repository.get_bridge(user_id, platform) {
                            if let Ok(msgs) = crate::utils::bridge::fetch_bridge_messages(
                                platform, state, user_id, cutoff, true,
                            )
                            .await
                            {
                                if !msgs.is_empty() {
                                    let formatted: Vec<String> = msgs
                                        .into_iter()
                                        .map(|m| {
                                            format!(
                                                "  [{}] {}: {}",
                                                m.formatted_timestamp, m.room_name, m.content
                                            )
                                        })
                                        .collect();
                                    chat_parts.push(format!(
                                        "{}:\n{}",
                                        platform,
                                        formatted.join("\n")
                                    ));
                                }
                            }
                        }
                    }
                    if chat_parts.is_empty() {
                        "No recent chat messages.".to_string()
                    } else {
                        chat_parts.join("\n\n")
                    }
                }
                "calendar" => {
                    let tz: chrono_tz::Tz = tz_label.parse().unwrap_or(chrono_tz::UTC);
                    let now_local = chrono::Utc::now().with_timezone(&tz);
                    let end_local = now_local + chrono::Duration::hours(24);
                    let args = format!(
                        r#"{{"start":"{}","end":"{}"}}"#,
                        now_local.format("%Y-%m-%dT%H:%M"),
                        end_local.format("%Y-%m-%dT%H:%M"),
                    );
                    crate::tool_call_utils::calendar::handle_fetch_calendar_events(
                        state, user_id, &args,
                    )
                    .await
                }
                "weather" => {
                    // Use user's location for weather
                    let location = ctx
                        .user_info
                        .as_ref()
                        .and_then(|i| i.location.as_deref())
                        .unwrap_or("current location");
                    match crate::utils::tool_exec::get_weather(
                        state, location, "metric", "current", user_id,
                    )
                    .await
                    {
                        Ok(answer) => answer,
                        Err(e) => format!("Weather fetch failed: {}", e),
                    }
                }
                "items" => match state.item_repository.get_items(user_id) {
                    Ok(items) => {
                        let current_item_id = item.id;
                        let now_ts = chrono::Utc::now().timestamp() as i32;
                        let in_24h = now_ts + 24 * 3600;
                        let filtered: Vec<_> = items
                            .into_iter()
                            .filter(|i| {
                                if i.id == current_item_id {
                                    return false;
                                }
                                // Exclude recurring items
                                let i_tags = parse_summary_tags(&i.summary);
                                if i_tags.item_type.as_deref() == Some("recurring") {
                                    return false;
                                }
                                // Only include items due within the next 24 hours
                                match i.due_at {
                                    Some(ts) => ts >= now_ts && ts <= in_24h,
                                    None => false,
                                }
                            })
                            .collect();
                        if filtered.is_empty() {
                            "No items due in the next 24 hours.".to_string()
                        } else {
                            filtered
                                .iter()
                                .map(|i| {
                                    let next = i
                                        .due_at
                                        .map(|ts| format!(" (due: {})", ts))
                                        .unwrap_or_default();
                                    format!("- [P{}] {}{}", i.priority, i.summary, next)
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    }
                    Err(e) => format!("Failed to fetch tracked items: {}", e),
                },
                _ => continue,
            };
            fetch_results.push(format!("[{}]:\n{}", f, result_text));
        }
        if !fetch_results.is_empty() {
            prefetched_context = format!("\n\nPre-fetched data:\n{}", fetch_results.join("\n\n"));
        }
    }

    // ── Build user message ─────────────────────────────────────────────
    let mut user_msg = if let Some(msg) = matched_message {
        format!(
            "Current time: {} (timezone: {})\nItem priority: {}\n\nItem summary:\n{}\n\nMatched message:\n{}",
            time_str, tz_label, item.priority, item.summary, msg
        )
    } else {
        format!(
            "Current time: {} (timezone: {})\nItem priority: {}\n\nItem summary:\n{}",
            time_str, tz_label, item.priority, item.summary
        )
    };

    // Append pre-fetched context if available
    if !prefetched_context.is_empty() {
        user_msg.push_str(&prefetched_context);
    }

    let system_prompt = match item_type {
        "tracking" => TRACKING_PROMPT.to_string(),
        _ => NOTIFICATION_PROMPT.to_string(), // oneshot, recurring, legacy
    };

    let mut messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(system_prompt),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(user_msg),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    // ── Build result tool based on item type ─────────────────────────
    let mut result_properties = std::collections::HashMap::new();
    let mut required_fields = Vec::new();

    match item_type {
        "tracking" => {
            // Tracking: LLM can update summary, decide if tracking is complete, and escalate priority
            result_properties.insert(
                "summary".to_string(),
                Box::new(types::JSONSchemaDefine {
                    schema_type: Some(types::JSONSchemaType::String),
                    description: Some(
                        "Updated item summary. Update with new information gathered. Keep the first-line tags intact."
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
                        "Notification text for the user (max 480 chars, plain text, second person). Empty string to skip notification. Include when the item description asks to notify, or when the update is noteworthy. Empty only for routine noise unrelated to the tracked condition."
                            .to_string(),
                    ),
                    ..Default::default()
                }),
            );
            result_properties.insert(
                "tracking_complete".to_string(),
                Box::new(types::JSONSchemaDefine {
                    schema_type: Some(types::JSONSchemaType::Boolean),
                    description: Some(
                        "true if the tracked condition is fully met and tracking should stop (e.g. package delivered, reply received, payment confirmed). false to continue tracking."
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
                        "0 = background (no notification), 1 = SMS, 2 = phone call. Can escalate from silent to sms/call when something important happens."
                            .to_string(),
                    ),
                    ..Default::default()
                }),
            );
            required_fields.push("summary".to_string());
            required_fields.push("sms_message".to_string());
            required_fields.push("tracking_complete".to_string());
        }
        _ => {
            // All notification items (oneshot, recurring, legacy): just sms_message
            result_properties.insert(
                "sms_message".to_string(),
                Box::new(types::JSONSchemaDefine {
                    schema_type: Some(types::JSONSchemaType::String),
                    description: Some(
                        "Notification text for the user (max 480 chars, plain text, second person). ALWAYS provide this unless the summary is a conditional check whose condition is not met."
                            .to_string(),
                    ),
                    ..Default::default()
                }),
            );
            required_fields.push("sms_message".to_string());
        }
    }

    let result_tool = chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "process_item_result".to_string(),
            description: Some("Return the processing result for this triggered item.".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(result_properties),
                required: Some(required_fields),
            },
        },
    };

    // ── Build fetch tools ────────────────────────────────────────────
    let fetch_tracked_items_tool = {
        use openai_api_rs::v1::{chat_completion as cc, types as t};
        cc::Tool {
            r#type: cc::ToolType::Function,
            function: t::Function {
                name: "fetch_tracked_items".to_string(),
                description: Some(
                    "Fetch the user's tracked items (reminders, deadlines, tracking items). Returns a list of items Lightfriend is watching.".to_string(),
                ),
                parameters: t::FunctionParameters {
                    schema_type: t::JSONSchemaType::Object,
                    properties: Some(std::collections::HashMap::new()),
                    required: None,
                },
            },
        }
    };

    let mut tools = Vec::new();

    if is_legacy {
        // Legacy item: provide all fetch tools (LLM decides what to call)
        tools.push(crate::tool_call_utils::email::get_fetch_emails_tool());
        tools.push(crate::tool_call_utils::bridge::get_fetch_recent_messages_tool());
        tools.push(crate::tool_call_utils::calendar::get_fetch_calendar_event_tool());
        tools.push(crate::tool_call_utils::internet::get_weather_tool());
        tools.push(fetch_tracked_items_tool);
    } else if is_tracking {
        // Tracking items keep the tool-call loop for dynamic tool selection
        for f in &tags.fetch {
            match f.as_str() {
                "email" => tools.push(crate::tool_call_utils::email::get_fetch_emails_tool()),
                "chat" => {
                    tools.push(crate::tool_call_utils::bridge::get_fetch_recent_messages_tool())
                }
                "calendar" => {
                    tools.push(crate::tool_call_utils::calendar::get_fetch_calendar_event_tool())
                }
                "weather" => tools.push(crate::tool_call_utils::internet::get_weather_tool()),
                "items" => tools.push(fetch_tracked_items_tool.clone()),
                _ => {}
            }
        }
    }
    // For oneshot/recurring with [fetch:X], data was pre-fetched above - no fetch tools needed

    tools.push(result_tool);

    // ── Tool-calling loop (max 5 iterations) ───────────────────────────
    let re_last_notified_capture = regex::Regex::new(r"\[last_notified:(\d+)\]").unwrap();
    let re_last_notified_strip = regex::Regex::new(r"\s*\[last_notified:\d+\]").unwrap();

    for iteration in 0..5 {
        tracing::debug!(
            "process_triggered_item loop iteration {}/5, msg_count={}",
            iteration + 1,
            messages.len()
        );
        let request =
            chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages.clone())
                .tools(tools.clone())
                .tool_choice(chat_completion::ToolChoiceType::Required)
                .temperature(0.0);

        let result = ctx.client.chat_completion(request).await?;
        let choice = result.choices.first().ok_or("No choices in LLM response")?;

        let tool_calls = choice
            .message
            .tool_calls
            .as_ref()
            .ok_or("No tool calls in response")?;
        tracing::debug!(
            "process_triggered_item iteration {}: tools called: {:?}",
            iteration + 1,
            tool_calls
                .iter()
                .map(|tc| tc.function.name.as_deref().unwrap_or("?"))
                .collect::<Vec<_>>()
        );

        let mut found_result = None;
        let assistant_content = choice.message.content.clone().unwrap_or_default();

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
                let mut parsed: TriggeredItemResult = serde_json::from_str(fn_args)?;

                // ── Override with pre-computed values ───────────────
                match item_type {
                    "tracking" => {
                        let now_ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i32;

                        // 6-hour cooldown: block notification if [last_notified:X] was recent
                        if parsed.sms_message.as_ref().is_some_and(|s| !s.is_empty()) {
                            let last_notified = re_last_notified_capture
                                .captures(&item.summary)
                                .and_then(|c| c.get(1))
                                .and_then(|m| m.as_str().parse::<i32>().ok());
                            if let Some(last_ts) = last_notified {
                                if now_ts - last_ts < 6 * 3600 {
                                    parsed.sms_message = None; // Cooldown: block notification
                                }
                            }
                        }

                        // Rescheduling
                        if parsed.tracking_complete.unwrap_or(false) {
                            parsed.due_at = None; // Resolved - delete
                        } else if item.due_at.is_some_and(|d| d <= now_ts) {
                            parsed.due_at = None; // At/past deadline - auto-delete
                        } else {
                            parsed.due_at = item.due_at.map(|ts| ts.to_string());
                            // Preserve deadline
                        }

                        if parsed.priority.is_none() {
                            if let Some(p) = pre_priority {
                                parsed.priority = Some(p);
                            }
                        }
                    }
                    _ => {
                        // All notification items: summary frozen, priority locked, due_at from Rust
                        parsed.summary = item.summary.clone();
                        if let Some(p) = pre_priority {
                            parsed.priority = Some(p);
                        }
                        if next_check_locked || item_type == "oneshot" {
                            parsed.due_at = pre_due_at.clone(); // None for oneshot, computed for recurring
                        }
                    }
                }

                // Stamp [last_notified:X] on summary when tracking notification passes cooldown
                if item_type == "tracking"
                    && parsed.sms_message.as_ref().is_some_and(|s| !s.is_empty())
                {
                    let now_ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i32;
                    parsed.summary = re_last_notified_strip
                        .replace(&parsed.summary, "")
                        .to_string();
                    if let Some(newline_pos) = parsed.summary.find('\n') {
                        parsed
                            .summary
                            .insert_str(newline_pos, &format!(" [last_notified:{}]", now_ts));
                    }
                }

                found_result = Some(parsed);
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
                "fetch_tracked_items" => match state.item_repository.get_items(user_id) {
                    Ok(items) => {
                        let current_item_id = item.id;
                        let filtered: Vec<_> = items
                            .into_iter()
                            .filter(|i| i.id != current_item_id)
                            .collect();
                        if filtered.is_empty() {
                            "No tracked items.".to_string()
                        } else {
                            filtered
                                .iter()
                                .map(|i| {
                                    let next = i
                                        .due_at
                                        .map(|ts| format!(" (due: {})", ts))
                                        .unwrap_or_default();
                                    format!("- [P{}] {}{}", i.priority, i.summary, next)
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        }
                    }
                    Err(e) => format!("Failed to fetch tracked items: {}", e),
                },
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
        .temperature(0.2);
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
- `due_at` (string): ISO datetime for expiration deadline. Use the actual deadline if mentioned (e.g. invoice due date, delivery ETA). Otherwise use common sense with a minimum of 5 days. Empty string only if truly no deadline applies.
- `priority` (integer): 0 = digest-only, 1 = standalone notification, 2 = urgent

Include any deadline or due date directly in the summary text (e.g. "AWS invoice $45.20 due Feb 20").
"#;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TrackableResponse {
    is_trackable: bool,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    due_at: String,
    #[serde(default)]
    priority: i32,
}

/// Checks whether an email contains a trackable item and silently creates
/// a tracking item if detected. Uses source_id for dedup. Priority is set by the LLM.
pub async fn check_trackable_items(
    state: &Arc<AppState>,
    user_id: i32,
    email_uid: &str,
    email_content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Quick dedup: skip LLM call if we already have an item for this email
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
                "Current date: {}\n\nAnalyze this email for trackable items:\n\n{}",
                chrono::Utc::now().format("%Y-%m-%d"),
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
        "due_at".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "ISO datetime for expiration deadline. Sooner for urgent items.".to_string(),
            ),
            ..Default::default()
        }),
    );
    properties.insert(
        "priority".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some(
                "0 = digest-only, 1 = standalone notification, 2 = urgent".to_string(),
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
                    "due_at".to_string(),
                    "priority".to_string(),
                ]),
            },
        },
    }];

    let request = chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages)
        .tools(tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .temperature(0.1);

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

            let max_future = now + 30 * 86400; // 30 days cap
            let parsed_due = parse_iso_to_timestamp(&resp.due_at);
            let due_at = match parsed_due {
                Some(ts) if ts <= now => Some(now + 3600), // Past: clamp to now + 1 hour
                Some(ts) if ts > max_future => Some(max_future), // >30 days: cap
                Some(ts) => Some(ts),
                None => Some(now + 5 * 86400), // No deadline: default 5 days
            };

            let priority = 0; // Auto-created items are always silent

            // Build summary with tracking tags and fetch:email
            let tagged_summary = format!(
                "[type:tracking] [notify:silent] [fetch:email]\n{}",
                resp.summary
            );

            let new_item = crate::models::user_models::NewItem {
                user_id,
                summary: tagged_summary,
                due_at,
                priority,
                source_id: Some(source_id),
                created_at: now,
            };

            match state.item_repository.create_item_if_not_exists(&new_item) {
                Ok(Some(_id)) => {
                    tracing::debug!(
                        "Created tracking item for user {} (priority {})",
                        user_id,
                        priority
                    );
                }
                Ok(None) => {
                    tracing::debug!(
                        "Skipped duplicate trackable item for user {} source_id={}",
                        user_id,
                        new_item.source_id.as_deref().unwrap_or("?")
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to create trackable item for user {}: {}",
                        user_id,
                        e
                    );
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Message trackable item detection (bridge messages)
// ---------------------------------------------------------------------------

const MESSAGE_TRACKABLE_PROMPT: &str = r#"You are an AI that detects items worth tracking from chat messages (WhatsApp, Telegram, Signal, etc.).

A "trackable item" is something the user would want to remember or follow up on. Examples:
- Invoices or payment requests ("please pay invoice #1234")
- Questions or requests from people that need a response
- Deliveries and shipping updates ("your package shipped, tracking #1Z999")
- Appointments and scheduled events ("dentist at 3pm Thursday")
- Deadlines or due dates ("report due by Friday")
- Promises or commitments someone made ("I'll send the contract tomorrow")
- Important decisions or confirmations ("we agreed on $500/month")

Skip these - they are NOT trackable:
- Greetings, small talk, casual chat ("hey", "how are you", "lol")
- Simple acknowledgements ("ok", "thanks", "got it", "sure")
- Reactions or emoji-only messages
- Marketing/spam messages
- Messages with no actionable content

Return JSON with:
- `is_trackable` (boolean): whether this message contains something worth tracking
- `summary` (string, max 80 chars): concise description e.g. "Invoice #1234 from John - $450 due Friday" or "" if not trackable
- `topic` (string, max 20 chars): short topic label for dedup grouping e.g. "invoice", "delivery", "dentist". Empty if not trackable.
- `due_at` (string): ISO datetime for expiration deadline. Use the actual deadline if mentioned. Otherwise use common sense with a minimum of 5 days. Empty string only if truly no deadline applies.

Include any deadline or due date directly in the summary text.
"#;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct MessageTrackableResponse {
    is_trackable: bool,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    topic: String,
    #[serde(default)]
    due_at: String,
}

/// Checks whether a bridge message contains a trackable item and silently
/// creates a tracking item if detected. Always priority 0 (silent).
/// LLM decides due_at (expiration deadline) based on message urgency.
/// Uses source_id = "msg_{service}_{room_id}_{topic}" for dedup.
pub async fn check_message_trackable_items(
    state: &Arc<AppState>,
    user_id: i32,
    service: &str,
    room_id: &str,
    sender: &str,
    message_content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Skip very short messages - no point evaluating "ok" or "hi"
    if message_content.len() < 10 {
        return Ok(());
    }

    let ctx = ContextBuilder::for_user(state, user_id).build().await?;

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(MESSAGE_TRACKABLE_PROMPT.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(format!(
                "Current date: {}\nPlatform: {}\nFrom: {}\nMessage: {}",
                chrono::Utc::now().format("%Y-%m-%d"),
                service,
                sender,
                message_content
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
            description: Some("Whether the message contains something worth tracking".to_string()),
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
        "topic".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Short topic label for dedup grouping, max 20 chars (e.g. invoice, delivery)"
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
                "ISO datetime for expiration deadline. Sooner for urgent items (1 day), later for non-urgent (3-7 days). Empty string if no deadline."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    let tools = vec![chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "analyze_message_trackable".to_string(),
            description: Some("Analyzes if a chat message contains a trackable item".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    "is_trackable".to_string(),
                    "summary".to_string(),
                    "topic".to_string(),
                    "due_at".to_string(),
                ]),
            },
        },
    }];

    let request = chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages)
        .tools(tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .temperature(0.1);

    let result = ctx.client.chat_completion(request).await?;

    let response = result.choices[0]
        .message
        .tool_calls
        .as_ref()
        .and_then(|tc| tc.first())
        .and_then(|tc| tc.function.arguments.as_ref())
        .and_then(|args| serde_json::from_str::<MessageTrackableResponse>(args).ok());

    if let Some(resp) = response {
        if resp.is_trackable && !resp.summary.is_empty() {
            let topic = if resp.topic.is_empty() {
                "general"
            } else {
                &resp.topic
            };

            // Dedup: one item per topic per room
            let source_id = format!(
                "msg_{}_{}_{}",
                service,
                room_id,
                topic.to_lowercase().replace(' ', "_")
            );
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            let max_future = now + 30 * 86400; // 30 days cap
            let parsed_due = parse_iso_to_timestamp(&resp.due_at);
            let due_at = match parsed_due {
                Some(ts) if ts <= now => Some(now + 3600), // Past: clamp to now + 1 hour
                Some(ts) if ts > max_future => Some(max_future), // >30 days: cap
                Some(ts) => Some(ts),
                None => Some(now + 5 * 86400), // No deadline: default 5 days
            };

            let summary = format!(
                "[type:tracking] [notify:silent] [fetch:chat] [platform:{}] [sender:{}] [topic:{}]\n{}",
                service, sender, topic, resp.summary
            );

            let new_item = crate::models::user_models::NewItem {
                user_id,
                summary,
                due_at,
                priority: 0,
                source_id: Some(source_id.clone()),
                created_at: now,
            };

            match state.item_repository.create_item_if_not_exists(&new_item) {
                Ok(Some(_id)) => {
                    tracing::debug!(
                        "Created message tracking item for user {} source_id={}",
                        user_id,
                        source_id
                    );
                }
                Ok(None) => {
                    tracing::debug!(
                        "Message trackable item already exists for user {} source_id={}",
                        user_id,
                        source_id
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to create message tracking item for user {}: {}",
                        user_id,
                        e
                    );
                }
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
        .temperature(0.7);

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
        .tool_choice(chat_completion::ToolChoiceType::Required);
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

/// Metadata for contextual quiet-mode rule matching.
pub struct NotificationMeta {
    pub platform: Option<String>,
    pub sender: Option<String>,
    pub content: Option<String>,
}

/// Extract platform name from a content_type string like "whatsapp_profile_sms".
pub fn extract_platform_from_content_type(ct: &str) -> Option<String> {
    let ct_lower = ct.to_lowercase();
    for prefix in &[
        "whatsapp",
        "telegram",
        "signal",
        "email",
        "calendar",
        "tesla",
        "messenger",
        "instagram",
        "bluesky",
        "digest",
    ] {
        if ct_lower.starts_with(prefix) {
            return Some(prefix.to_string());
        }
    }
    None
}

pub async fn send_notification(
    state: &Arc<AppState>,
    user_id: i32,
    notification: &str,
    content_type: String,
    first_message: Option<String>,
) {
    send_notification_with_context(
        state,
        user_id,
        notification,
        content_type,
        first_message,
        None,
    )
    .await;
}

pub async fn send_notification_with_context(
    state: &Arc<AppState>,
    user_id: i32,
    notification: &str,
    content_type: String,
    first_message: Option<String>,
    meta: Option<NotificationMeta>,
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

    // Check quiet mode with rule-based filtering
    let inferred_platform = extract_platform_from_content_type(&content_type);
    let check_platform = meta
        .as_ref()
        .and_then(|m| m.platform.as_deref())
        .or(inferred_platform.as_deref());
    let check_sender = meta.as_ref().and_then(|m| m.sender.as_deref());
    let check_content = meta.as_ref().and_then(|m| m.content.as_deref());

    match state.user_core.check_quiet_with_context(
        user_id,
        check_platform,
        check_sender,
        check_content,
    ) {
        Ok(true) => {
            tracing::debug!(
                "Suppressed notification for user {} by quiet rule (platform={:?}, sender={:?})",
                user_id,
                check_platform,
                check_sender,
            );
            return;
        }
        Ok(false) => {}
        Err(e) => {
            tracing::error!(
                "Quiet mode check failed for user {}: {} - proceeding with notification",
                user_id,
                e
            );
            // Fail open - send the notification
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
    // Digests are always SMS-only (not affected by user's default notification type)
    let notification_type = if content_type.contains("digest") {
        "sms"
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
            // Call notification: Initiate call first (loud alert), then send SMS with content.
            // The call acts as a loud alert; SMS contains the actual message content.
            // If the user doesn't answer the call (call_initiation_failure webhook),
            // we don't charge for the call - only for the SMS.

            // Step 1: Check credits for SMS (always required)
            if let Err(e) =
                crate::utils::usage::check_user_credits(state, &user, "noti_msg", None).await
            {
                tracing::warn!(
                    "User {} has insufficient credits for call notification: {}",
                    user.id,
                    e
                );
                return;
            }

            // Step 2: Initiate call first as alert (charged conditionally via webhook)
            // The call will only be charged if the user answers (post_call_transcription webhook)
            // If declined/no-answer (call_initiation_failure webhook), no charge for call
            if crate::utils::usage::check_user_credits(state, &user, "noti_call", None)
                .await
                .is_ok()
            {
                match crate::api::elevenlabs::make_notification_call(
                    &state.clone(),
                    format!("{}_call_conditional", content_type),
                    first_message.clone().unwrap_or(
                        "Hello, you have a notification. Check your SMS for details.".to_string(),
                    ),
                    notification.to_string(),
                    user.id.to_string(),
                    user_info.timezone.clone(),
                )
                .await
                {
                    Ok(response) => {
                        tracing::info!(
                            "Call: Call initiated for user {} (will be charged if answered)",
                            user_id
                        );

                        // Log call usage as "ongoing" - it will be updated by webhook
                        if let Err(e) = state.user_repository.log_usage(LogUsageParams {
                            user_id,
                            sid: response
                                .get("sid")
                                .and_then(|v| v.as_str())
                                .map(String::from),
                            activity_type: format!("{}_call_conditional", content_type),
                            credits: None,
                            time_consumed: None,
                            success: None,
                            reason: None,
                            status: Some("ongoing".to_string()),
                            recharge_threshold_timestamp: None,
                            zero_credits_timestamp: None,
                        }) {
                            tracing::error!("Failed to log call notification call usage: {}", e);
                        }
                    }
                    Err((_, json_err)) => {
                        tracing::error!(
                            "Call: Failed to initiate call for user {}: {:?}",
                            user_id,
                            json_err
                        );
                    }
                }
            } else {
                tracing::info!(
                    "Call: Skipping call for user {} (insufficient credits for call)",
                    user_id
                );
            }

            // Step 3: Send SMS with message content (always sent regardless of call result)
            match state
                .twilio_message_service
                .send_sms(notification, None, &user)
                .await
            {
                Ok(response_sid) => {
                    tracing::info!("Call: SMS sent successfully for user {}", user_id);

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
                        tracing::error!("Failed to store call notification in history: {}", e);
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
                        tracing::error!("Failed to log call notification SMS usage: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Call: Failed to send SMS for user {}: {}", user_id, e);
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

/// Resolve email tracking items for emails the user has since read.
///
/// Fetches all tracking items with source_id prefix "email_", extracts UIDs,
/// opens a lightweight IMAP connection to check flags, and deletes items
/// whose emails now have the \Seen flag set.
pub async fn resolve_read_email_items(
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Only auto-feature plans (autopilot/byot) have tracking items
    let user_plan = state.user_repository.get_plan_type(user_id).unwrap_or(None);
    if !crate::utils::plan_features::has_auto_features(user_plan.as_deref()) {
        return Ok(());
    }

    // Get all email tracking items for this user
    let email_items = state
        .item_repository
        .get_items_by_source_prefix(user_id, "email_")?;

    if email_items.is_empty() {
        return Ok(());
    }

    // Extract UIDs from source_ids: "email_{uid}" -> "{uid}"
    let uid_to_item: std::collections::HashMap<String, i32> = email_items
        .iter()
        .filter_map(|item| {
            let source = item.source_id.as_deref()?;
            let uid = source.strip_prefix("email_")?;
            Some((uid.to_string(), item.id?))
        })
        .collect();

    if uid_to_item.is_empty() {
        return Ok(());
    }

    // Get IMAP credentials
    let (email, password, imap_server, imap_port) = state
        .user_repository
        .get_imap_credentials(user_id)?
        .ok_or("No IMAP credentials configured")?;

    // Connect to IMAP
    let tls = native_tls::TlsConnector::builder().build()?;
    let server = imap_server.as_deref().unwrap_or("imap.gmail.com");
    let port = imap_port.unwrap_or(993) as u16;
    let client = imap::connect((server, port), server, &tls)?;
    let mut session = client
        .login(&email, &password)
        .map_err(|(e, _)| format!("IMAP login failed: {}", e))?;
    session.select("INBOX")?;

    // Build a single UID FETCH for all UIDs at once
    let uid_list: String = uid_to_item.keys().cloned().collect::<Vec<_>>().join(",");

    let mut resolved = 0usize;
    if let Ok(fetched) = session.uid_fetch(&uid_list, "FLAGS") {
        for msg in fetched.iter() {
            if let Some(uid) = msg.uid {
                let uid_str = uid.to_string();
                if let Some(&item_id) = uid_to_item.get(&uid_str) {
                    // Check if \Seen flag is present
                    let is_read = msg.flags().iter().any(|flag| flag.to_string() == "\\Seen");
                    if is_read {
                        if let Ok(true) = state.item_repository.delete_item(item_id, user_id) {
                            resolved += 1;
                        }
                    }
                }
            }
        }
    }

    let _ = session.logout();

    if resolved > 0 {
        tracing::info!(
            "Auto-resolved {} email tracking item(s) for user {}",
            resolved,
            user_id
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(WAITING_CHECK_PROMPT.contains("tracking"));
        assert!(WAITING_CHECK_PROMPT.contains("match"));
        assert!(WAITING_CHECK_PROMPT.contains("item"));
    }
}
