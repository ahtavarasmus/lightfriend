use axum::{
    extract::{Path, Query, State},
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
    pub upcoming_items: Vec<UpcomingItem>,
    pub upcoming_digests: Vec<UpcomingDigest>,
    pub watched_contacts: Vec<WatchedContact>,
    pub next_digest: Option<NextDigestInfo>,
    pub quiet_mode: QuietModeInfo,
    pub sunrise_hour: Option<f32>,
    pub sunset_hour: Option<f32>,
    /// Items beyond the current timeline range (for preview in extend button tooltip)
    pub items_beyond: Vec<UpcomingItem>,
    /// Total count of items beyond the current timeline range
    pub items_beyond_count: i32,
    /// Total number of tracked items (for status line display)
    pub total_tracked_count: i32,
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
    pub item_type: String, // "monitor", "tracked_item"
    pub summary: String,
    pub description: String,
    pub priority: i32,
    pub monitor: bool,
    pub next_check_at: Option<i32>,
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_display: Option<String>,
}

#[derive(Serialize)]
pub struct ScheduledItem {
    pub time_display: String, // "2:30pm"
    pub description: String,  // "Check on Mom"
    pub item_id: Option<i32>,
}

#[derive(Serialize, Clone)]
pub struct UpcomingItem {
    pub item_id: Option<i32>,
    pub timestamp: i32,           // Unix timestamp for positioning
    pub time_display: String,     // "2:30pm"
    pub description: String,      // "Check on Mom"
    pub date_display: String,     // "Feb 10"
    pub relative_display: String, // "in 5 days"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_type: Option<String>, // "oneshot", "tracking", "recurring"
    pub monitor: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>, // "call", "sms"
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
    pub item_id: Option<i32>,
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

    // Get all items for this user
    let items = state
        .item_repository
        .get_dashboard_items(user_id)
        .unwrap_or_default();

    // Split items into categories for display
    let total_tracked_count = items.len() as i32;
    let mut attention_items: Vec<AttentionItem> = Vec::new();
    for item in &items {
        let is_future_scheduled = item.next_check_at.is_some_and(|nca| nca > now_ts);
        // Items scheduled for the future (non-priority) go to upcoming_items/upcoming_digests
        if is_future_scheduled && item.priority == 0 {
            continue;
        }
        // Everything else is an attention item (monitors, high-priority, tracked items)
        let item_type = if item.monitor {
            "monitor"
        } else {
            "tracked_item"
        };

        // Parse tags and strip tag line from description
        let tags = crate::proactive::utils::parse_summary_tags(&item.summary);
        let description = item
            .summary
            .lines()
            .skip(if tags.has_tags { 1 } else { 0 })
            .collect::<Vec<_>>()
            .join("\n");

        // Format time/relative display from next_check_at
        let time_display = item.next_check_at.map(|nca| format_time_display(nca, &tz));
        let relative_display = item.next_check_at.map(|nca| {
            if nca <= now_ts {
                "overdue".to_string()
            } else {
                format_relative_days(nca, now_ts, &tz)
            }
        });

        attention_items.push(AttentionItem {
            id: item.id.unwrap_or(0),
            item_type: item_type.to_string(),
            summary: item.summary.clone(),
            description,
            priority: item.priority,
            monitor: item.monitor,
            next_check_at: item.next_check_at,
            source: item.source_id.clone(),
            source_id: item.source_id.clone(),
            notify: tags.notify,
            sender: tags.sender,
            platform: tags.platform,
            time_display,
            relative_display,
        });
    }

