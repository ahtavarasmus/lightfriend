use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Datelike;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use std::collections::HashMap;

use crate::{handlers::auth_middleware::AuthUser, AppState, UserCoreOps};

#[derive(Deserialize)]
pub struct DashboardQuery {
    pub until: Option<i32>,
}

#[derive(Serialize)]
pub struct DashboardSummaryResponse {
    // Calm dashboard fields
    pub status: String, // "all_caught_up" | "needs_attention"
    pub messages_handled_today: i64,
    pub notifications_sent_today: i64,
    pub rules_active: i64,
    pub action_items: Vec<ActionItem>,
    pub filtered_count: i64,
    pub events: Vec<EventItem>,
    // Existing fields kept for compatibility
    pub quiet_mode: QuietModeInfo,
    pub sunrise_hour: Option<f32>,
    pub sunset_hour: Option<f32>,
    pub watched_contacts: Vec<WatchedContact>,
}

#[derive(Serialize)]
pub struct EventItem {
    pub id: i32,
    pub description: String,
    pub remind_at: Option<i32>,
    pub due_at: Option<i32>,
    pub status: String,
    pub created_at: i32,
}

#[derive(Serialize)]
pub struct EventMessageItem {
    pub id: i64,
    pub platform: String,
    pub sender_name: String,
    pub content: String,
    pub created_at: i32,
    pub room_id: String,
}

#[derive(Serialize)]
pub struct EventDetailResponse {
    pub event: EventItem,
    pub linked_messages: Vec<EventMessageItem>,
}

#[derive(Serialize)]
pub struct ActionItem {
    pub message_id: i64,
    pub person_name: String,
    pub platform: String,
    pub preview: String,
    pub timestamp: i32,
    pub person_id: Option<i32>,
}

#[derive(Serialize)]
pub struct QuietModeInfo {
    pub is_quiet: bool,
    pub until: Option<i32>,
    pub until_display: Option<String>,
    pub rule_count: i32,
}

#[derive(Serialize)]
pub struct WatchedContact {
    pub nickname: String,
    pub notification_mode: String,
}

