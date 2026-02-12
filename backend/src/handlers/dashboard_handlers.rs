use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{handlers::auth_middleware::AuthUser, AppState, UserCoreOps};

#[derive(Deserialize)]
pub struct DashboardQuery {
    /// Unix timestamp for the end of the timeline range (default: now + 7 days)
    pub until: Option<i32>,
}

#[derive(Serialize)]
pub struct DashboardSummaryResponse {
    pub attention_count: i32,
    pub attention_items: Vec<AttentionItem>,
    pub next_scheduled: Option<ScheduledItem>,
    pub upcoming_tasks: Vec<UpcomingTask>,
    pub upcoming_digests: Vec<UpcomingDigest>,
    pub watched_contacts: Vec<WatchedContact>,
    pub next_digest: Option<NextDigestInfo>,
    pub quiet_mode: QuietModeInfo,
    pub sunrise_hour: Option<f32>,
    pub sunset_hour: Option<f32>,
    /// Tasks beyond the current timeline range (for preview in extend button tooltip)
    pub tasks_beyond: Vec<UpcomingTask>,
    /// Total count of tasks beyond the current timeline range
    pub tasks_beyond_count: i32,
}

#[derive(Serialize)]
pub struct QuietModeInfo {
    pub is_quiet: bool,
    pub until: Option<i32>,
    pub until_display: Option<String>,
}

#[derive(Serialize)]
pub struct AttentionItem {
    pub id: i32,
    pub item_type: String, // "email_critical", "bridge_disconnected", "task_due"
    pub summary: String,
    pub timestamp: i32,
    pub source: Option<String>,
}

#[derive(Serialize)]
pub struct ScheduledItem {
    pub time_display: String, // "2:30pm"
    pub description: String,  // "Check on Mom"
    pub task_id: Option<i32>,
}

#[derive(Serialize, Clone)]
pub struct UpcomingTask {
    pub task_id: Option<i32>,
    pub timestamp: i32,           // Unix timestamp for positioning
    pub time_display: String,     // "2:30pm"
    pub description: String,      // "Check on Mom"
    pub date_display: String,     // "Feb 10"
    pub relative_display: String, // "in 5 days"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>, // "if it's below freezing"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<String>, // raw source types: "weather,email"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources_display: Option<String>, // formatted: "Weather (Helsinki) + Email"
}

#[derive(Serialize)]
pub struct WatchedContact {
    pub nickname: String,
    pub notification_mode: String,
}

#[derive(Serialize)]
pub struct NextDigestInfo {
    pub time_display: String, // "9am tomorrow"
}

#[derive(Serialize, Clone)]
pub struct UpcomingDigest {
    pub task_id: Option<i32>,
    pub timestamp: i32,
    pub time_display: String,
    pub sources: Option<String>, // "email,whatsapp,telegram"
}

