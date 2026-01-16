use std::sync::Arc;
use axum::{
    Json,
    extract::{State, Query},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use diesel::prelude::*;
use diesel::dsl::{count, sum};

use crate::AppState;
use crate::schema::{message_status_log, usage_logs, users};

#[derive(Deserialize)]
pub struct StatsQuery {
    pub days: Option<i32>,
}

// Cost Stats Response
#[derive(Serialize)]
pub struct CostStatsResponse {
    // Key metrics - averages per user
    pub avg_cost_per_intl_user_30d: f32,
    pub avg_cost_per_us_ca_user_30d: f32,
    pub avg_cost_per_intl_user_7d_projected: f32,
    pub avg_cost_per_us_ca_user_7d_projected: f32,
    // Counts
    pub intl_user_count: i64,
    pub us_ca_user_count: i64,
    // Totals (for details section)
    pub total_cost: f32,
    pub total_sms_cost: f32,
    pub total_voice_cost: f32,
    pub international_sms_cost: f32,
    pub us_ca_sms_cost: f32,
    // Per-user costs for the graph (sorted by cost desc)
    pub costs_per_user: Vec<UserCostEntry>,
}

#[derive(Serialize)]
pub struct UserCostEntry {
    pub user_id: i32,
    pub country_code: String,
    pub sms_cost: f32,
    pub sms_count: i64,
    pub is_international: bool,
}

// Usage Stats Response
#[derive(Serialize)]
pub struct UsageStatsResponse {
    pub daily_stats: Vec<DailyUsageStat>,
    pub total_messages_7d: i64,
    pub total_messages_30d: i64,
    pub growth_rate_7d: f32,
    pub growth_rate_30d: f32,
    pub active_users_7d: i64,
    pub active_users_30d: i64,
    pub breakdown_by_type: Vec<ActivityTypeBreakdown>,
}

#[derive(Serialize)]
pub struct DailyUsageStat {
    pub date: String,
    pub sms_count: i64,
    pub call_count: i64,
    pub total_cost: f32,
}

#[derive(Serialize)]
pub struct ActivityTypeBreakdown {
    pub activity_type: String,
    pub count: i64,
    pub total_credits: f32,
}

/// Get cost statistics
/// GET /api/admin/stats/costs
pub async fn get_cost_stats(
    State(state): State<Arc<AppState>>,
    Query(_params): Query<StatsQuery>,
) -> Result<Json<CostStatsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let now = chrono::Utc::now().timestamp() as i32;
    let from_30d = now - (30 * 86400);
    let from_7d = now - (7 * 86400);

    let conn = &mut state.db_pool.get().map_err(|e| {
        tracing::error!("Failed to get DB connection: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database connection error"})))
    })?;

    // Get all SMS data for last 30 days with user_id and price
    let sms_data_30d: Vec<(i32, Option<f32>, i32)> = message_status_log::table
        .filter(message_status_log::created_at.ge(from_30d))
        .select((message_status_log::user_id, message_status_log::price, message_status_log::created_at))
        .load(conn)
        .map_err(|e| {
            tracing::error!("Failed to get SMS stats: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to get SMS stats"})))
        })?;

    // Twilio prices are negative (money out), so use .abs()
    let total_sms_cost: f32 = sms_data_30d.iter().filter_map(|(_, p, _)| p.map(|v| v.abs())).sum();

    // Get voice costs from usage_logs (last 30 days)
    let total_voice_cost: f32 = usage_logs::table
        .filter(usage_logs::created_at.ge(from_30d))
        .filter(usage_logs::activity_type.eq("call"))
        .select(usage_logs::credits)
        .load::<Option<f32>>(conn)
        .unwrap_or_default()
        .iter()
        .filter_map(|p| *p)
        .sum();

    // Get user countries and plan types (to exclude BYOT users)
    let user_ids: Vec<i32> = sms_data_30d.iter().map(|(uid, _, _)| *uid).collect();
    let user_data: Vec<(i32, Option<String>, Option<String>)> = users::table
        .filter(users::id.eq_any(&user_ids))
        .select((users::id, users::phone_number_country, users::plan_type))
        .load(conn)
        .unwrap_or_default();

    // Build maps for country and identify BYOT users to exclude
    let mut country_map: std::collections::HashMap<i32, String> = std::collections::HashMap::new();
    let mut byot_users: std::collections::HashSet<i32> = std::collections::HashSet::new();

    for (id, country, plan_type) in user_data {
        country_map.insert(id, country.unwrap_or_else(|| "Unknown".to_string()));
        if plan_type.as_deref() == Some("byot") {
            byot_users.insert(id);
        }
    }

    // Aggregate costs per user for 30 days (use .abs() for Twilio's negative prices)
    let mut user_costs_30d: std::collections::HashMap<i32, (f32, i64)> = std::collections::HashMap::new();
    for (user_id, price, _) in &sms_data_30d {
        let entry = user_costs_30d.entry(*user_id).or_insert((0.0, 0));
        entry.0 += price.unwrap_or(0.0).abs();
        entry.1 += 1;
    }

    // Aggregate costs per user for 7 days only
    let mut user_costs_7d: std::collections::HashMap<i32, f32> = std::collections::HashMap::new();
    for (user_id, price, created_at) in &sms_data_30d {
        if *created_at >= from_7d {
            let entry = user_costs_7d.entry(*user_id).or_insert(0.0);
            *entry += price.unwrap_or(0.0).abs();
        }
    }

    // Calculate user stats (excluding BYOT users)
    let mut intl_total_cost_30d: f32 = 0.0;
    let mut intl_total_cost_7d: f32 = 0.0;
    let mut us_ca_total_cost_30d: f32 = 0.0;
    let mut us_ca_total_cost_7d: f32 = 0.0;
    let mut intl_users: std::collections::HashSet<i32> = std::collections::HashSet::new();
    let mut us_ca_users: std::collections::HashSet<i32> = std::collections::HashSet::new();

    for (user_id, (cost, _)) in &user_costs_30d {
        // Skip BYOT users - they pay Twilio directly
        if byot_users.contains(user_id) {
            continue;
        }
        let country = country_map.get(user_id).cloned().unwrap_or_else(|| "Unknown".to_string());
        let is_intl = country != "US" && country != "CA";
        if is_intl {
            intl_total_cost_30d += cost;
            intl_users.insert(*user_id);
        } else {
            us_ca_total_cost_30d += cost;
            us_ca_users.insert(*user_id);
        }
    }

    for (user_id, cost) in &user_costs_7d {
        // Skip BYOT users
        if byot_users.contains(user_id) {
            continue;
        }
        let country = country_map.get(user_id).cloned().unwrap_or_else(|| "Unknown".to_string());
        let is_intl = country != "US" && country != "CA";
        if is_intl {
            intl_total_cost_7d += cost;
        } else {
            us_ca_total_cost_7d += cost;
        }
    }

    let intl_user_count = intl_users.len() as i64;
    let us_ca_user_count = us_ca_users.len() as i64;

    // Key metrics: average cost per user (30d)
    let avg_cost_per_intl_user_30d = if intl_user_count > 0 {
        intl_total_cost_30d / intl_user_count as f32
    } else {
        0.0
    };

    let avg_cost_per_us_ca_user_30d = if us_ca_user_count > 0 {
        us_ca_total_cost_30d / us_ca_user_count as f32
    } else {
        0.0
    };

    // 7-day cost projected to 30 days (multiply by 30/7)
    let avg_cost_per_intl_user_7d_projected = if intl_user_count > 0 {
        (intl_total_cost_7d / intl_user_count as f32) * (30.0 / 7.0)
    } else {
        0.0
    };

    let avg_cost_per_us_ca_user_7d_projected = if us_ca_user_count > 0 {
        (us_ca_total_cost_7d / us_ca_user_count as f32) * (30.0 / 7.0)
    } else {
        0.0
    };

    // Build per-user cost list for graph (excluding BYOT users)
    let mut costs_per_user: Vec<UserCostEntry> = user_costs_30d
        .into_iter()
        .filter(|(user_id, _)| !byot_users.contains(user_id))
        .map(|(user_id, (sms_cost, sms_count))| {
            let country = country_map.get(&user_id).cloned().unwrap_or_else(|| "Unknown".to_string());
            let is_international = country != "US" && country != "CA";
            UserCostEntry {
                user_id,
                country_code: country,
                sms_cost,
                sms_count,
                is_international,
            }
        })
        .collect();

    // Sort by cost descending
    costs_per_user.sort_by(|a, b| b.sms_cost.partial_cmp(&a.sms_cost).unwrap_or(std::cmp::Ordering::Equal));

    Ok(Json(CostStatsResponse {
        avg_cost_per_intl_user_30d,
        avg_cost_per_us_ca_user_30d,
        avg_cost_per_intl_user_7d_projected,
        avg_cost_per_us_ca_user_7d_projected,
        intl_user_count,
        us_ca_user_count,
        total_cost: total_sms_cost + total_voice_cost,
        total_sms_cost,
        total_voice_cost,
        international_sms_cost: intl_total_cost_30d,
        us_ca_sms_cost: us_ca_total_cost_30d,
        costs_per_user,
    }))
}

