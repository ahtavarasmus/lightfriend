use axum::{extract::State, http::StatusCode, Json};
use chrono::Datelike;
use serde::Serialize;
use std::sync::Arc;

use crate::{handlers::auth_middleware::AuthUser, AppState, UserCoreOps};

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
    pub timestamp: i32,       // Unix timestamp for positioning
    pub time_display: String, // "2:30pm"
    pub description: String,  // "Check on Mom"
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
    pub timestamp: i32,
    pub time_display: String,
    pub sources: Option<String>, // "email,whatsapp,telegram"
}

/// GET /api/dashboard/summary
/// Returns a minimal dashboard summary for the "peace of mind" view
pub async fn get_dashboard_summary(
    State(state): State<Arc<AppState>>,
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
            // Skip digest tasks and permanent recurring tasks for attention
            if task.action == "generate_digest" {
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

    // Find all upcoming tasks for the next 7 days
    let upcoming_tasks = find_upcoming_tasks(&state, user_id, now_ts, &tz);

    // Find all upcoming digests for the next 7 days
    let upcoming_digests = find_upcoming_digests(&state, user_id, now_ts, &tz);

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
        // Skip digest tasks - they have their own display
        if task.action == "generate_digest" {
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
    tz: &chrono_tz::Tz,
) -> Vec<UpcomingTask> {
    let tasks = match state.user_repository.get_user_tasks(user_id) {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    // 7 days in seconds
    let seven_days = 7 * 24 * 60 * 60;
    let max_ts = now_ts + seven_days;

    let mut upcoming: Vec<UpcomingTask> = Vec::new();

    for task in &tasks {
        // Skip digest tasks - they have their own display
        if task.action == "generate_digest" {
            continue;
        }

        // Parse trigger timestamp
        if let Some(ts_str) = task.trigger.strip_prefix("once_") {
            if let Ok(trigger_ts) = ts_str.parse::<i32>() {
                // Only consider future tasks within 7 days
                if trigger_ts > now_ts && trigger_ts <= max_ts {
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

                    upcoming.push(UpcomingTask {
                        task_id: task.id,
                        timestamp: trigger_ts,
                        time_display,
                        description,
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
    tz: &chrono_tz::Tz,
) -> Vec<UpcomingDigest> {
    let tasks = match state.user_repository.get_user_tasks(user_id) {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    // 7 days in seconds
    let seven_days = 7 * 24 * 60 * 60;
    let max_ts = now_ts + seven_days;

    let mut digests: Vec<UpcomingDigest> = Vec::new();

    for task in &tasks {
        // Only include digest tasks
        if task.action != "generate_digest" {
            continue;
        }

        // Parse trigger timestamp
        if let Some(ts_str) = task.trigger.strip_prefix("once_") {
            if let Ok(trigger_ts) = ts_str.parse::<i32>() {
                // Only consider future digests within 7 days
                if trigger_ts > now_ts && trigger_ts <= max_ts {
                    let time_display = format_time_display(trigger_ts, tz);

                    digests.push(UpcomingDigest {
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

    // Find digest tasks (action = "generate_digest" and is_permanent = 1)
    let digest_task = tasks
        .iter()
        .find(|t| t.action == "generate_digest" && t.is_permanent.unwrap_or(0) == 1)?;

    // Parse the trigger to get next occurrence
    if let Some(ts_str) = digest_task.trigger.strip_prefix("once_") {
        if let Ok(trigger_ts) = ts_str.parse::<i32>() {
            let time_display = format_relative_time(trigger_ts, now_ts, tz);
            return Some(NextDigestInfo { time_display });
        }
    }

    None
}

fn format_time_display(timestamp: i32, tz: &chrono_tz::Tz) -> String {
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

/// Format an action spec into a human-readable description
/// Examples:
/// - "control_tesla(climate_on)" -> "Tesla: Turn on climate"
/// - "control_tesla(climate_off)" -> "Tesla: Turn off climate"
/// - "send_reminder(Pick up package)" -> "Reminder: Pick up package"
/// - "get_weather(New York)" -> "Weather: New York"
/// - "generate_digest" -> "Generate digest"
fn format_action_description(action: &str) -> String {
    let action = action.trim();
    if action.is_empty() {
        return "Scheduled task".to_string();
    }

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
        // Check if it looks like a known tool
        if is_known_tool(first_action) {
            (first_action, None)
        } else {
            // Natural language - return as-is but capitalized
            return capitalize_first(first_action);
        }
    };

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
            // Format: send_chat_message(platform, recipient, message)
            if let Some(p) = param {
                let parts: Vec<&str> = p.splitn(3, ',').map(|s| s.trim()).collect();
                if parts.len() >= 2 {
                    let platform = capitalize_first(parts[0]);
                    let recipient = parts[1];
                    format!("{}: Message {}", platform, recipient)
                } else {
                    "Send message".to_string()
                }
            } else {
                "Send message".to_string()
            }
        }
        "get_weather" => {
            let location = param.unwrap_or("current location");
            format!("Weather: {}", location)
        }
        "fetch_calendar_events" => "Calendar: Fetch events".to_string(),
        "generate_digest" => "Generate digest".to_string(),
        _ => {
            // Generic formatting: replace underscores with spaces and capitalize
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