/// GET /api/dashboard/summary
/// Returns a minimal dashboard summary for the "peace of mind" view
/// Query params:
/// - `until`: Unix timestamp for the end of the timeline range (default: now + 7 days)
pub async fn get_dashboard_summary(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DashboardQuery>,
    auth_user: AuthUser,
) -> Result<Json<DashboardSummaryResponse>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;

    // Get user info for timezone and location
    let user_info = state.user_core.get_user_info(user_id).ok();
    let user_tz_str = user_info
        .as_ref()
        .and_then(|info| info.timezone.clone())
        .unwrap_or_else(|| "UTC".to_string());

    let tz: chrono_tz::Tz = user_tz_str.parse().unwrap_or(chrono_tz::UTC);
    let now = chrono::Utc::now();
    let now_ts = now.timestamp() as i32;

    // Calculate max_ts for timeline range (default: 7 days from now)
    let seven_days = 7 * 24 * 60 * 60;
    let max_ts = query.until.unwrap_or(now_ts + seven_days);

    // Use stored lat/lon (geocoded when user sets location)
    let (latitude, longitude) = match user_info.as_ref() {
        Some(info) => (
            info.latitude.map(|l| l as f64),
            info.longitude.map(|l| l as f64),
        ),
        None => (None, None),
    };

    // Calculate sunrise/sunset based on user's coordinates
    let (sunrise_hour, sunset_hour) =
        calculate_sun_times_from_coords(latitude, longitude, now, &tz);

    // Collect attention items from various sources
    let mut attention_items: Vec<AttentionItem> = Vec::new();

    // 1. Critical emails (should_notify = true) from last 24 hours
    let twenty_four_hours_ago = now_ts - (24 * 60 * 60);
    if let Ok(judgments) = state.user_repository.get_user_email_judgments(user_id) {
        for judgment in judgments {
            if judgment.should_notify && judgment.processed_at >= twenty_four_hours_ago {
                attention_items.push(AttentionItem {
                    id: judgment.id.unwrap_or(0),
                    item_type: "email_critical".to_string(),
                    summary: truncate_reason(&judgment.reason, 60),
                    timestamp: judgment.processed_at,
                    source: Some("email".to_string()),
                });
            }
        }
    }

    // 2. Bridge disconnection events (unhandled)
    if let Ok(events) = state
        .user_repository
        .get_pending_disconnection_events(user_id)
    {
        for event in events {
            attention_items.push(AttentionItem {
                id: event.id.unwrap_or(0),
                item_type: "bridge_disconnected".to_string(),
                summary: format!(
                    "{} disconnected",
                    capitalize_bridge_type(&event.bridge_type)
                ),
                timestamp: event.detected_at,
                source: Some(event.bridge_type.clone()),
            });
        }
    }

    // 3. Due/overdue tasks (non-digest, non-recurring active tasks)
    if let Ok(tasks) = state.user_repository.get_user_tasks(user_id) {
        for task in &tasks {
            // Skip digest/quiet_mode tasks and permanent recurring tasks for attention
            if is_digest_task(&task.action) || is_quiet_mode_task(&task.action) {
                continue;
            }
            if task.is_permanent.unwrap_or(0) == 1 {
                continue;
            }

            // Check if task is due (trigger timestamp has passed)
            if let Some(ts_str) = task.trigger.strip_prefix("once_") {
                if let Ok(trigger_ts) = ts_str.parse::<i32>() {
                    if trigger_ts <= now_ts {
                        attention_items.push(AttentionItem {
                            id: task.id.unwrap_or(0),
                            item_type: "task_due".to_string(),
                            summary: task.action.clone(),
                            timestamp: trigger_ts,
                            source: None,
                        });
                    }
                }
            }
        }
    }

    // Sort by timestamp (most recent first)
    attention_items.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    let attention_count = attention_items.len() as i32;

    // Find next scheduled item (soonest upcoming task)
    let next_scheduled = find_next_scheduled(&state, user_id, now_ts, &tz);

    // Find all upcoming tasks within the timeline range
    let upcoming_tasks = find_upcoming_tasks(&state, user_id, now_ts, max_ts, &tz);

    // Find all upcoming digests within the timeline range
    let upcoming_digests = find_upcoming_digests(&state, user_id, now_ts, max_ts, &tz);

    // Find tasks beyond the timeline range (for extend button)
    let (tasks_beyond, tasks_beyond_count) =
        find_tasks_beyond(&state, user_id, now_ts, max_ts, &tz);

    // Get watched contacts (contact profiles with notification modes)
    let watched_contacts = get_watched_contacts(&state, user_id);

    // Find next digest time
    let next_digest = find_next_digest(&state, user_id, now_ts, &tz);

    // Get quiet mode status
    let quiet_mode = get_quiet_mode_info(&state, user_id, now_ts, &tz);

    Ok(Json(DashboardSummaryResponse {
        attention_count,
        attention_items,
        next_scheduled,
        upcoming_tasks,
        upcoming_digests,
        watched_contacts,
        next_digest,
        quiet_mode,
        sunrise_hour,
        sunset_hour,
        tasks_beyond,
        tasks_beyond_count,
    }))
}