    // Sort by next_check_at (soonest first, items without it sort last)
    attention_items.sort_by(|a, b| match (a.next_check_at, b.next_check_at) {
        (Some(a_ts), Some(b_ts)) => a_ts.cmp(&b_ts),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    let attention_count = attention_items.len() as i32;

    // Find next scheduled item (soonest upcoming non-digest item)
    let next_scheduled = find_next_scheduled_item(&items, now_ts, &tz);

    // Find all upcoming items within the timeline range
    let upcoming_items = find_upcoming_items(&items, now_ts, max_ts, &tz);

    // Find all upcoming digests within the timeline range
    let upcoming_digests = find_upcoming_digest_items(&items, now_ts, max_ts, &tz);

    // Find items beyond the timeline range (for extend button)
    let (items_beyond, items_beyond_count) = find_items_beyond(&items, now_ts, max_ts, &tz);

    // Get watched contacts (contact profiles with notification modes)
    let watched_contacts = get_watched_contacts(&state, user_id);

    // Find next digest time
    let next_digest = find_next_digest_item(&items, now_ts, &tz);

    // Get quiet mode status
    let quiet_mode = get_quiet_mode_info(&state, user_id, now_ts, &tz);

    Ok(Json(DashboardSummaryResponse {
        attention_count,
        attention_items,
        next_scheduled,
        upcoming_items,
        upcoming_digests,
        watched_contacts,
        next_digest,
        quiet_mode,
        sunrise_hour,
        sunset_hour,
        items_beyond,
        items_beyond_count,
        total_tracked_count,
    }))
}

fn find_next_scheduled_item(
    items: &[crate::models::user_models::Item],
    now_ts: i32,
    tz: &chrono_tz::Tz,
) -> Option<ScheduledItem> {
    items
        .iter()
        .filter(|item| !item.monitor)
        .filter_map(|item| {
            item.next_check_at
                .filter(|&nca| nca > now_ts)
                .map(|nca| (item, nca))
        })
        .min_by_key(|(_, nca)| *nca)
        .map(|(item, nca)| ScheduledItem {
            time_display: format_time_display(nca, tz),
            description: item.summary.clone(),
            item_id: item.id,
        })
}

fn find_upcoming_items(
    items: &[crate::models::user_models::Item],
    now_ts: i32,
    max_ts: i32,
    tz: &chrono_tz::Tz,
) -> Vec<UpcomingItem> {
    let mut upcoming: Vec<UpcomingItem> = items
        .iter()
        .filter(|item| !item.monitor)
        .filter_map(|item| {
            item.next_check_at
                .filter(|&nca| nca > now_ts && nca <= max_ts)
                .map(|nca| {
                    let tags = crate::proactive::utils::parse_summary_tags(&item.summary);
                    let description = item
                        .summary
                        .lines()
                        .skip(if tags.has_tags { 1 } else { 0 })
                        .collect::<Vec<_>>()
                        .join("\n");
                    let sources_display = if !tags.fetch.is_empty() {
                        Some(tags.fetch.join(", "))
                    } else {
                        None
                    };
                    UpcomingItem {
                        item_id: item.id,
                        timestamp: nca,
                        time_display: format_time_display(nca, tz),
                        description,
                        date_display: format_date_display(nca, tz),
                        relative_display: format_relative_days(nca, now_ts, tz),
                        item_type: tags.item_type,
                        monitor: item.monitor,
                        notify: tags.notify,
                        sources_display,
                    }
                })
        })
        .collect();

    upcoming.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    upcoming
}

fn find_upcoming_digest_items(
    items: &[crate::models::user_models::Item],
    now_ts: i32,
    max_ts: i32,
    tz: &chrono_tz::Tz,
) -> Vec<UpcomingDigest> {
    let mut digests: Vec<UpcomingDigest> = items
        .iter()
        .filter(|item| !item.monitor && item.summary.starts_with("Daily digest"))
        .filter_map(|item| {
            item.next_check_at
                .filter(|&nca| nca > now_ts && nca <= max_ts)
                .map(|nca| UpcomingDigest {
                    item_id: item.id,
                    timestamp: nca,
                    time_display: format_time_display(nca, tz),
                    sources: None,
                })
        })
        .collect();

    digests.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    digests
}

/// Find items beyond the current timeline range (for the extend button)
/// Returns up to 5 items for preview and the total count
fn find_items_beyond(
    items: &[crate::models::user_models::Item],
    now_ts: i32,
    max_ts: i32,
    tz: &chrono_tz::Tz,
) -> (Vec<UpcomingItem>, i32) {
    let ninety_days = 90 * 24 * 60 * 60;
    let lookahead_ts = max_ts + ninety_days;

    let mut beyond: Vec<UpcomingItem> = items
        .iter()
        .filter(|item| !item.monitor)
        .filter_map(|item| {
            item.next_check_at
                .filter(|&nca| nca > max_ts && nca <= lookahead_ts)
                .map(|nca| {
                    let tags = crate::proactive::utils::parse_summary_tags(&item.summary);
                    let description = item
                        .summary
                        .lines()
                        .skip(if tags.has_tags { 1 } else { 0 })
                        .collect::<Vec<_>>()
                        .join("\n");
                    let sources_display = if !tags.fetch.is_empty() {
                        Some(tags.fetch.join(", "))
                    } else {
                        None
                    };
                    UpcomingItem {
                        item_id: item.id,
                        timestamp: nca,
                        time_display: format_time_display(nca, tz),
                        description,
                        date_display: format_date_display(nca, tz),
                        relative_display: format_relative_days(nca, now_ts, tz),
                        item_type: tags.item_type,
                        monitor: item.monitor,
                        notify: tags.notify,
                        sources_display,
                    }
                })
        })
        .collect();

    beyond.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    let total_count = beyond.len() as i32;
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

fn find_next_digest_item(
    items: &[crate::models::user_models::Item],
    now_ts: i32,
    tz: &chrono_tz::Tz,
) -> Option<NextDigestInfo> {
    let earliest_nca = items
        .iter()
        .filter(|item| !item.monitor && item.summary.starts_with("Daily digest"))
        .filter_map(|item| item.next_check_at.filter(|&nca| nca > now_ts))
        .min()?;

    let time_display = format_relative_time(earliest_nca, now_ts, tz);
    Some(NextDigestInfo { time_display })
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

// Item API types

#[derive(Serialize)]
pub struct ItemResponse {
    pub id: i32,
    pub summary: String,
    pub monitor: bool,
    pub next_check_at: Option<i32>,
    pub priority: i32,
    pub source_id: Option<String>,
    pub created_at: i32,
}

#[derive(Serialize)]
pub struct ItemListResponse {
    pub items: Vec<ItemResponse>,
    pub count: i32,
}

#[derive(Deserialize)]
pub struct SnoozeRequest {
    pub minutes: Option<i32>, // default 60
}

fn item_to_response(item: crate::models::user_models::Item) -> ItemResponse {
    ItemResponse {
        id: item.id.unwrap_or(0),
        summary: item.summary,
        monitor: item.monitor,
        next_check_at: item.next_check_at,
        priority: item.priority,
        source_id: item.source_id,
        created_at: item.created_at,
    }
}

/// GET /api/items
/// Returns all items for the authenticated user.
pub async fn get_items(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ItemListResponse>, (StatusCode, Json<serde_json::Value>)> {
    let items = state
        .item_repository
        .get_items(auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to get items: {}", e)})),
            )
        })?;

    let count = items.len() as i32;
    let responses: Vec<ItemResponse> = items.into_iter().map(item_to_response).collect();

    Ok(Json(ItemListResponse {
        items: responses,
        count,
    }))
}

/// GET /api/items/{id}
/// Returns a single item with formatted display fields for preview.
pub async fn get_item_detail(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let item = state
        .item_repository
        .get_item(id, auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("DB error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Item not found"})),
            )
        })?;

    let user_info = state.user_core.get_user_info(auth_user.user_id).ok();
    let tz: chrono_tz::Tz = user_info
        .as_ref()
        .and_then(|info| info.timezone.clone())
        .unwrap_or_else(|| "UTC".to_string())
        .parse()
        .unwrap_or(chrono_tz::UTC);
    let now_ts = chrono::Utc::now().timestamp() as i32;

    let trigger_ts = item.next_check_at.unwrap_or(item.created_at);

    // Parse structured tags from summary and extract clean description
    let tags = crate::proactive::utils::parse_summary_tags(&item.summary);
    let description = item
        .summary
        .lines()
        .skip(if tags.has_tags { 1 } else { 0 })
        .collect::<Vec<_>>()
        .join("\n");

    // Build sources_display from [fetch:...] tags (e.g. "email, calendar")
    let sources_display = if !tags.fetch.is_empty() {
        Some(tags.fetch.join(", "))
    } else {
        None
    };

    Ok(Json(serde_json::json!({
        "id": item.id.unwrap_or(0),
        "trigger_timestamp": trigger_ts,
        "time_display": format_time_display(trigger_ts, &tz),
        "date_display": format_date_display(trigger_ts, &tz),
        "relative_display": format_relative_days(trigger_ts, now_ts, &tz),
        "description": description,
        "item_type": tags.item_type,
        "monitor": item.monitor,
        "notify": tags.notify,
        "sources_display": sources_display,
    })))
}

/// POST /api/items/{id}/snooze
/// Snoozes an item by setting next_check_at to a future time.
pub async fn snooze_item(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(id): Path<i32>,
    body: Option<Json<SnoozeRequest>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Verify ownership
    let item = state
        .item_repository
        .get_item(id, auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("DB error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Item not found"})),
            )
        })?;

    let minutes = body.as_ref().and_then(|b| b.minutes).unwrap_or(60);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    let snooze_until = now + (minutes * 60);

    state
        .item_repository
        .update_next_check_at(item.id.unwrap_or(0), Some(snooze_until))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to snooze: {}", e)})),
            )
        })?;

    Ok(Json(
        serde_json::json!({"success": true, "snooze_until": snooze_until}),
    ))
}

/// DELETE /api/items/{id}
/// Dismisses an item by deleting it.
pub async fn dismiss_item(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = state
        .item_repository
        .delete_item(id, auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to dismiss: {}", e)})),
            )
        })?;

    if deleted {
        Ok(Json(serde_json::json!({"success": true})))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Item not found"})),
        ))
    }
}
