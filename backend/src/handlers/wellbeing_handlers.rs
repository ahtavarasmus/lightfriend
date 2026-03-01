use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{handlers::auth_middleware::AuthUser, models::user_models::NewDailyCheckin, AppState};

// --- Request/Response types ---

#[derive(Serialize)]
pub struct DumbphoneResponse {
    pub on: bool,
}

#[derive(Deserialize)]
pub struct DumbphoneRequest {
    pub on: bool,
}

#[derive(Serialize)]
pub struct CheckinResponse {
    pub id: Option<i32>,
    pub checkin_date: String,
    pub mood: i32,
    pub energy: i32,
    pub sleep_quality: i32,
}

#[derive(Deserialize)]
pub struct CheckinRequest {
    pub mood: i32,
    pub energy: i32,
    pub sleep_quality: i32,
}

#[derive(Serialize)]
pub struct CalmerResponse {
    pub on: bool,
    pub schedule: Option<String>,
}

#[derive(Deserialize)]
pub struct CalmerRequest {
    pub on: bool,
    pub schedule: Option<String>,
}

#[derive(Serialize)]
pub struct PointsResponse {
    pub points: i32,
    pub current_streak: i32,
    pub longest_streak: i32,
    pub recent_events: Vec<PointEventResponse>,
}

#[derive(Serialize)]
pub struct PointEventResponse {
    pub event_type: String,
    pub points_earned: i32,
    pub event_date: String,
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub days_active: i32,
    pub hours_saved: f32,
    pub notifications_reduced: i32,
}

fn today_str() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn internal_err(msg: String) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": msg})),
    )
}

// --- Dumbphone Mode ---

pub async fn get_dumbphone(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> ApiResult<DumbphoneResponse> {
    let on = state
        .wellbeing_repository
        .get_dumbphone_mode(auth_user.user_id)
        .map_err(|e| internal_err(format!("Failed to get dumbphone mode: {}", e)))?;
    Ok(Json(DumbphoneResponse { on }))
}

pub async fn set_dumbphone(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<DumbphoneRequest>,
) -> ApiResult<DumbphoneResponse> {
    state
        .wellbeing_repository
        .set_dumbphone_mode(auth_user.user_id, req.on)
        .map_err(|e| internal_err(format!("Failed to set dumbphone mode: {}", e)))?;

    // Award points if turning on
    if req.on {
        let today = today_str();
        let _ =
            state
                .wellbeing_repository
                .award_points(auth_user.user_id, "dumbphone_on", 5, &today);
    }

    Ok(Json(DumbphoneResponse { on: req.on }))
}

// --- Daily Check-in ---

pub async fn get_today_checkin(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> ApiResult<serde_json::Value> {
    let today = today_str();
    let checkin = state
        .wellbeing_repository
        .get_checkin_for_date(auth_user.user_id, &today)
        .map_err(|e| internal_err(format!("Failed to get checkin: {}", e)))?;

    match checkin {
        Some(c) => Ok(Json(serde_json::json!({
            "has_checkin": true,
            "checkin": {
                "id": c.id,
                "checkin_date": c.checkin_date,
                "mood": c.mood,
                "energy": c.energy,
                "sleep_quality": c.sleep_quality,
            }
        }))),
        None => Ok(Json(serde_json::json!({
            "has_checkin": false,
            "checkin": null
        }))),
    }
}

pub async fn create_checkin(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<CheckinRequest>,
) -> ApiResult<CheckinResponse> {
    // Validate ranges
    if !(1..=5).contains(&req.mood)
        || !(1..=5).contains(&req.energy)
        || !(1..=5).contains(&req.sleep_quality)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Values must be between 1 and 5"})),
        ));
    }

    let today = today_str();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let new = NewDailyCheckin {
        user_id: auth_user.user_id,
        checkin_date: today.clone(),
        mood: req.mood,
        energy: req.energy,
        sleep_quality: req.sleep_quality,
        created_at: now,
    };

    let checkin = state
        .wellbeing_repository
        .upsert_checkin(&new)
        .map_err(|e| internal_err(format!("Failed to save checkin: {}", e)))?;

    // Award points
    let _ = state
        .wellbeing_repository
        .award_points(auth_user.user_id, "checkin", 10, &today);

    // Ensure wellbeing signup timestamp
    let _ = state
        .wellbeing_repository
        .ensure_wellbeing_signup_timestamp(auth_user.user_id, now);

    Ok(Json(CheckinResponse {
        id: checkin.id,
        checkin_date: checkin.checkin_date,
        mood: checkin.mood,
        energy: checkin.energy,
        sleep_quality: checkin.sleep_quality,
    }))
}

