use std::sync::Arc;
use diesel::result::Error as DieselError;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct TimezoneUpdateRequest {
    timezone: String,
}
use axum::extract::Path;
use serde_json::json;

use crate::AppState;


#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    email: String,
    phone_number: String,
    nickname: String,
    info: String,
    timezone: String,
    timezone_auto: bool,
}

#[derive(Serialize)]
pub struct SubscriptionInfo {
    id: String,
    status: String,
    next_bill_date: i32,
    stage: String,
    is_scheduled_to_cancel: Option<bool>,
}

#[derive(Serialize)]
pub struct ProfileResponse {
    id: i32,
    email: String,
    phone_number: String,
    nickname: Option<String>,
    verified: bool,
    time_to_live: i32,
    time_to_delete: bool,
    credits: f32,
    notify: bool,
    info: Option<String>,
    preferred_number: Option<String>,
    charge_when_under: bool,
    charge_back_to: Option<f32>,
    stripe_payment_method_id: Option<String>,
    timezone: Option<String>,
    timezone_auto: Option<bool>,
    sub_tier: Option<String>,
    msgs_left: i32,
    credits_left: f32,
    discount: bool,
}

use crate::handlers::auth_middleware::AuthUser;


pub async fn get_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ProfileResponse>, (StatusCode, Json<serde_json::Value>)> {

    // Get user profile from database
    let user = state.user_repository.find_by_id(auth_user.user_id).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    match user {
        Some(user) => {
            let current_time = std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap()
                                                    .as_secs() as i32;

            let ttl = user.time_to_live.unwrap_or(0);
            let time_to_delete = current_time > ttl;

            Ok(Json(ProfileResponse {
                id: user.id,
                email: user.email,
                phone_number: user.phone_number,
                nickname: user.nickname,
                verified: user.verified,
                time_to_live: ttl,
                time_to_delete: time_to_delete,
                credits: user.credits,
                notify: user.notify,
                info: user.info,
                preferred_number: user.preferred_number,
                charge_when_under: user.charge_when_under,
                charge_back_to: user.charge_back_to,
                stripe_payment_method_id: user.stripe_payment_method_id,
                timezone: user.timezone,
                timezone_auto: user.timezone_auto,
                sub_tier: user.sub_tier,
                msgs_left: user.msgs_left,
                credits_left: user.credits_left,
                discount: user.discount,
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        )),
    }
}


#[derive(Deserialize)]
pub struct NotifyCreditsRequest {
    notify: bool,
}

#[derive(Deserialize)]
pub struct PreferredNumberRequest {
    preferred_number: String,
}

pub async fn update_preferred_number(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<PreferredNumberRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    let allowed_numbers = vec![
        std::env::var("USA_PHONE").expect("USA_PHONE must be set in environment"),
        std::env::var("FIN_PHONE").expect("FIN_PHONE must be set in environment"),
        std::env::var("AUS_PHONE").expect("AUS_PHONE must be set in environment"),
        std::env::var("GB_PHONE").expect("GB_PHONE must be set in environment"),
    ];

    if !allowed_numbers.contains(&request.preferred_number) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid preferred number"}))
        ));
    }

    // Update preferred number
    state.user_repository.update_preferred_number(auth_user.user_id, &request.preferred_number)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    println!("Updated preferred number to: {}", request.preferred_number);
    Ok(Json(json!({
        "message": "Preferred number updated successfully"
    })))
}



pub async fn update_notify(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(user_id): Path<i32>,
    Json(request): Json<NotifyCreditsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Check if user is modifying their own settings or is an admin
    if auth_user.user_id != user_id && !auth_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "You can only modify your own settings unless you're an admin"}))
        ));
    }

    // Update notify preference
    state.user_repository.update_notify(user_id, request.notify)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    Ok(Json(json!({
        "message": "Notification preference updated successfully"
    })))
}

pub async fn update_timezone(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<TimezoneUpdateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    match state.user_repository.update_timezone(
        auth_user.user_id,
        &request.timezone,
    ) {
        Ok(_) => Ok(Json(json!({
            "message": "Timezone updated successfully"
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        )),
    }
}

pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(update_req): Json<UpdateProfileRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("here");
 
    use regex::Regex;
    let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    if !email_regex.is_match(&update_req.email) {
        println!("Invalid email format: {}", update_req.email);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid email format"}))
        ));
    }
    
    let phone_regex = Regex::new(r"^\+[1-9]\d{1,14}$").unwrap();
    if !phone_regex.is_match(&update_req.phone_number) {
        println!("Invalid phone number format: {}", update_req.phone_number);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Phone number must be in E.164 format (e.g., +1234567890)"}))
        ));
    }

    match state.user_repository.update_profile(
        auth_user.user_id,
        &update_req.email,
        &update_req.phone_number,
        &update_req.nickname,
        &update_req.info,
        &update_req.timezone,
        &update_req.timezone_auto,
    ) {
        Ok(_) => (),
        Err(DieselError::RollbackTransaction) => {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({"error": "Phone number already exists"}))
            ));
        }
        Err(DieselError::NotFound) => {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({"error": "Email already exists"}))
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ));
        }
    }

    Ok(Json(json!({
        "message": "Profile updated successfully"
    })))
}