/// GET /api/dashboard/summary
pub async fn get_dashboard_summary(
    State(state): State<Arc<AppState>>,
    Query(_query): Query<DashboardQuery>,
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

    // Calculate start of today in user's timezone
    let local_now = now.with_timezone(&tz);
    let today_start_local = local_now.date_naive().and_hms_opt(0, 0, 0).unwrap();
    let today_start_ts = today_start_local
        .and_local_timezone(tz)
        .earliest()
        .map(|dt| dt.timestamp() as i32)
        .unwrap_or(now_ts - 86400);

    // 24h ago for action items
    let last_24h_ts = now_ts - 86400;

    // Use stored lat/lon
    let (latitude, longitude) = match user_info.as_ref() {
        Some(info) => (
            info.latitude.map(|l| l as f64),
            info.longitude.map(|l| l as f64),
        ),
        None => (None, None),
    };

    let (sunrise_hour, sunset_hour) =
        calculate_sun_times_from_coords(latitude, longitude, now, &tz);

    // Count messages handled today
    let messages_handled_today = state
        .ontology_repository
        .count_messages_since(user_id, today_start_ts)
        .unwrap_or(0);

    // Count notifications sent today
    let notifications_sent_today = state
        .user_repository
        .count_notifications_since(user_id, today_start_ts)
        .unwrap_or(0);

    // Count active rules
    let active_rules = state
        .ontology_repository
        .get_active_rules(user_id)
        .unwrap_or_default();
    let rules_active = active_rules.len() as i64;

    // Get persons with channels for name lookups
    let persons = state
        .ontology_repository
        .get_persons_with_channels(user_id, 500, 0)
        .unwrap_or_default();

    // Get notifiable messages (from known persons in last 24h)
    let notifiable_messages = state
        .ontology_repository
        .get_notifiable_messages(user_id, last_24h_ts, 20)
        .unwrap_or_default();

    // Build action items with person name lookups
    let action_items: Vec<ActionItem> = notifiable_messages
        .iter()
        .map(|msg| {
            let person_name = msg
                .person_id
                .and_then(|pid| persons.iter().find(|p| p.person.id == pid))
                .map(|p| p.display_name().to_string())
                .unwrap_or_else(|| msg.sender_name.clone());

            let preview = if msg.content.len() > 100 {
                format!("{}...", &msg.content[..100])
            } else {
                msg.content.clone()
            };

            ActionItem {
                message_id: msg.id,
                person_name,
                platform: msg.platform.clone(),
                preview,
                timestamp: msg.created_at,
                person_id: msg.person_id,
            }
        })
        .collect();

    let action_count = action_items.len() as i64;
    let filtered_count = (messages_handled_today - action_count).max(0);
    let status = if action_items.is_empty() {
        "all_caught_up".to_string()
    } else {
        "needs_attention".to_string()
    };

    // Get watched contacts
    let watched_contacts: Vec<WatchedContact> = persons
        .iter()
        .flat_map(|person| {
            let name = person.display_name().to_string();
            person.channels.iter().filter_map(move |channel| {
                let mode = person.effective_notification_mode(channel, "digest");
                if mode != "digest" {
                    Some(WatchedContact {
                        nickname: format!("{} ({})", name, channel.platform),
                        notification_mode: mode,
                    })
                } else {
                    None
                }
            })
        })
        .collect();

    // Get quiet mode status
    let quiet_mode = get_quiet_mode_info(&state, user_id, now_ts, &tz);

    // Get active events
    let events: Vec<EventItem> = state
        .ontology_repository
        .get_active_events(user_id)
        .unwrap_or_default()
        .into_iter()
        .map(|e| EventItem {
            id: e.id,
            description: e.description,
            remind_at: e.remind_at,
            due_at: e.due_at,
            status: e.status,
            created_at: e.created_at,
        })
        .collect();

    Ok(Json(DashboardSummaryResponse {
        status,
        messages_handled_today,
        notifications_sent_today,
        rules_active,
        action_items,
        filtered_count,
        events,
        quiet_mode,
        sunrise_hour,
        sunset_hour,
        watched_contacts,
    }))
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

fn get_quiet_mode_info(
    state: &Arc<AppState>,
    user_id: i32,
    now_ts: i32,
    tz: &chrono_tz::Tz,
) -> QuietModeInfo {
    let quiet_until = state.user_core.get_quiet_mode(user_id).ok().flatten();

    let rule_count = state
        .ontology_repository
        .get_active_rules(user_id)
        .map(|r| r.len() as i32)
        .unwrap_or(0);

    match quiet_until {
        None => QuietModeInfo {
            is_quiet: false, // only quiet if explicitly set via quiet_until
            until: None,
            until_display: None,
            rule_count,
        },
        Some(0) => QuietModeInfo {
            is_quiet: true,
            until: Some(0),
            until_display: Some("indefinitely".to_string()),
            rule_count,
        },
        Some(ts) => {
            if ts <= now_ts {
                let _ = state.user_core.set_quiet_mode(user_id, None);
                QuietModeInfo {
                    is_quiet: false,
                    until: None,
                    until_display: None,
                    rule_count,
                }
            } else {
                let until_display = format_relative_time(ts, now_ts, tz);
                QuietModeInfo {
                    is_quiet: true,
                    until: Some(ts),
                    until_display: Some(until_display),
                    rule_count,
                }
            }
        }
    }
}

/// Calculate sunrise and sunset hours from coordinates
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

    let local_now = now.with_timezone(tz);
    let local_date = local_now.date_naive();
    let day_of_year = local_date.ordinal() as f64;

    use chrono::Offset;
    let tz_offset_hours = local_now.offset().fix().local_minus_utc() as f64 / 3600.0;

    let gamma = ((360.0_f64 / 365.0) * (day_of_year - 81.0)).to_radians();
    let declination = 23.45_f64.to_radians() * gamma.sin();

    let lat_rad = lat.to_radians();
    let cos_hour_angle = -(lat_rad.tan() * declination.tan());

    if cos_hour_angle < -1.0 {
        return (Some(0.0), Some(24.0));
    } else if cos_hour_angle > 1.0 {
        return (Some(12.0), Some(12.0));
    }

    let hour_angle = cos_hour_angle.acos().to_degrees();

    let solar_noon_utc = 12.0 - (lon / 15.0);
    let solar_noon_local = solar_noon_utc + tz_offset_hours;

    let sunrise = solar_noon_local - (hour_angle / 15.0);
    let sunset = solar_noon_local + (hour_angle / 15.0);

    let sunrise = sunrise.clamp(0.0, 24.0) as f32;
    let sunset = sunset.clamp(0.0, 24.0) as f32;

    (Some(sunrise), Some(sunset))
}