fn find_next_scheduled(
    state: &Arc<AppState>,
    user_id: i32,
    now_ts: i32,
    tz: &chrono_tz::Tz,
) -> Option<ScheduledItem> {
    let tasks = state.user_repository.get_user_tasks(user_id).ok()?;

    let mut next_task: Option<(&crate::models::user_models::Task, i32)> = None;

    for task in &tasks {
        // Skip digest/quiet_mode tasks - they have their own display
        if is_digest_task(&task.action) || is_quiet_mode_task(&task.action) {
            continue;
        }

        // Parse trigger timestamp
        if let Some(ts_str) = task.trigger.strip_prefix("once_") {
            if let Ok(trigger_ts) = ts_str.parse::<i32>() {
                // Only consider future tasks
                if trigger_ts > now_ts {
                    match &next_task {
                        Some((_, current_ts)) if trigger_ts < *current_ts => {
                            next_task = Some((task, trigger_ts));
                        }
                        None => {
                            next_task = Some((task, trigger_ts));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    next_task.map(|(task, trigger_ts)| {
        let time_display = format_time_display(trigger_ts, tz);
        // Use formatted action description if action exists, otherwise format condition
        let description = if !task.action.is_empty() {
            format_action_description(&task.action)
        } else if let Some(ref cond) = task.condition {
            // Also format condition if it looks like a tool call
            format_action_description(cond)
        } else {
            "Scheduled task".to_string()
        };

        ScheduledItem {
            time_display,
            description,
            task_id: task.id,
        }
    })
}

fn find_upcoming_tasks(
    state: &Arc<AppState>,
    user_id: i32,
    now_ts: i32,
    max_ts: i32,
    tz: &chrono_tz::Tz,
) -> Vec<UpcomingTask> {
    let tasks = match state.user_repository.get_user_tasks(user_id) {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    let mut upcoming: Vec<UpcomingTask> = Vec::new();

    for task in &tasks {
        // Skip digest/quiet_mode tasks - they have their own display
        if is_digest_task(&task.action) || is_quiet_mode_task(&task.action) {
            continue;
        }

        // Parse trigger timestamp
        if let Some(ts_str) = task.trigger.strip_prefix("once_") {
            if let Ok(trigger_ts) = ts_str.parse::<i32>() {
                // Only consider future tasks within the timeline range
                if trigger_ts > now_ts && trigger_ts <= max_ts {
                    let time_display = format_time_display(trigger_ts, tz);
                    let date_display = format_date_display(trigger_ts, tz);
                    let relative_display = format_relative_days(trigger_ts, now_ts, tz);
                    // Use formatted action description if action exists, otherwise format condition
                    let description = if !task.action.is_empty() {
                        format_action_description(&task.action)
                    } else if let Some(ref cond) = task.condition {
                        // Also format condition if it looks like a tool call
                        format_action_description(cond)
                    } else {
                        "Scheduled task".to_string()
                    };

                    // Extract condition - filter out JSON objects (those are action data, not conditions)
                    let condition = task.condition.as_ref().and_then(|c| {
                        let trimmed = c.trim();
                        if trimmed.starts_with('{') || trimmed.is_empty() {
                            None
                        } else {
                            Some(c.clone())
                        }
                    });

                    let sources = task.sources.clone();
                    let sources_display = sources
                        .as_ref()
                        .map(|s| format_sources_display(s, state, user_id));

                    upcoming.push(UpcomingTask {
                        task_id: task.id,
                        timestamp: trigger_ts,
                        time_display,
                        description,
                        date_display,
                        relative_display,
                        condition,
                        sources,
                        sources_display,
                    });
                }
            }
        }
    }

    // Sort by timestamp (earliest first)
    upcoming.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    upcoming
}

fn find_upcoming_digests(
    state: &Arc<AppState>,
    user_id: i32,
    now_ts: i32,
    max_ts: i32,
    tz: &chrono_tz::Tz,
) -> Vec<UpcomingDigest> {
    let tasks = match state.user_repository.get_user_tasks(user_id) {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    let mut digests: Vec<UpcomingDigest> = Vec::new();

    for task in &tasks {
        // Only include digest tasks
        if !is_digest_task(&task.action) {
            continue;
        }

        // Parse trigger timestamp
        if let Some(ts_str) = task.trigger.strip_prefix("once_") {
            if let Ok(trigger_ts) = ts_str.parse::<i32>() {
                // Only consider future digests within the timeline range
                if trigger_ts > now_ts && trigger_ts <= max_ts {
                    let time_display = format_time_display(trigger_ts, tz);

                    digests.push(UpcomingDigest {
                        task_id: task.id,
                        timestamp: trigger_ts,
                        time_display,
                        sources: task.sources.clone(),
                    });
                }
            }
        }
    }

    // Sort by timestamp (earliest first)
    digests.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    digests
}

/// Find tasks beyond the current timeline range (for the extend button)
/// Returns up to 5 tasks for preview and the total count
fn find_tasks_beyond(
    state: &Arc<AppState>,
    user_id: i32,
    now_ts: i32,
    max_ts: i32,
    tz: &chrono_tz::Tz,
) -> (Vec<UpcomingTask>, i32) {
    let tasks = match state.user_repository.get_user_tasks(user_id) {
        Ok(t) => t,
        Err(_) => return (vec![], 0),
    };

    // Look up to 90 days ahead to avoid scanning forever
    let ninety_days = 90 * 24 * 60 * 60;
    let lookahead_ts = max_ts + ninety_days;

    let mut beyond: Vec<UpcomingTask> = Vec::new();

    for task in &tasks {
        // Skip digest/quiet_mode tasks - they have their own display
        if is_digest_task(&task.action) || is_quiet_mode_task(&task.action) {
            continue;
        }

        // Parse trigger timestamp
        if let Some(ts_str) = task.trigger.strip_prefix("once_") {
            if let Ok(trigger_ts) = ts_str.parse::<i32>() {
                // Only consider tasks beyond max_ts but within lookahead
                if trigger_ts > max_ts && trigger_ts <= lookahead_ts {
                    let time_display = format_time_display(trigger_ts, tz);
                    let date_display = format_date_display(trigger_ts, tz);
                    let relative_display = format_relative_days(trigger_ts, now_ts, tz);
                    let description = if !task.action.is_empty() {
                        format_action_description(&task.action)
                    } else if let Some(ref cond) = task.condition {
                        format_action_description(cond)
                    } else {
                        "Scheduled task".to_string()
                    };

                    let condition = task.condition.as_ref().and_then(|c| {
                        let trimmed = c.trim();
                        if trimmed.starts_with('{') || trimmed.is_empty() {
                            None
                        } else {
                            Some(c.clone())
                        }
                    });

                    let sources = task.sources.clone();
                    let sources_display = sources
                        .as_ref()
                        .map(|s| format_sources_display(s, state, user_id));

                    beyond.push(UpcomingTask {
                        task_id: task.id,
                        timestamp: trigger_ts,
                        time_display,
                        description,
                        date_display,
                        relative_display,
                        condition,
                        sources,
                        sources_display,
                    });
                }
            }
        }
    }

    // Sort by timestamp (earliest first)
    beyond.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let total_count = beyond.len() as i32;
    // Return up to 5 for preview
    let preview = beyond.into_iter().take(5).collect();

    (preview, total_count)
}

fn get_watched_contacts(state: &Arc<AppState>, user_id: i32) -> Vec<WatchedContact> {
    state
        .user_repository
        .get_contact_profiles(user_id)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.notification_mode != "digest") // Only show those with active watching
        .map(|p| WatchedContact {
            nickname: p.nickname,
            notification_mode: p.notification_mode,
        })
        .collect()
}

fn find_next_digest(
    state: &Arc<AppState>,
    user_id: i32,
    now_ts: i32,
    tz: &chrono_tz::Tz,
) -> Option<NextDigestInfo> {
    let tasks = state.user_repository.get_user_tasks(user_id).ok()?;

    // Find digest tasks (action contains "generate_digest" and is_permanent = 1)
    let digest_task = tasks
        .iter()
        .find(|t| is_digest_task(&t.action) && t.is_permanent.unwrap_or(0) == 1)?;

    // Parse the trigger to get next occurrence
    if let Some(ts_str) = digest_task.trigger.strip_prefix("once_") {
        if let Ok(trigger_ts) = ts_str.parse::<i32>() {
            let time_display = format_relative_time(trigger_ts, now_ts, tz);
            return Some(NextDigestInfo { time_display });
        }
    }

    None
}

pub fn format_time_display(timestamp: i32, tz: &chrono_tz::Tz) -> String {
    use chrono::TimeZone;

    let dt = chrono::Utc
        .timestamp_opt(timestamp as i64, 0)
        .single()
        .map(|t| t.with_timezone(tz));

    match dt {
        Some(local_dt) => {
            let hour = local_dt.format("%l").to_string().trim().to_string();
            let minute = local_dt.format("%M").to_string();
            let ampm = local_dt.format("%P").to_string();

            if minute == "00" {
                format!("{}{}", hour, ampm)
            } else {
                format!("{}:{}{}", hour, minute, ampm)
            }
        }
        None => "?".to_string(),
    }
}

fn format_relative_time(timestamp: i32, now_ts: i32, tz: &chrono_tz::Tz) -> String {
    use chrono::TimeZone;

    let now_local = chrono::Utc
        .timestamp_opt(now_ts as i64, 0)
        .single()
        .map(|t| t.with_timezone(tz));

    let target_local = chrono::Utc
        .timestamp_opt(timestamp as i64, 0)
        .single()
        .map(|t| t.with_timezone(tz));

    match (now_local, target_local) {
        (Some(now), Some(target)) => {
            let time_part = format_time_display(timestamp, tz);

            let now_date = now.date_naive();
            let target_date = target.date_naive();
            let days_diff = (target_date - now_date).num_days();

            if days_diff == 0 {
                format!("{} today", time_part)
            } else if days_diff == 1 {
                format!("{} tomorrow", time_part)
            } else if days_diff < 7 {
                let day_name = target.format("%A").to_string();
                format!("{} {}", time_part, day_name)
            } else {
                let date_part = target.format("%b %d").to_string();
                format!("{} {}", time_part, date_part)
            }
        }
        _ => "unknown".to_string(),
    }
}

/// Format a date for tooltip display (e.g., "Feb 10")
pub fn format_date_display(timestamp: i32, tz: &chrono_tz::Tz) -> String {
    use chrono::TimeZone;

    let dt = chrono::Utc
        .timestamp_opt(timestamp as i64, 0)
        .single()
        .map(|t| t.with_timezone(tz));

    match dt {
        Some(local_dt) => local_dt.format("%b %d").to_string(),
        None => "?".to_string(),
    }
}

/// Format relative days for tooltip display (e.g., "in 5 days", "tomorrow", "today")
pub fn format_relative_days(timestamp: i32, now_ts: i32, tz: &chrono_tz::Tz) -> String {
    use chrono::TimeZone;

    let now_local = chrono::Utc
        .timestamp_opt(now_ts as i64, 0)
        .single()
        .map(|t| t.with_timezone(tz));

    let target_local = chrono::Utc
        .timestamp_opt(timestamp as i64, 0)
        .single()
        .map(|t| t.with_timezone(tz));

    match (now_local, target_local) {
        (Some(now), Some(target)) => {
            let now_date = now.date_naive();
            let target_date = target.date_naive();
            let days_diff = (target_date - now_date).num_days();

            if days_diff == 0 {
                "today".to_string()
            } else if days_diff == 1 {
                "tomorrow".to_string()
            } else {
                format!("in {} days", days_diff)
            }
        }
        _ => "".to_string(),
    }
}

/// Format source types into a human-readable display string.
/// e.g., "weather" -> "Weather (Helsinki)", "email,calendar" -> "Email + Calendar"
pub fn format_sources_display(sources: &str, state: &Arc<AppState>, user_id: i32) -> String {
    let parts: Vec<String> = sources
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .map(|source| match source.as_str() {
            "weather" => {
                let location = state
                    .user_core
                    .get_user_info(user_id)
                    .ok()
                    .and_then(|u| u.location);
                match location {
                    Some(loc) => format!("Weather ({})", loc),
                    None => "Weather".to_string(),
                }
            }
            "email" => "Email".to_string(),
            "calendar" => "Calendar".to_string(),
            "whatsapp" => "WhatsApp".to_string(),
            "telegram" => "Telegram".to_string(),
            "signal" => "Signal".to_string(),
            other => capitalize_first(other),
        })
        .collect();

    parts.join(" + ")
}

/// Check if a task's action is a digest task.
/// Handles both old format ("generate_digest") and new JSON format ({"tool":"generate_digest"}).
pub fn is_digest_task(action: &str) -> bool {
    if action == "generate_digest" {
        return true;
    }
    // Try JSON parse
    if let Ok(structured) =
        serde_json::from_str::<crate::utils::action_executor::StructuredAction>(action)
    {
        return structured.tool == "generate_digest";
    }
    false
}

/// Check if a task's action is a quiet mode task.
/// Handles both JSON format ({"tool":"quiet_mode"}) and plain string "quiet_mode".
pub fn is_quiet_mode_task(action: &str) -> bool {
    if action == "quiet_mode" {
        return true;
    }
    if let Ok(structured) =
        serde_json::from_str::<crate::utils::action_executor::StructuredAction>(action)
    {
        return structured.tool == "quiet_mode";
    }
    false
}

/// Format an action spec into a human-readable description.
/// Handles both new JSON format and old `tool(param)` format.
///
/// Examples:
/// - `{"tool":"send_reminder","params":{"message":"Call mom"}}` -> "Reminder: Call mom"
/// - "control_tesla(climate_on)" -> "Tesla: Turn on climate"
/// - "send_reminder(Pick up package)" -> "Reminder: Pick up package"
/// - "generate_digest" -> "Generate digest"
pub fn format_action_description(action: &str) -> String {
    let action = action.trim();
    if action.is_empty() {
        return "Scheduled task".to_string();
    }

    // Try JSON parse first (new structured format)
    if let Ok(structured) =
        serde_json::from_str::<crate::utils::action_executor::StructuredAction>(action)
    {
        return format_structured_action(&structured);
    }

    // Fall back to old format parsing
    // Handle multiple actions separated by periods - just take the first one for display
    let first_action = action.split('.').next().unwrap_or(action).trim();

    // Parse action(param) format
    let (tool, param) = if let Some(idx) = first_action.find('(') {
        if first_action.ends_with(')') {
            let tool = &first_action[..idx];
            let param = &first_action[idx + 1..first_action.len() - 1];
            (tool, Some(param))
        } else {
            // Doesn't look like a tool call - return as-is (natural language)
            return capitalize_first(first_action);
        }
    } else {
        // No parentheses - might be a simple tool name or natural language
        if is_known_tool(first_action) {
            (first_action, None)
        } else {
            return capitalize_first(first_action);
        }
    };

    format_tool_display(tool, param)
}

/// Format a StructuredAction into human-readable text
fn format_structured_action(action: &crate::utils::action_executor::StructuredAction) -> String {
    let params = action.params.as_ref();
    let tool = action.tool.as_str();

    // Extract the primary param as a string for display
    let param_str: Option<String> = params.and_then(|p| {
        let obj = p.as_object()?;
        let val = match tool {
            "send_reminder" => obj.get("message"),
            "control_tesla" => obj.get("command"),
            "get_weather" => obj.get("location"),
            "send_chat_message" => {
                // Build "platform,contact,message" for format_tool_display
                let platform = obj.get("platform").and_then(|v| v.as_str()).unwrap_or("");
                let contact = obj
                    .get("contact")
                    .or_else(|| obj.get("chat_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let message = obj.get("message").and_then(|v| v.as_str()).unwrap_or("");
                return Some(format!("{},{},{}", platform, contact, message));
            }
            "send_email" => {
                // Build "to,subject,body" for format_tool_display
                let to = obj.get("to").and_then(|v| v.as_str()).unwrap_or("");
                let subject = obj.get("subject").and_then(|v| v.as_str()).unwrap_or("");
                let body = obj.get("body").and_then(|v| v.as_str()).unwrap_or("");
                return Some(format!("{},{},{}", to, subject, body));
            }
            _ => obj.values().next(),
        };
        val.and_then(|v| v.as_str()).map(|s| s.to_string())
    });

    format_tool_display(tool, param_str.as_deref())
}

/// Shared formatting logic for tool + param display
fn format_tool_display(tool: &str, param: Option<&str>) -> String {
    match tool {
        "control_tesla" => {
            let cmd = param.unwrap_or("command");
            match cmd {
                "climate_on" => "Tesla: Turn on climate".to_string(),
                "climate_off" => "Tesla: Turn off climate".to_string(),
                "lock" => "Tesla: Lock".to_string(),
                "unlock" => "Tesla: Unlock".to_string(),
                "honk" => "Tesla: Honk horn".to_string(),
                "flash" => "Tesla: Flash lights".to_string(),
                "start" => "Tesla: Start vehicle".to_string(),
                "status" => "Tesla: Check status".to_string(),
                "location" => "Tesla: Get location".to_string(),
                _ => format!("Tesla: {}", cmd.replace('_', " ")),
            }
        }
        "send_reminder" => {
            let message = param.unwrap_or("reminder");
            format!("Reminder: {}", message)
        }
        "send_chat_message" => {
            if let Some(p) = param {
                let parts: Vec<&str> = p.splitn(3, ',').collect();
                let platform = capitalize_first(parts[0].trim());
                let contact = parts.get(1).map(|s| s.trim()).unwrap_or("");
                let message = parts.get(2).map(|s| s.trim()).unwrap_or("");
                match (!contact.is_empty(), !message.is_empty()) {
                    (true, true) => format!("{} to {}: {}", platform, contact, message),
                    (true, false) => format!("{} to {}", platform, contact),
                    (false, true) => format!("{}: {}", platform, message),
                    _ => format!("{}: Send message", platform),
                }
            } else {
                "Send message".to_string()
            }
        }
        "get_weather" => {
            let location = param.unwrap_or("current location");
            format!("Weather: {}", location)
        }
        "send_email" => {
            if let Some(p) = param {
                let parts: Vec<&str> = p.splitn(3, ',').collect();
                let to = parts[0].trim();
                let subject = parts.get(1).map(|s| s.trim()).unwrap_or("");
                let body = parts.get(2).map(|s| s.trim()).unwrap_or("");
                match (!to.is_empty(), !subject.is_empty(), !body.is_empty()) {
                    (true, true, true) => format!("Email to {} - {}: {}", to, subject, body),
                    (true, true, false) => format!("Email to {} - {}", to, subject),
                    (true, false, true) => format!("Email to {}: {}", to, body),
                    (true, false, false) => format!("Email to {}", to),
                    _ => "Send email".to_string(),
                }
            } else {
                "Send email".to_string()
            }
        }
        "fetch_calendar_events" => "Calendar: Fetch events".to_string(),
        "generate_digest" => "Generate digest".to_string(),
        _ => {
            let formatted = tool.replace('_', " ");
            capitalize_first(&formatted)
        }
    }
}

/// Check if a string looks like a known tool name
fn is_known_tool(name: &str) -> bool {
    matches!(
        name,
        "control_tesla"
            | "send_reminder"
            | "send_chat_message"
            | "send_email"
            | "get_weather"
            | "fetch_calendar_events"
            | "generate_digest"
    )
}

/// Capitalize the first letter of a string
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Scheduled task".to_string(),
    }
}

fn truncate_reason(reason: &str, max_len: usize) -> String {
    if reason.len() <= max_len {
        reason.to_string()
    } else {
        format!("{}...", &reason[..max_len - 3])
    }
}

fn capitalize_bridge_type(bridge_type: &str) -> String {
    let mut chars = bridge_type.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn get_quiet_mode_info(
    state: &Arc<AppState>,
    user_id: i32,
    now_ts: i32,
    tz: &chrono_tz::Tz,
) -> QuietModeInfo {
    let quiet_until = state.user_core.get_quiet_mode(user_id).ok().flatten();

    match quiet_until {
        None => QuietModeInfo {
            is_quiet: false,
            until: None,
            until_display: None,
        },
        Some(0) => QuietModeInfo {
            is_quiet: true,
            until: Some(0),
            until_display: Some("indefinitely".to_string()),
        },
        Some(ts) => {
            if ts <= now_ts {
                // Quiet mode expired - clear it and return not quiet
                let _ = state.user_core.set_quiet_mode(user_id, None);
                QuietModeInfo {
                    is_quiet: false,
                    until: None,
                    until_display: None,
                }
            } else {
                // Still in quiet mode - format the display time
                let until_display = format_relative_time(ts, now_ts, tz);
                QuietModeInfo {
                    is_quiet: true,
                    until: Some(ts),
                    until_display: Some(until_display),
                }
            }
        }
    }
}

/// Calculate sunrise and sunset hours from coordinates
/// Uses a simplified solar position algorithm
fn calculate_sun_times_from_coords(
    latitude: Option<f64>,
    longitude: Option<f64>,
    now: chrono::DateTime<chrono::Utc>,
    tz: &chrono_tz::Tz,
) -> (Option<f32>, Option<f32>) {
    let (lat, lon) = match (latitude, longitude) {
        (Some(lat), Some(lon)) => (lat, lon),
        _ => return (None, None),
    };

    // Convert to local date and get timezone offset
    let local_now = now.with_timezone(tz);
    let local_date = local_now.date_naive();
    let day_of_year = local_date.ordinal() as f64;

    // Get timezone offset in hours (e.g., UTC+2 = 2.0)
    use chrono::Offset;
    let tz_offset_hours = local_now.offset().fix().local_minus_utc() as f64 / 3600.0;

    // Solar declination (simplified equation)
    let gamma = ((360.0_f64 / 365.0) * (day_of_year - 81.0)).to_radians();
    let declination = 23.45_f64.to_radians() * gamma.sin();

    // Hour angle at sunrise/sunset
    let lat_rad = lat.to_radians();
    let cos_hour_angle = -(lat_rad.tan() * declination.tan());

    // Check for polar day/night
    if cos_hour_angle < -1.0 {
        // Polar day - sun never sets
        return (Some(0.0), Some(24.0));
    } else if cos_hour_angle > 1.0 {
        // Polar night - sun never rises
        return (Some(12.0), Some(12.0));
    }

    let hour_angle = cos_hour_angle.acos().to_degrees();

    // Solar noon in UTC: 12:00 adjusted for longitude
    // Then convert to local time by adding timezone offset
    let solar_noon_utc = 12.0 - (lon / 15.0);
    let solar_noon_local = solar_noon_utc + tz_offset_hours;

    // Calculate sunrise and sunset in local time
    let sunrise = solar_noon_local - (hour_angle / 15.0);
    let sunset = solar_noon_local + (hour_angle / 15.0);

    // Clamp to valid range
    let sunrise = sunrise.clamp(0.0, 24.0) as f32;
    let sunset = sunset.clamp(0.0, 24.0) as f32;

    (Some(sunrise), Some(sunset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_digest_task_old_format() {
        assert!(is_digest_task("generate_digest"));
    }

    #[test]
    fn test_is_digest_task_new_format() {
        assert!(is_digest_task(r#"{"tool":"generate_digest"}"#));
    }

    #[test]
    fn test_is_digest_task_not_digest() {
        assert!(!is_digest_task("send_reminder(Call mom)"));
        assert!(!is_digest_task(
            r#"{"tool":"send_reminder","params":{"message":"hi"}}"#
        ));
    }

    #[test]
    fn test_format_action_new_json_reminder() {
        let action = r#"{"tool":"send_reminder","params":{"message":"Call lena"}}"#;
        assert_eq!(format_action_description(action), "Reminder: Call lena");
    }

    #[test]
    fn test_format_action_new_json_tesla() {
        let action = r#"{"tool":"control_tesla","params":{"command":"climate_on"}}"#;
        assert_eq!(format_action_description(action), "Tesla: Turn on climate");
    }

    #[test]
    fn test_format_action_old_format_reminder() {
        assert_eq!(
            format_action_description("send_reminder(Pick up package)"),
            "Reminder: Pick up package"
        );
    }

    #[test]
    fn test_format_action_old_format_tesla() {
        assert_eq!(
            format_action_description("control_tesla(climate_on)"),
            "Tesla: Turn on climate"
        );
    }

    #[test]
    fn test_format_action_simple_tool() {
        assert_eq!(
            format_action_description("generate_digest"),
            "Generate digest"
        );
    }

    #[test]
    fn test_format_action_new_json_email() {
        let action = r#"{"tool":"send_email","params":{"to":"john@example.com","subject":"Meeting Update","body":"The meeting is moved"}}"#;
        assert_eq!(
            format_action_description(action),
            "Email to john@example.com - Meeting Update: The meeting is moved"
        );
    }

    #[test]
    fn test_format_action_new_json_email_no_subject() {
        let action = r#"{"tool":"send_email","params":{"to":"john@example.com","body":"Hello"}}"#;
        assert_eq!(
            format_action_description(action),
            "Email to john@example.com: Hello"
        );
    }

    #[test]
    fn test_format_action_old_format_email() {
        // Old format with dots in param is lossy (splits on '.') - known limitation
        // New JSON format handles this correctly
        assert_eq!(
            format_action_description("send_email(john)"),
            "Email to john"
        );
    }
}
