use std::sync::Arc;
use diesel::result::Error as DieselError;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ProactiveAgentEnabledRequest {
    enabled: bool,
}

#[derive(Serialize)]
pub struct ProactiveAgentEnabledResponse {
    enabled: bool,
}

#[derive(Deserialize)]
pub struct CriticalEnabledRequest {
    enabled: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct CriticalEnabledResponse {
    enabled: Option<String>,
    average_critical_per_day: f32,
    estimated_monthly_price: f32,
}

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
    agent_language: String,
    notification_type: Option<String>,
    save_context: Option<i32>,
    require_confirmation: bool,
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
    credits_left: f32,
    discount: bool,
    agent_language: String,
    notification_type: Option<String>,
    sub_country: Option<String>,
    save_context: Option<i32>,
    require_confirmation: bool,
    days_until_billing: Option<i32>,
    digests_reserved: i32,
    pairing_code: Option<String>,
    server_ip: Option<String>,
    twilio_sid: Option<String>,
    twilio_token: Option<String>,
    openrouter_api_key: Option<String>,
    textbee_device_id: Option<String>,
    textbee_api_key: Option<String>,
    estimated_monitoring_cost: f32,
}

use crate::handlers::auth_middleware::AuthUser;


pub async fn get_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ProfileResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get user profile and settings from database
    let user = state.user_core.find_by_id(auth_user.user_id).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?;
    match user {
        Some(user) => {
            let user_settings = state.user_core.get_user_settings(auth_user.user_id).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
            let user_info = state.user_core.get_user_info(auth_user.user_id).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
            let current_time = std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap()
                                                    .as_secs() as i32;
            // Get current digest settings
            let (morning_digest_time, day_digest_time, evening_digest_time) = state.user_core.get_digests(auth_user.user_id)
                .map_err(|e| {
                    tracing::error!("Failed to get digest settings: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Failed to get digest settings: {}", e)}))
                    )
                })?;
            // Count current active digests
            let current_count: i32 = [morning_digest_time.as_ref(), day_digest_time.as_ref(), evening_digest_time.as_ref()]
                .iter()
                .filter(|&&x| x.is_some())
                .count() as i32;
            let ttl = user.time_to_live.unwrap_or(0);
            let time_to_delete = current_time > ttl;
            let days_until_billing: Option<i32> = user.next_billing_date_timestamp.map(|date| {
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32;
                (date - current_time) / (24 * 60 * 60)
            });
            let digests_reserved = current_count * days_until_billing.unwrap_or(30);
            // Fetch Twilio credentials and mask them
            let (twilio_sid, twilio_token) = match state.user_core.get_twilio_credentials(auth_user.user_id) {
                Ok((sid, token)) => {
                    let masked_sid = if sid.len() >= 4 {
                        format!("...{}", &sid[sid.len() - 4..])
                    } else {
                        "...".to_string()
                    };
                    let masked_token = if token.len() >= 4 {
                        format!("...{}", &token[token.len() - 4..])
                    } else {
                        "...".to_string()
                    };
                    (Some(masked_sid), Some(masked_token))
                },
                Err(_) => (None, None),
            };
            // Fetch Textbee credentials and mask them
            let (textbee_device_id, textbee_api_key) = match state.user_core.get_textbee_credentials(auth_user.user_id) {
                Ok((id, key)) => {
                    let masked_key= if key.len() >= 4 {
                        format!("...{}", &key[key.len() - 4..])
                    } else {
                        "...".to_string()
                    };
                    let masked_id= if id.len() >= 4 {
                        format!("...{}", &id[id.len() - 4..])
                    } else {
                        "...".to_string()
                    };
                    (Some(masked_id), Some(masked_key))
                },
                Err(_) => (None, None),
            };
            let openrouter_api_key = match state.user_core.get_openrouter_api_key(auth_user.user_id) {
                Ok(key) => {
                    let masked_key= if key.len() >= 4 {
                        format!("...{}", &key[key.len() - 4..])
                    } else {
                        "...".to_string()
                    };
                    Some(masked_key)
                },
                Err(_) => None,
            };
            // Determine country based on phone number
            let phone_number = user.phone_number.clone();
            let country = if phone_number.starts_with("+1") {
                "US".to_string()
            } else if phone_number.starts_with("+358") {
                "FI".to_string()
            } else if phone_number.starts_with("+44") {
                "UK".to_string()
            } else if phone_number.starts_with("+61") {
                "AU".to_string()
            } else {
                "Other".to_string()
            };
            // Get critical notification info
            let critical_info = state.user_core.get_critical_notification_info(auth_user.user_id).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
            let estimated_critical_monthly = critical_info.estimated_monthly_price;
            // Get priority notification info
            let priority_info = state.user_core.get_priority_notification_info(auth_user.user_id).map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))?;
            let estimated_priority_monthly = priority_info.estimated_monthly_price;
            // Calculate digest estimated monthly cost
            let estimated_digest_monthly = if current_count > 0 {
                let active_count_f = current_count as f32;
                let cost_per_digest = if country == "US" {
                    0.5
                } else if country == "Other" {
                    0.0
                } else {
                    0.30
                };
                active_count_f * 30.0 * cost_per_digest
            } else {
                0.0
            };
            println!("digest est: {}, count: {}", estimated_digest_monthly, current_count);
            println!("critical est: {}", estimated_critical_monthly);
            // Calculate total estimated monitoring cost
            let estimated_monitoring_cost = estimated_critical_monthly + estimated_priority_monthly + estimated_digest_monthly;
            Ok(Json(ProfileResponse {
                id: user.id,
                email: user.email,
                phone_number: user.phone_number,
                nickname: user.nickname,
                verified: user.verified,
                time_to_live: ttl,
                time_to_delete: time_to_delete,
                credits: user.credits,
                notify: user_settings.notify,
                info: user_info.info,
                preferred_number: user.preferred_number,
                charge_when_under: user.charge_when_under,
                charge_back_to: user.charge_back_to,
                stripe_payment_method_id: user.stripe_payment_method_id,
                timezone: user_info.timezone,
                timezone_auto: user_settings.timezone_auto,
                sub_tier: user.sub_tier,
                credits_left: user.credits_left,
                discount: user.discount,
                agent_language: user_settings.agent_language,
                notification_type: user_settings.notification_type,
                sub_country: user_settings.sub_country,
                save_context: user_settings.save_context,
                require_confirmation: user_settings.require_confirmation,
                days_until_billing: days_until_billing,
                digests_reserved: digests_reserved,
                pairing_code: user_settings.server_instance_id,
                server_ip: user_settings.server_ip,
                twilio_sid: twilio_sid,
                twilio_token: twilio_token,
                openrouter_api_key: openrouter_api_key,
                textbee_device_id: textbee_device_id,
                textbee_api_key: textbee_api_key,
                estimated_monitoring_cost,
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
    // Get user and settings to check their subscription status
    let user = state.user_core.find_by_id(auth_user.user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ))?;

    let preferred_number = if user.discount_tier.is_some() {
        // If user has a discount_tier, get their dedicated number from environment
        let env_var_name = format!("TWILIO_USER_PHONE_NUMBER_{}", auth_user.user_id);
        std::env::var(&env_var_name).map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("No dedicated phone number found for user {}", auth_user.user_id)}))
        ))?
    } else {
        // If no discount_tier, validate the requested number is allowed
        let allowed_numbers = vec![
            std::env::var("USA_PHONE").expect("USA_PHONE must be set in environment"),
            std::env::var("FIN_PHONE").expect("FIN_PHONE must be set in environment"),
            std::env::var("AUS_PHONE").expect("AUS_PHONE must be set in environment"),
            std::env::var("GB_PHONE").expect("GB_PHONE must be set in environment"),
        ];
        
        if !allowed_numbers.contains(&request.preferred_number) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid preferred number. Must be one of the allowed Twilio numbers"}))
            ));
        }
        request.preferred_number.clone()
    };

    // Update preferred number
    state.user_core.update_preferred_number(auth_user.user_id, &preferred_number)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    println!("Updated preferred number to: {}", preferred_number);
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
    state.user_core.update_notify(user_id, request.notify)
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

    match state.user_core.update_timezone(
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
    println!("Updating profile with notification type: {:?}", update_req.notification_type);
 
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

    // Validate agent language
    let allowed_languages = vec!["en", "fi", "de"];
    if !allowed_languages.contains(&update_req.agent_language.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid agent language. Must be 'en', 'fi', or 'de'"}))
        ));
    }

    match state.user_core.update_profile(
        auth_user.user_id,
        &update_req.email,
        &update_req.phone_number,
        &update_req.nickname,
        &update_req.info,
        &update_req.timezone,
        &update_req.timezone_auto,
        update_req.notification_type.as_deref(),
        update_req.save_context,
        update_req.require_confirmation,
    ) {
        Ok(_) => {
            // Update agent language separately // TODO put to same down the line
            if let Err(e) = state.user_core.update_agent_language(auth_user.user_id, &update_req.agent_language) {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to update agent language: {}", e)}))
                ));
            }
        },        Err(DieselError::NotFound) => {
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



#[derive(Serialize)]
pub struct EmailJudgmentResponse {
    pub id: i32,
    pub email_timestamp: i32,
    pub processed_at: i32,
    pub should_notify: bool,
    pub score: i32,
    pub reason: String,
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


#[derive(Serialize)]
pub struct DigestsResponse {
    morning_digest_time: Option<String>,
    day_digest_time: Option<String>,
    evening_digest_time: Option<String>,
    amount_affordable_with_messages: i32,
    amount_affordable_with_messages_and_credits: i32,
}

#[derive(Deserialize)]
pub struct UpdateDigestsRequest {
    morning_digest_time: Option<String>,
    day_digest_time: Option<String>,
    evening_digest_time: Option<String>,
}


pub async fn get_digests(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<DigestsResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get current digest settings
    let (morning_digest_time, day_digest_time, evening_digest_time) = state.user_core.get_digests(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get digest settings: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get digest settings: {}", e)}))
            )
        })?;

    // Count current active digests
    let current_count = [morning_digest_time.as_ref(), day_digest_time.as_ref(), evening_digest_time.as_ref()]
        .iter()
        .filter(|&&x| x.is_some())
        .count();

    // Get next billing date
    let mut next_billing_date = state.user_core.get_next_billing_date(auth_user.user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to get next billing date: {}", e)}))
        ))?;

    // If no next billing date found, fetch it from Stripe
    if next_billing_date.is_none() {
        if let Ok(Json(response)) = crate::handlers::stripe_handlers::fetch_next_billing_date(
            State(state.clone()),
            auth_user.clone(),
            Path(auth_user.user_id)
        ).await {
            if let Some(date) = response.get("next_billing_date").and_then(|v| v.as_i64()) {
                next_billing_date = Some(date as i32);
            }
        }
    }

    // Calculate days until next billing
    let days_until_billing = next_billing_date.map(|date| {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;
        (date - current_time) / (24 * 60 * 60)
    }).unwrap_or(30); // Default to 30 days if we can't calculate

    // Get user for credit check
    let user = state.user_core.find_by_id(auth_user.user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to get user: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ))?;

    // Calculate credits needed per digest
    let credits_needed_per_digest = days_until_billing as i32;

    // Calculate available slots (max 3 - current)
    let available_slots = 3 - current_count as i32;

    // Calculate how many additional digests user can afford with credits_left
    let affordable_with_credits_left = if available_slots > 0 {
        let max_affordable = (user.credits_left / credits_needed_per_digest as f32).floor() as i32;
        std::cmp::min(max_affordable, available_slots)
    } else {
        0
    };

    // Calculate how many additional digests user can afford with total credits
    let affordable_with_total_credits = if available_slots > 0 {
        let mut max_affordable = 0;
        for additional in 1..=available_slots {
            let credits_needed = additional * credits_needed_per_digest;
            if crate::utils::usage::check_user_credits(
                &state,
                &user,
                "digest",
                Some(credits_needed)
            ).await.is_ok() {
                max_affordable = additional;
            } else {
                break;
            }
        }
        max_affordable
    } else {
        0
    };

    Ok(Json(DigestsResponse {
        morning_digest_time,
        day_digest_time,
        evening_digest_time,
        amount_affordable_with_messages: affordable_with_credits_left,
        amount_affordable_with_messages_and_credits: affordable_with_total_credits,
    }))
}