/// Get usage statistics
/// GET /api/admin/stats/usage
pub async fn get_usage_stats(
    State(state): State<Arc<AppState>>,
    Query(params): Query<StatsQuery>,
) -> Result<Json<UsageStatsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let days = params.days.unwrap_or(30);
    let now = chrono::Utc::now().timestamp() as i32;
    let from_timestamp = now - (days * 86400);

    let conn = &mut state.db_pool.get().map_err(|e| {
        tracing::error!("Failed to get DB connection: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database connection error"})))
    })?;

    // Get daily SMS stats
    let sms_records: Vec<(i32, Option<f32>)> = message_status_log::table
        .filter(message_status_log::created_at.ge(from_timestamp))
        .select((message_status_log::created_at, message_status_log::price))
        .load(conn)
        .map_err(|e| {
            tracing::error!("Failed to get SMS records: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to get SMS records"})))
        })?;

    // Get daily call stats
    let call_records: Vec<i32> = usage_logs::table
        .filter(usage_logs::created_at.ge(from_timestamp))
        .filter(usage_logs::activity_type.eq("call"))
        .select(usage_logs::created_at)
        .load(conn)
        .map_err(|e| {
            tracing::error!("Failed to get call records: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to get call records"})))
        })?;

    // Aggregate by day (use .abs() for Twilio's negative prices)
    let mut daily_sms: std::collections::HashMap<i32, (i64, f32)> = std::collections::HashMap::new();
    for (created_at, price) in &sms_records {
        let day = (created_at / 86400) * 86400;
        let entry = daily_sms.entry(day).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += price.unwrap_or(0.0).abs();
    }

    let mut daily_calls: std::collections::HashMap<i32, i64> = std::collections::HashMap::new();
    for created_at in &call_records {
        let day = (created_at / 86400) * 86400;
        *daily_calls.entry(day).or_insert(0) += 1;
    }

    // Build daily stats
    let mut all_days: std::collections::HashSet<i32> = daily_sms.keys().cloned().collect();
    all_days.extend(daily_calls.keys().cloned());

    let mut daily_stats: Vec<DailyUsageStat> = all_days
        .into_iter()
        .map(|day| {
            let (sms_count, total_cost) = daily_sms.get(&day).cloned().unwrap_or((0, 0.0));
            let call_count = daily_calls.get(&day).cloned().unwrap_or(0);
            let date = chrono::DateTime::from_timestamp(day as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            DailyUsageStat {
                date,
                sms_count,
                call_count,
                total_cost,
            }
        })
        .collect();

    daily_stats.sort_by(|a, b| a.date.cmp(&b.date));

    // Calculate 7d and 30d totals
    let seven_days_ago = now - (7 * 86400);
    let thirty_days_ago = now - (30 * 86400);
    let fourteen_days_ago = now - (14 * 86400);
    let sixty_days_ago = now - (60 * 86400);

    let total_messages_7d = sms_records.iter().filter(|(ts, _)| *ts >= seven_days_ago).count() as i64;
    let total_messages_30d = sms_records.iter().filter(|(ts, _)| *ts >= thirty_days_ago).count() as i64;

    // For growth rate, compare to previous period
    let prev_7d_count = sms_records.iter().filter(|(ts, _)| *ts >= fourteen_days_ago && *ts < seven_days_ago).count() as i64;
    let prev_30d_count = sms_records.iter().filter(|(ts, _)| *ts >= sixty_days_ago && *ts < thirty_days_ago).count() as i64;

    let growth_rate_7d = if prev_7d_count > 0 {
        ((total_messages_7d as f32 - prev_7d_count as f32) / prev_7d_count as f32) * 100.0
    } else if total_messages_7d > 0 {
        100.0
    } else {
        0.0
    };

    let growth_rate_30d = if prev_30d_count > 0 {
        ((total_messages_30d as f32 - prev_30d_count as f32) / prev_30d_count as f32) * 100.0
    } else if total_messages_30d > 0 {
        100.0
    } else {
        0.0
    };

    // Active users
    let active_user_ids_7d: std::collections::HashSet<i32> = usage_logs::table
        .filter(usage_logs::created_at.ge(seven_days_ago))
        .select(usage_logs::user_id)
        .load::<i32>(conn)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let active_user_ids_30d: std::collections::HashSet<i32> = usage_logs::table
        .filter(usage_logs::created_at.ge(thirty_days_ago))
        .select(usage_logs::user_id)
        .load::<i32>(conn)
        .unwrap_or_default()
        .into_iter()
        .collect();

    let active_users_7d = active_user_ids_7d.len() as i64;
    let active_users_30d = active_user_ids_30d.len() as i64;

    // Breakdown by activity type
    let activity_breakdown: Vec<(String, i64, Option<f32>)> = usage_logs::table
        .filter(usage_logs::created_at.ge(from_timestamp))
        .group_by(usage_logs::activity_type)
        .select((
            usage_logs::activity_type,
            count(usage_logs::id),
            sum(usage_logs::credits),
        ))
        .load(conn)
        .unwrap_or_default();

    let breakdown_by_type: Vec<ActivityTypeBreakdown> = activity_breakdown
        .into_iter()
        .map(|(activity_type, cnt, total_credits)| ActivityTypeBreakdown {
            activity_type,
            count: cnt,
            total_credits: total_credits.unwrap_or(0.0),
        })
        .collect();

    Ok(Json(UsageStatsResponse {
        daily_stats,
        total_messages_7d,
        total_messages_30d,
        growth_rate_7d,
        growth_rate_30d,
        active_users_7d,
        active_users_30d,
        breakdown_by_type,
    }))
}