// -----------------------------------------------------------------------
// Activity Feed
// -----------------------------------------------------------------------

#[derive(Serialize)]
pub struct ActivityFeedEntry {
    pub id: String,
    pub entry_type: String, // "changelog", "notification", "message"
    pub timestamp: i32,
    pub title: String,
    pub detail: Option<String>,
    pub icon: String, // FA icon class
    pub success: Option<bool>,
}

#[derive(Deserialize)]
pub struct ActivityFeedQuery {
    pub since: Option<i32>,
    pub limit: Option<i64>,
}

/// GET /api/dashboard/activity-feed
pub async fn get_activity_feed(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ActivityFeedQuery>,
    auth_user: AuthUser,
) -> Result<Json<Vec<ActivityFeedEntry>>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;
    let now_ts = chrono::Utc::now().timestamp() as i32;
    let since_ts = query.since.unwrap_or(now_ts - 7 * 86400);
    let limit = query.limit.unwrap_or(100);

    // Fetch all data sources
    let changelog = state
        .ontology_repository
        .get_recent_changelog(user_id, since_ts, 50)
        .unwrap_or_default();

    let usage_logs = state
        .user_repository
        .get_recent_usage_logs(user_id, since_ts, 50)
        .unwrap_or_default();

    let messages = state
        .ontology_repository
        .get_recent_messages_all_platforms(user_id, since_ts)
        .unwrap_or_default();

    // Build name lookup maps
    let persons = state
        .ontology_repository
        .get_persons_with_channels(user_id, 500, 0)
        .unwrap_or_default();
    let person_names: HashMap<i32, String> = persons
        .iter()
        .map(|p| (p.person.id, p.display_name().to_string()))
        .collect();

    let rules = state
        .ontology_repository
        .get_rules(user_id)
        .unwrap_or_default();
    let rule_names: HashMap<i32, String> = rules.iter().map(|r| (r.id, r.name.clone())).collect();

    let mut entries: Vec<ActivityFeedEntry> = Vec::new();

    // Convert changelog entries
    for entry in &changelog {
        let entity_name = match entry.entity_type.to_lowercase().as_str() {
            "person" => person_names
                .get(&entry.entity_id)
                .cloned()
                .unwrap_or_else(|| format!("#{}", entry.entity_id)),
            "rule" => rule_names
                .get(&entry.entity_id)
                .cloned()
                .unwrap_or_else(|| format!("#{}", entry.entity_id)),
            "channel" => person_names
                .get(&entry.entity_id)
                .cloned()
                .unwrap_or_else(|| "channel".to_string()),
            _ => format!("#{}", entry.entity_id),
        };

        let (title, icon) = match (
            entry.entity_type.to_lowercase().as_str(),
            entry.change_type.as_str(),
        ) {
            ("person", "created") => (format!("New person: {}", entity_name), "fa-user-plus"),
            ("person", "updated") => (format!("Updated {}", entity_name), "fa-user-pen"),
            ("person", "deleted") => (format!("Removed {}", entity_name), "fa-user-minus"),
            ("person", "merged") => (format!("Merged into {}", entity_name), "fa-people-arrows"),
            ("channel", "created") => (format!("Channel added for {}", entity_name), "fa-plug"),
            ("channel", "deleted") => ("Channel removed".to_string(), "fa-plug-circle-minus"),
            ("rule", "created") => (
                format!("Rule created: {}", entity_name),
                "fa-wand-magic-sparkles",
            ),
            ("rule", "deleted") => (format!("Rule deleted: {}", entity_name), "fa-trash-can"),
            _ => (
                format!(
                    "{} {} {}",
                    entry.change_type, entry.entity_type, entity_name
                ),
                "fa-circle-info",
            ),
        };

        // Build human-readable detail from changed_fields JSON
        let detail = {
            let mut parts: Vec<String> = Vec::new();
            if entry.source == "pipeline" {
                parts.push("automatic".to_string());
            }
            if let Some(ref fields_json) = entry.changed_fields {
                if let Ok(fields) = serde_json::from_str::<serde_json::Value>(fields_json) {
                    if let Some(obj) = fields.as_object() {
                        for (key, val) in obj {
                            let val_str = val
                                .as_str()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| val.to_string());
                            // Skip internal fields, show user-facing ones
                            match key.as_str() {
                                "name" | "nickname" => parts.push(format!("Name: {}", val_str)),
                                "platform" => parts.push(format!("Platform: {}", val_str)),
                                "handle" => parts.push(format!("Handle: {}", val_str)),
                                "trigger_type" => {
                                    let t = match val_str.as_str() {
                                        "schedule" => "Scheduled",
                                        "ontology_change" => "Monitoring",
                                        _ => &val_str,
                                    };
                                    parts.push(format!("Type: {}", t));
                                }
                                "action_type" => {
                                    let a = match val_str.as_str() {
                                        "notify" => "Notify",
                                        "tool_call" => "Run action",
                                        _ => &val_str,
                                    };
                                    parts.push(format!("Action: {}", a));
                                }
                                _ => parts.push(format!("{}={}", key, val_str)),
                            }
                        }
                    } else {
                        // Not an object - use as-is
                        let s = fields_json.trim().to_string();
                        if !s.is_empty() && s != "null" {
                            parts.push(s);
                        }
                    }
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(" - "))
            }
        };

        entries.push(ActivityFeedEntry {
            id: format!("changelog-{}", entry.id),
            entry_type: "changelog".to_string(),
            timestamp: entry.created_at,
            title,
            detail,
            icon: format!("fa-solid {}", icon),
            success: None,
        });
    }

    // Convert usage logs
    for log in &usage_logs {
        let humanize_platform = |p: &str| -> String {
            match p {
                "whatsapp" | "Whatsapp" => "WhatsApp".to_string(),
                "telegram" | "Telegram" => "Telegram".to_string(),
                "signal" | "Signal" => "Signal".to_string(),
                "email" | "Email" => "email".to_string(),
                "rule" => "rule trigger".to_string(),
                "item" => "scheduled item".to_string(),
                other => other.to_string(),
            }
        };

        let (title, icon, detail) = match log.activity_type.as_str() {
            "noti_msg" => (
                "Sent you an SMS notification".to_string(),
                "fa-comment-sms",
                log.reason.clone(),
            ),
            "noti_call" => {
                let duration_str = log.call_duration.map(|d| {
                    if d < 60 {
                        format!("{}s", d)
                    } else {
                        format!("{}m {}s", d / 60, d % 60)
                    }
                });
                let detail = match (&log.reason, duration_str) {
                    (Some(r), Some(d)) => Some(format!("{} ({})", r, d)),
                    (Some(r), None) => Some(r.clone()),
                    (None, Some(d)) => Some(format!("Duration: {}", d)),
                    (None, None) => None,
                };
                (
                    "Called you with a notification".to_string(),
                    "fa-phone",
                    detail,
                )
            }
            "sms" => (
                "You sent an SMS".to_string(),
                "fa-message",
                log.reason.clone(),
            ),
            "call" => {
                let duration_str = log.call_duration.map(|d| {
                    if d < 60 {
                        format!("{}s", d)
                    } else {
                        format!("{}m {}s", d / 60, d % 60)
                    }
                });
                (
                    "You made a voice call".to_string(),
                    "fa-phone-volume",
                    duration_str.or(log.reason.clone()),
                )
            }
            "web_call" => {
                let duration_str = log.call_duration.map(|d| {
                    if d < 60 {
                        format!("{}s", d)
                    } else {
                        format!("{}m {}s", d / 60, d % 60)
                    }
                });
                (
                    "Web voice call".to_string(),
                    "fa-headset",
                    duration_str.or(log.reason.clone()),
                )
            }
            "web_chat" => (
                "You chatted via web dashboard".to_string(),
                "fa-comments",
                log.reason.clone(),
            ),
            "sms_test" => ("SMS test sent".to_string(), "fa-vial", log.reason.clone()),
            at if at.ends_with("_sms") => {
                let platform = humanize_platform(at.trim_end_matches("_sms"));
                (
                    format!("Notified you about a {} message", platform),
                    "fa-bell",
                    log.reason.clone(),
                )
            }
            at if at.ends_with("_call_conditional") => {
                let platform = humanize_platform(at.split('_').next().unwrap_or("unknown"));
                (
                    format!("Called you about a {} message", platform),
                    "fa-phone-flip",
                    log.reason.clone(),
                )
            }
            at if at.ends_with("_call") => {
                let platform = humanize_platform(at.trim_end_matches("_call"));
                (
                    format!("Called you about a {} message", platform),
                    "fa-phone-flip",
                    log.reason.clone(),
                )
            }
            _ => (
                format!("Activity: {}", log.activity_type),
                "fa-circle-dot",
                log.reason.clone(),
            ),
        };

        entries.push(ActivityFeedEntry {
            id: format!("usage-{}", log.id),
            entry_type: "notification".to_string(),
            timestamp: log.created_at,
            title,
            detail,
            icon: format!("fa-solid {}", icon),
            success: log.success,
        });
    }

    // Convert messages (only from known persons to reduce noise)
    for msg in messages.iter().filter(|m| m.person_id.is_some()).take(50) {
        let person_name = msg
            .person_id
            .and_then(|pid| person_names.get(&pid))
            .cloned()
            .unwrap_or_else(|| msg.sender_name.clone());

        let preview = if msg.content.len() > 120 {
            format!("{}...", &msg.content[..120])
        } else {
            msg.content.clone()
        };

        let platform_display = match msg.platform.as_str() {
            "whatsapp" => "WhatsApp",
            "telegram" => "Telegram",
            "signal" => "Signal",
            "email" => "email",
            other => other,
        };

        let icon = match msg.platform.as_str() {
            "whatsapp" => "fa-brands fa-whatsapp",
            "telegram" => "fa-brands fa-telegram",
            "signal" => "fa-solid fa-comment-dots",
            "email" => "fa-solid fa-envelope",
            _ => "fa-solid fa-message",
        };

        entries.push(ActivityFeedEntry {
            id: format!("message-{}", msg.id),
            entry_type: "message".to_string(),
            timestamp: msg.created_at,
            title: format!("{} via {}", person_name, platform_display),
            detail: Some(preview),
            icon: icon.to_string(),
            success: None,
        });
    }

    // Sort by timestamp descending, truncate
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    entries.truncate(limit as usize);

    Ok(Json(entries))
}