pub async fn update_digests(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<UpdateDigestsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Get current digest settings
    let (current_morning, current_day, current_evening) = state.user_core.get_digests(auth_user.user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to get current digest settings: {}", e)}))
        ))?;
    // Count current active digests
    let current_count = [current_morning.as_ref(), current_day.as_ref(), current_evening.as_ref()]
        .iter()
        .filter(|&&x| x.is_some())
        .count();
    // Count new active digests
    let new_count = [request.morning_digest_time.as_ref(), request.day_digest_time.as_ref(), request.evening_digest_time.as_ref()]
        .iter()
        .filter(|&&x| x.is_some())
        .count();
    match state.user_core.update_digests(
        auth_user.user_id,
        request.morning_digest_time.as_deref(),
        request.day_digest_time.as_deref(),
        request.evening_digest_time.as_deref(),
    ) {
        Ok(_) => {
            let message = String::from("Digest settings updated successfully");
            let response = json!({
                "message": message,
                "digests_changed": new_count != current_count,
                "previous_digest_count": current_count,
                "new_digest_count": new_count,
            });
            Ok(Json(response))
        },
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update digest settings: {}", e)}))
        )),
    }
}

pub async fn update_critical_enabled(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<CriticalEnabledRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Update critical enabled setting
    match state.user_core.update_critical_enabled(auth_user.user_id, request.enabled) {
        Ok(_) => Ok(Json(json!({
            "message": "Critical enabled setting updated successfully"
        }))),
        Err(e) => {
            tracing::error!("Failed to update critical enabled setting: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update critical enabled setting: {}", e)}))
            ))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct CriticalNotificationInfo {
    pub enabled: Option<String>,
    pub average_critical_per_day: f32,
    pub estimated_monthly_price: f32,
}