pub async fn get_checkin_history(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> ApiResult<Vec<CheckinResponse>> {
    let checkins = state
        .wellbeing_repository
        .get_checkin_history(auth_user.user_id, 7)
        .map_err(|e| internal_err(format!("Failed to get history: {}", e)))?;

    Ok(Json(
        checkins
            .into_iter()
            .map(|c| CheckinResponse {
                id: c.id,
                checkin_date: c.checkin_date,
                mood: c.mood,
                energy: c.energy,
                sleep_quality: c.sleep_quality,
            })
            .collect(),
    ))
}

// --- Notification Calmer ---

pub async fn get_calmer(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> ApiResult<CalmerResponse> {
    let (on, schedule) = state
        .wellbeing_repository
        .get_notification_calmer(auth_user.user_id)
        .map_err(|e| internal_err(format!("Failed to get calmer: {}", e)))?;
    Ok(Json(CalmerResponse { on, schedule }))
}

pub async fn set_calmer(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<CalmerRequest>,
) -> ApiResult<CalmerResponse> {
    state
        .wellbeing_repository
        .set_notification_calmer(auth_user.user_id, req.on, req.schedule.clone())
        .map_err(|e| internal_err(format!("Failed to set calmer: {}", e)))?;

    // Award points if turning on
    if req.on {
        let today = today_str();
        let _ = state
            .wellbeing_repository
            .award_points(auth_user.user_id, "calmer_on", 5, &today);
    }

    Ok(Json(CalmerResponse {
        on: req.on,
        schedule: req.schedule,
    }))
}

// --- Wellbeing Points ---

pub async fn get_points(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> ApiResult<PointsResponse> {
    let points = state
        .wellbeing_repository
        .get_or_create_points(auth_user.user_id)
        .map_err(|e| internal_err(format!("Failed to get points: {}", e)))?;

    let events = state
        .wellbeing_repository
        .get_recent_events(auth_user.user_id, 10)
        .map_err(|e| internal_err(format!("Failed to get events: {}", e)))?;

    Ok(Json(PointsResponse {
        points: points.points,
        current_streak: points.current_streak,
        longest_streak: points.longest_streak,
        recent_events: events
            .into_iter()
            .map(|e| PointEventResponse {
                event_type: e.event_type,
                points_earned: e.points_earned,
                event_date: e.event_date,
            })
            .collect(),
    }))
}

// --- Wellbeing Stats ---

pub async fn get_stats(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> ApiResult<StatsResponse> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let signup_ts = state
        .wellbeing_repository
        .get_wellbeing_signup_timestamp(auth_user.user_id)
        .map_err(|e| internal_err(format!("Failed to get stats: {}", e)))?
        .unwrap_or(now);

    let days_active = ((now - signup_ts) / 86400).max(0);

    // Estimate hours saved: ~0.5 hours per day with wellbeing features active
    let hours_saved = days_active as f32 * 0.5;

    // Estimate notifications reduced based on calmer usage
    let (calmer_on, _) = state
        .wellbeing_repository
        .get_notification_calmer(auth_user.user_id)
        .unwrap_or((false, None));
    let notifications_reduced = if calmer_on {
        days_active * 12 // ~12 notifications reduced per day
    } else {
        0
    };

    Ok(Json(StatsResponse {
        days_active,
        hours_saved,
        notifications_reduced,
    }))
}