// -----------------------------------------------------------------------
// Rule Sources (available prefetch sources for rule builder)
// -----------------------------------------------------------------------

#[derive(Serialize)]
pub struct RuleSourceOption {
    pub source_type: String,
    pub label: String,
    pub available: bool,
    pub meta: serde_json::Value,
}

/// GET /api/dashboard/rule-sources
/// Returns available prefetch source types for the rule builder,
/// with availability based on user's connected services.
pub async fn get_rule_sources(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<RuleSourceOption>>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;
    let mut sources = Vec::new();

    // Email: available if user has IMAP credentials
    let has_email = state
        .user_repository
        .get_imap_credentials(user_id)
        .ok()
        .flatten()
        .is_some();
    sources.push(RuleSourceOption {
        source_type: "email".to_string(),
        label: "Email".to_string(),
        available: has_email,
        meta: serde_json::json!({}),
    });

    // Chat: always available. Include distinct platforms.
    let platforms: Vec<String> = state
        .ontology_repository
        .get_distinct_senders(user_id)
        .unwrap_or_default()
        .iter()
        .map(|(_, plat, _)| plat.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    sources.push(RuleSourceOption {
        source_type: "chat".to_string(),
        label: "Chat".to_string(),
        available: true,
        meta: serde_json::json!({ "platforms": platforms }),
    });

    // Weather: always available (user can type location manually)
    let user_info = state.user_core.get_user_info(user_id).ok();
    let location_name = user_info
        .as_ref()
        .and_then(|i| i.location.clone())
        .unwrap_or_default();
    sources.push(RuleSourceOption {
        source_type: "weather".to_string(),
        label: "Weather".to_string(),
        available: true,
        meta: serde_json::json!({ "location": location_name }),
    });

    // Internet: always available
    sources.push(RuleSourceOption {
        source_type: "internet".to_string(),
        label: "Internet".to_string(),
        available: true,
        meta: serde_json::json!({}),
    });

    // Tesla: available if user has Tesla connection
    let has_tesla = state
        .user_repository
        .get_tesla_token_info(user_id)
        .ok()
        .is_some();
    sources.push(RuleSourceOption {
        source_type: "tesla".to_string(),
        label: "Tesla".to_string(),
        available: has_tesla,
        meta: serde_json::json!({}),
    });

    // Events: available if user has tracked obligations
    let events_count = state
        .ontology_repository
        .get_active_events(user_id)
        .map(|e| e.len())
        .unwrap_or(0);
    sources.push(RuleSourceOption {
        source_type: "events".to_string(),
        label: "Tracked obligations".to_string(),
        available: true,
        meta: serde_json::json!({ "count": events_count }),
    });

    // MCP: available if user has enabled MCP servers
    let mcp_repo = crate::repositories::mcp_repository::McpRepository::new(state.pg_pool.clone());
    let mcp_servers = mcp_repo
        .get_enabled_servers_for_user(user_id)
        .unwrap_or_default();
    if !mcp_servers.is_empty() {
        let server_list: Vec<serde_json::Value> = mcp_servers
            .iter()
            .map(|s| serde_json::json!({ "id": s.id, "name": s.name }))
            .collect();
        sources.push(RuleSourceOption {
            source_type: "mcp".to_string(),
            label: "MCP".to_string(),
            available: true,
            meta: serde_json::json!({ "servers": server_list }),
        });
    } else {
        sources.push(RuleSourceOption {
            source_type: "mcp".to_string(),
            label: "MCP".to_string(),
            available: false,
            meta: serde_json::json!({ "servers": [] }),
        });
    }

    Ok(Json(sources))
}

// -----------------------------------------------------------------------
// Senders list (for rule builder autocomplete)
// -----------------------------------------------------------------------

#[derive(Serialize)]
pub struct SenderOption {
    pub name: String,
    pub platform: Option<String>, // None = person (matches all channels), Some = specific channel
    pub source: String,           // "person", "chat", "group"
    pub msg_count: Option<i64>,
    #[serde(default)]
    pub is_group: bool,
}

/// GET /api/dashboard/senders
/// Returns known senders for rule builder autocomplete.
/// Combines: persons (with their channels) + distinct senders from ont_messages.
pub async fn get_senders(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<SenderOption>>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;
    let mut options: Vec<SenderOption> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 1. Persons with channels (highest priority)
    let persons = state
        .ontology_repository
        .get_persons_with_channels(user_id, 500, 0)
        .unwrap_or_default();

    for person in &persons {
        let display = person.display_name().to_string();

        // Person-level entry (matches all channels)
        let key = format!("person:{}", display.to_lowercase());
        if seen.insert(key) {
            options.push(SenderOption {
                name: display.clone(),
                platform: None,
                source: "person".to_string(),
                msg_count: None,
                is_group: false,
            });
        }

        // Per-channel entries
        for ch in &person.channels {
            let key = format!("channel:{}:{}", display.to_lowercase(), ch.platform);
            if seen.insert(key) {
                options.push(SenderOption {
                    name: display.clone(),
                    platform: Some(ch.platform.clone()),
                    source: "person".to_string(),
                    msg_count: None,
                    is_group: false,
                });
            }
        }
    }

    // 2. Distinct senders from ont_messages (chat rooms not yet assigned to persons)
    let senders = state
        .ontology_repository
        .get_distinct_senders(user_id)
        .unwrap_or_default();

    for (sender_name, platform, count) in &senders {
        let key = format!("chat:{}:{}", sender_name.to_lowercase(), platform);
        if seen.insert(key) {
            options.push(SenderOption {
                name: sender_name.clone(),
                platform: Some(platform.clone()),
                source: "chat".to_string(),
                msg_count: Some(*count),
                is_group: false,
            });
        }
    }

    // 3. All bridge rooms (fills in chats not yet in ont_messages + identifies groups)
    let services = ["signal", "whatsapp", "telegram"];
    let matrix_clients = state.matrix_clients.lock().await;
    if let Some(client) = matrix_clients.get(&user_id) {
        for service in &services {
            let bridge_ok = state
                .user_repository
                .get_bridge(user_id, service)
                .ok()
                .flatten()
                .map(|b| b.status == "connected")
                .unwrap_or(false);
            if !bridge_ok {
                continue;
            }
            if let Ok(rooms) = crate::utils::bridge::get_service_rooms(client, service).await {
                for room in rooms {
                    let display = crate::utils::bridge::remove_bridge_suffix(&room.display_name);
                    if room.is_group {
                        let key = format!("group:{}:{}", display.to_lowercase(), service);
                        if seen.insert(key) {
                            options.push(SenderOption {
                                name: display,
                                platform: Some(service.to_string()),
                                source: "group".to_string(),
                                msg_count: None,
                                is_group: true,
                            });
                        }
                    } else {
                        // Non-group bridge room - add if not already present from ont_messages
                        let key = format!("chat:{}:{}", display.to_lowercase(), service);
                        if seen.insert(key) {
                            options.push(SenderOption {
                                name: display,
                                platform: Some(service.to_string()),
                                source: "chat".to_string(),
                                msg_count: None,
                                is_group: false,
                            });
                        }
                    }
                }
            }
        }
    }
    drop(matrix_clients);

    Ok(Json(options))
}

/// POST /api/events/{id}/dismiss
pub async fn dismiss_event(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(event_id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .ontology_repository
        .dismiss_event(auth_user.user_id, event_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to dismiss event: {}", e) })),
            )
        })?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /api/events/{id}
pub async fn get_event_detail(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(event_id): Path<i32>,
) -> Result<Json<EventDetailResponse>, (StatusCode, Json<serde_json::Value>)> {
    let event = state
        .ontology_repository
        .get_event(auth_user.user_id, event_id)
        .map_err(|e| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": format!("Failed to get event: {}", e) })),
            )
        })?;

    let linked_messages = state
        .ontology_repository
        .get_messages_for_event(auth_user.user_id, event_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to get linked messages: {}", e)
                })),
            )
        })?
        .into_iter()
        .map(|message| EventMessageItem {
            id: message.id,
            platform: message.platform,
            sender_name: message.sender_name,
            content: message.content,
            created_at: message.created_at,
            room_id: message.room_id,
        })
        .collect();

    Ok(Json(EventDetailResponse {
        event: EventItem {
            id: event.id,
            description: event.description,
            remind_at: event.remind_at,
            due_at: event.due_at,
            status: event.status,
            created_at: event.created_at,
        },
        linked_messages,
    }))
}