pub async fn get_critical_enabled(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<CriticalNotificationInfo>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_core.get_critical_notification_info(auth_user.user_id) {
        Ok(info) => Ok(Json(info)),
        Err(e) => {
            tracing::error!("Failed to get critical notification info: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get critical notification info: {}", e)})),
            ))
        }
    }
}


pub async fn update_proactive_agent_on(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<ProactiveAgentEnabledRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Update critical enabled setting
    match state.user_core.update_proactive_agent_on(auth_user.user_id, request.enabled) {
        Ok(_) => Ok(Json(json!({
            "message": "Proactive notifications setting updated successfully"
        }))),
        Err(e) => {
            tracing::error!("Failed to update proactive notifications setting: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update proactive notifications setting: {}", e)}))
            ))
        }
    }
}

pub async fn get_proactive_agent_on(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ProactiveAgentEnabledResponse>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_core.get_proactive_agent_on(auth_user.user_id) {
        Ok(enabled) => {
            Ok(Json(ProactiveAgentEnabledResponse{
                enabled,
            }))
        },
        Err(e) => {
            tracing::error!("Failed to get critical enabled setting: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get critical enabled setting: {}", e)}))
            ))
        }
    }
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Deleting user: {}", auth_user.user_id);

    if auth_user.user_id != user_id && !auth_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "You can only delete your own account unless you're an admin"}))
        ));
    }
    
    // First verify the user exists
    match state.user_core.find_by_id(user_id) {
        Ok(Some(_)) => {
            println!("user exists");
            // User exists, proceed with deletion
            match state.user_core.delete_user(user_id) {
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