#[derive(Deserialize)]
pub struct ImapGeneralChecksRequest {
    checks: Option<String>,
}

#[derive(Serialize)]
pub struct ImapGeneralChecksResponse {
    checks: String,
}

pub async fn get_imap_general_checks(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ImapGeneralChecksResponse>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_repository.get_imap_general_checks(auth_user.user_id) {
        Ok(checks) => Ok(Json(ImapGeneralChecksResponse { checks })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to get IMAP general checks: {}", e)}))
        )),
    }
}

#[derive(Deserialize)]
pub struct ImapProactiveRequest {
    proactive: bool,
}

pub async fn update_imap_proactive(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<ImapProactiveRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_repository.update_imap_proactive(auth_user.user_id, request.proactive) {
        Ok(_) => Ok(Json(json!({
            "message": "IMAP proactive setting updated successfully"
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update IMAP proactive setting: {}", e)}))
        )),
    }
}

#[derive(Serialize)]
pub struct ImapProactiveResponse {
    proactive: bool,
}

pub async fn get_imap_proactive(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ImapProactiveResponse>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_repository.get_imap_proactive(auth_user.user_id) {
        Ok(proactive) => Ok(Json(ImapProactiveResponse { proactive })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to get IMAP proactive setting: {}", e)}))
        )),
    }
}

#[derive(Serialize)]
pub struct EmailJudgmentResponse {
    pub id: i32,
    pub email_timestamp: i32,
    pub processed_at: i32,
    pub should_notify: bool,
    pub score: i32,
    pub reason: String,
}

#[derive(Serialize)]
pub struct CalendarProactiveResponse {
    proactive: bool,
}

pub async fn get_calendar_proactive(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<CalendarProactiveResponse>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_repository.get_proactive_calendar(auth_user.user_id) {
        Ok(proactive) => Ok(Json(CalendarProactiveResponse { proactive })),
        Err(e) => {
            tracing::error!("Failed to get calendar proactive setting: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get calendar proactive setting: {}", e)}))
            ))
        }
    }
}

#[derive(Deserialize)]
pub struct CalendarProactiveRequest {
    proactive: bool,
}

pub async fn update_calendar_proactive(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<CalendarProactiveRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_repository.update_proactive_calendar(auth_user.user_id, request.proactive) {
        Ok(_) => Ok(Json(json!({
            "message": "Calendar proactive setting updated successfully"
        }))),
        Err(e) => {
            tracing::error!("Failed to update calendar proactive setting: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update calendar proactive setting: {}", e)}))
            ))
        }
    }
}

pub async fn get_email_judgments(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<EmailJudgmentResponse>>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_repository.get_user_email_judgments(auth_user.user_id) {
        Ok(judgments) => {
            let responses: Vec<EmailJudgmentResponse> = judgments
                .into_iter()
                .map(|j| EmailJudgmentResponse {
                    id: j.id.unwrap_or(0),
                    email_timestamp: j.email_timestamp,
                    processed_at: j.processed_at,
                    should_notify: j.should_notify,
                    score: j.score,
                    reason: j.reason,
                })
                .collect();
            Ok(Json(responses))
        },
        Err(e) => {
            tracing::error!("Failed to get email judgments: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get email judgments: {}", e)}))
            ))
        }
    }
}

pub async fn update_imap_general_checks(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<ImapGeneralChecksRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Convert Option<String> to Option<&str>
    let checks_ref: Option<&str> = request.checks.as_deref();

    // Update the IMAP general checks
    match state.user_repository.update_imap_general_checks(auth_user.user_id, checks_ref) {
        Ok(_) => Ok(Json(json!({
            "message": "IMAP general checks updated successfully"
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update IMAP general checks: {}", e)}))
        )),
    }
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Check if the user is deleting their own account or is an admin
    if auth_user.user_id != user_id && !state.user_repository.is_admin(auth_user.user_id).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))? {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "You can only delete your own account unless you're an admin"}))
        ));
    }

    tracing::info!("Attempting to delete user {}", user_id);
    
    // First verify the user exists
    match state.user_repository.find_by_id(user_id) {
        Ok(Some(_)) => {
            // User exists, proceed with deletion
            match state.user_repository.delete_user(user_id) {
                Ok(_) => {
                    tracing::info!("Successfully deleted user {}", user_id);
                    Ok(Json(json!({"message": "User deleted successfully"})))
                },
                Err(e) => {
                    tracing::error!("Failed to delete user {}: {}", user_id, e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Failed to delete user: {}", e)}))
                    ))
                }
            }
        },
        Ok(None) => {
            tracing::warn!("Attempted to delete non-existent user {}", user_id);
            Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"}))
            ))
        },
        Err(e) => {
            tracing::error!("Database error while checking user {}: {}", user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        }
    }
}