// -- Digest endpoints --

#[derive(Serialize)]
pub struct DigestMessageResponse {
    pub id: i64,
    pub sender_name: String,
    pub platform: String,
    pub content: String,
    pub urgency: Option<String>,
    pub category: Option<String>,
    pub summary: Option<String>,
    pub created_at: i32,
}

pub async fn get_pending_digest(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<DigestMessageResponse>>, (StatusCode, Json<serde_json::Value>)> {
    let messages = state
        .ontology_repository
        .get_pending_digest_messages(auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    let items: Vec<DigestMessageResponse> = messages
        .into_iter()
        .map(|m| DigestMessageResponse {
            id: m.id,
            sender_name: m.sender_name,
            platform: m.platform,
            content: m.content,
            urgency: m.urgency,
            category: m.category,
            summary: m.summary,
            created_at: m.created_at,
        })
        .collect();

    Ok(Json(items))
}

#[derive(Deserialize)]
pub struct MarkDigestReadRequest {
    pub message_ids: Option<Vec<i64>>,
}

pub async fn mark_digest_read(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<MarkDigestReadRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32;

    let ids = if let Some(ids) = request.message_ids {
        ids
    } else {
        // Mark all pending digest messages as read
        let pending = state
            .ontology_repository
            .get_pending_digest_messages(auth_user.user_id)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("Database error: {}", e)})),
                )
            })?;
        pending.into_iter().map(|m| m.id).collect()
    };

    if !ids.is_empty() {
        state
            .ontology_repository
            .mark_digest_delivered(&ids, now)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": format!("Database error: {}", e)})),
                )
            })?;
    }

    Ok(Json(serde_json::json!({"marked": ids.len()})))
}
