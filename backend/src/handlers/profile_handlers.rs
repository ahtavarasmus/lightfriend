use crate::UserCoreOps;
use axum::{extract::State, http::StatusCode, Json};
use diesel::result::Error as DieselError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Geocode a location name to lat/lon using Geoapify API
async fn geocode_location(location: &str) -> Option<(f64, f64)> {
    let api_key = std::env::var("GEOAPIFY_API_KEY").ok()?;
    let client = reqwest::Client::new();

    let url = format!(
        "https://api.geoapify.com/v1/geocode/search?text={}&format=json&apiKey={}",
        urlencoding::encode(location),
        api_key
    );

    let response: serde_json::Value = client.get(&url).send().await.ok()?.json().await.ok()?;
    let results = response["results"].as_array()?;

    if results.is_empty() {
        return None;
    }

    let result = &results[0];
    let lat = result["lat"].as_f64()?;
    let lon = result["lon"].as_f64()?;

    Some((lat, lon))
}

#[derive(Deserialize)]
pub struct TimezoneUpdateRequest {
    timezone: String,
}
use axum::extract::Path;
use serde_json::json;

use crate::repositories::user_core::UpdateProfileParams;
use crate::repositories::user_repository::LogUsageParams;
use crate::utils::country::get_country_code_from_phone;
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
    location: String,
    nearby_places: String,
    preferred_number: Option<String>,
    // Optional 2FA verification for sensitive changes
    totp_code: Option<String>,
    passkey_response: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct SensitiveChangeRequirements {
    pub requires_2fa: bool,
    pub has_passkeys: bool,
    pub has_totp: bool,
    pub passkey_options: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct ProfileResponse {
    id: i32,
    email: String,
    phone_number: String,
    nickname: Option<String>,
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
    agent_language: String,
    notification_type: Option<String>,
    sub_country: Option<String>,
    save_context: Option<i32>,
    days_until_billing: Option<i32>,
    twilio_sid: Option<String>,
    twilio_token: Option<String>,
    estimated_monitoring_cost: f32,
    location: Option<String>,
    nearby_places: Option<String>,
    plan_type: Option<String>,     // "assistant", "autopilot", or "byot"
    phone_service_active: bool,    // whether phone service is active - can be disabled for security
    llm_provider: Option<String>, // "openai" (default) or "tinfoil" - user's LLM provider preference
    auto_create_items: bool, // whether to auto-detect and create trackable items from emails/messages
    system_important_notify: bool, // whether system auto-notifies for important messages
    has_any_connection: bool, // whether user has connected any service (email, bridges)
}
use crate::handlers::auth_middleware::AuthUser;

pub async fn get_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ProfileResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get user profile and settings from database
    let user = state.user_core.find_by_id(auth_user.user_id).map_err(|e| {
        tracing::error!("get_profile: find_by_id failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        )
    })?;
    match user {
        Some(user) => {
            let user_settings = state
                .user_core
                .get_user_settings(auth_user.user_id)
                .map_err(|e| {
                    tracing::error!("get_profile: get_user_settings failed: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
            let user_info = state
                .user_core
                .get_user_info(auth_user.user_id)
                .map_err(|e| {
                    tracing::error!("get_profile: get_user_info failed: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
            let days_until_billing: Option<i32> = user.next_billing_date_timestamp.map(|date| {
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32;
                (date - current_time) / (24 * 60 * 60)
            });
            // Fetch Twilio credentials and mask them
            let (twilio_sid, twilio_token) = match state
                .user_repository
                .get_twilio_credentials(auth_user.user_id)
            {
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
                }
                Err(_) => (None, None),
            };
            // Determine country based on phone number (default to "US" if unknown)
            let _country =
                get_country_code_from_phone(&user.phone_number).unwrap_or_else(|| "US".to_string());
            // Get critical notification info
            let critical_info = state
                .user_core
                .get_critical_notification_info(auth_user.user_id)
                .map_err(|e| {
                    tracing::error!("get_profile: get_critical_notification_info failed: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
            let estimated_critical_monthly = critical_info.estimated_monthly_price;
            // Calculate total estimated monitoring cost
            let estimated_monitoring_cost = estimated_critical_monthly;
            // Check if user has any connected services (for onboarding modal)
            let has_any_connection = state
                .user_repository
                .get_imap_credentials(auth_user.user_id)
                .ok()
                .flatten()
                .is_some()
                || state
                    .user_repository
                    .get_bridge(auth_user.user_id, "whatsapp")
                    .ok()
                    .flatten()
                    .is_some()
                || state
                    .user_repository
                    .get_bridge(auth_user.user_id, "telegram")
                    .ok()
                    .flatten()
                    .is_some()
                || state
                    .user_repository
                    .get_bridge(auth_user.user_id, "signal")
                    .ok()
                    .flatten()
                    .is_some();
            Ok(Json(ProfileResponse {
                id: user.id,
                email: user.email,
                phone_number: user.phone_number,
                nickname: user.nickname,
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
                agent_language: user_settings.agent_language,
                notification_type: user_settings.notification_type,
                sub_country: user_settings.sub_country,
                save_context: user_settings.save_context,
                days_until_billing,
                twilio_sid,
                twilio_token,
                estimated_monitoring_cost,
                location: user_info.location,
                nearby_places: user_info.nearby_places,
                plan_type: user.plan_type,
                phone_service_active: user_settings.phone_service_active,
                llm_provider: user_settings.llm_provider,
                auto_create_items: user_settings.auto_create_items,
                system_important_notify: user_settings.system_important_notify,
                has_any_connection,
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        )),
    }
}

/// Returns available sending numbers for notification-only country users
/// Allows them to choose between US messaging service and local numbers (FI, NL, GB, AU)
pub async fn get_available_sending_numbers(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let user = state
        .user_core
        .find_by_id(auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;

    let is_notification_only =
        crate::utils::country::is_notification_only_country(&user.phone_number);
    let has_byot = state.user_core.is_byot_user(auth_user.user_id);

    // Only show selector for notification-only users without BYOT
    let show_selector = is_notification_only && !has_byot;

    if !show_selector {
        return Ok(Json(json!({
            "show_selector": false,
            "available_numbers": [],
            "current_preferred": user.preferred_number,
            "is_notification_only": is_notification_only
        })));
    }

    // Build list of available numbers
    let mut available_numbers = Vec::new();

    if let Ok(num) = std::env::var("USA_PHONE") {
        available_numbers.push(json!({
            "code": "US",
            "number": num,
            "label": "United States (Default)"
        }));
    }
    if let Ok(num) = std::env::var("FIN_PHONE") {
        available_numbers.push(json!({
            "code": "FI",
            "number": num,
            "label": "Finland"
        }));
    }
    if let Ok(num) = std::env::var("NL_PHONE") {
        available_numbers.push(json!({
            "code": "NL",
            "number": num,
            "label": "Netherlands"
        }));
    }
    if let Ok(num) = std::env::var("GB_PHONE") {
        available_numbers.push(json!({
            "code": "GB",
            "number": num,
            "label": "United Kingdom"
        }));
    }
    if let Ok(num) = std::env::var("AUS_PHONE") {
        available_numbers.push(json!({
            "code": "AU",
            "number": num,
            "label": "Australia"
        }));
    }

    Ok(Json(json!({
        "show_selector": true,
        "available_numbers": available_numbers,
        "current_preferred": user.preferred_number,
        "is_notification_only": true
    })))
}

#[derive(Deserialize)]
pub struct NotifyCreditsRequest {
    notify: bool,
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
            Json(json!({"error": "You can only modify your own settings unless you're an admin"})),
        ));
    }

    // Update notify preference
    state
        .user_core
        .update_notify(user_id, request.notify)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    Ok(Json(json!({
        "message": "Notification preference updated successfully"
    })))
}

pub async fn update_timezone(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<TimezoneUpdateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    match state
        .user_core
        .update_timezone(auth_user.user_id, &request.timezone)
    {
        Ok(_) => Ok(Json(json!({
            "message": "Timezone updated successfully"
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        )),
    }
}

#[derive(Deserialize)]
pub struct PatchFieldRequest {
    field: String,
    value: serde_json::Value,
}

/// Generic endpoint to update individual profile fields
/// Allows inline editing on the frontend without bulk updates
pub async fn patch_profile_field(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<PatchFieldRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;

    match request.field.as_str() {
        "nickname" => {
            let value = request.value.as_str().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "nickname must be a string"})),
                )
            })?;
            if value.len() > 30 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Nickname must be 30 characters or less"})),
                ));
            }
            state
                .user_core
                .update_nickname(user_id, value)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
        }
        "info" => {
            let value = request.value.as_str().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "info must be a string"})),
                )
            })?;
            if value.len() > 500 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Info must be 500 characters or less"})),
                ));
            }
            state.user_core.update_info(user_id, value).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Database error: {}", e)})),
                )
            })?;
        }
        "location" => {
            let value = request.value.as_str().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "location must be a string"})),
                )
            })?;
            state
                .user_core
                .update_location(user_id, value)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;

            // Geocode and store coordinates for sunrise/sunset calculation
            if let Some((lat, lon)) = geocode_location(value).await {
                let _ = state
                    .user_core
                    .update_user_coordinates(user_id, lat as f32, lon as f32);
            }
        }
        "nearby_places" => {
            let value = request.value.as_str().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "nearby_places must be a string"})),
                )
            })?;
            state
                .user_core
                .update_nearby_places(user_id, value)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
        }
        "timezone" => {
            let value = request.value.as_str().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "timezone must be a string"})),
                )
            })?;
            // Validate timezone
            if value.parse::<chrono_tz::Tz>().is_err() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Invalid timezone"})),
                ));
            }
            state
                .user_core
                .update_timezone(user_id, value)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
        }
        "timezone_auto" => {
            let value = request.value.as_bool().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "timezone_auto must be a boolean"})),
                )
            })?;
            state
                .user_core
                .update_timezone_auto(user_id, value)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
        }
        "agent_language" => {
            let value = request.value.as_str().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "agent_language must be a string"})),
                )
            })?;
            let allowed_languages = ["en", "fi", "de"];
            if !allowed_languages.contains(&value) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Invalid agent language. Must be 'en', 'fi', or 'de'"})),
                ));
            }
            state
                .user_core
                .update_agent_language(user_id, value)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
        }
        "notification_type" => {
            let value = request.value.as_str().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "notification_type must be a string"})),
                )
            })?;
            let allowed_types = ["sms", "call"];
            if !allowed_types.contains(&value) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Invalid notification type. Must be 'sms' or 'call'"})),
                ));
            }
            state
                .user_core
                .update_notification_type(user_id, Some(value))
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
        }
        "save_context" => {
            let value = request.value.as_i64().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "save_context must be an integer"})),
                )
            })? as i32;
            if !(0..=10).contains(&value) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "save_context must be between 0 and 10"})),
                ));
            }
            state
                .user_core
                .update_save_context(user_id, value)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
        }
        "phone_service_active" => {
            let value = request.value.as_bool().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "phone_service_active must be a boolean"})),
                )
            })?;
            state
                .user_core
                .update_phone_service_active(user_id, value)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
        }
        "auto_create_items" => {
            let value = request.value.as_bool().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "auto_create_items must be a boolean"})),
                )
            })?;
            state
                .user_core
                .update_auto_create_items(user_id, value)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
        }
        "system_important_notify" => {
            let value = request.value.as_bool().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "system_important_notify must be a boolean"})),
                )
            })?;
            state
                .user_core
                .update_system_important_notify(user_id, value)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
        }
        "llm_provider" => {
            // No-op: Tinfoil is now the sole provider. Accept silently for
            // backward compatibility with older frontends.
            tracing::debug!("llm_provider change ignored - Tinfoil is sole provider");
        }
        "preferred_number" => {
            let value = request.value.as_str().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "preferred_number must be a string"})),
                )
            })?;

            // Get user to check if they're in a notification-only country
            let user = state
                .user_core
                .find_by_id(user_id)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?
                .ok_or_else(|| {
                    (
                        StatusCode::NOT_FOUND,
                        Json(json!({"error": "User not found"})),
                    )
                })?;

            // Only allow notification-only country users to change this setting
            if !crate::utils::country::is_notification_only_country(&user.phone_number) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(
                        json!({"error": "This setting is only available for notification-only countries"}),
                    ),
                ));
            }

            // Validate the number is one of the allowed local numbers
            let allowed_numbers = vec![
                std::env::var("USA_PHONE").ok(),
                std::env::var("FIN_PHONE").ok(),
                std::env::var("NL_PHONE").ok(),
                std::env::var("GB_PHONE").ok(),
                std::env::var("AUS_PHONE").ok(),
            ];
            let allowed_numbers: Vec<String> = allowed_numbers.into_iter().flatten().collect();

            if !allowed_numbers.contains(&value.to_string()) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(
                        json!({"error": "Invalid preferred number. Must be one of the available local numbers."}),
                    ),
                ));
            }

            state
                .user_core
                .update_preferred_number(user_id, value)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Database error: {}", e)})),
                    )
                })?;
        }
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Unknown field: {}", request.field)})),
            ));
        }
    }

    Ok(Json(json!({"success": true})))
}

/// Recalculate credits_left when user changes phone country.
/// With unified credit budget (25.0 for all countries), credits survive country changes unchanged.
async fn recalculate_credits_for_country_change(
    _state: &Arc<AppState>,
    user_id: i32,
    old_country: Option<&str>,
    new_country: Option<&str>,
    old_credits_left: f32,
    _plan_type: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Budget is 25.0 for all countries - no recalculation needed
    tracing::info!(
        "Country change for user {}: {:?} -> {:?}, credits_left={:.2} (unchanged)",
        user_id,
        old_country,
        new_country,
        old_credits_left
    );
    Ok(())
}

/// Check if 2FA is required for sensitive profile changes
/// Returns the 2FA requirements and passkey options if available
pub async fn check_sensitive_change_requirements(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<SensitiveChangeRequirements>, (StatusCode, Json<serde_json::Value>)> {
    // Check if user has TOTP enabled
    let has_totp = state
        .totp_repository
        .is_totp_enabled(auth_user.user_id)
        .unwrap_or(false);

    // Check if user has passkeys
    let passkey_count = state
        .webauthn_repository
        .get_passkey_count(auth_user.user_id)
        .unwrap_or(0);
    let has_passkeys = passkey_count > 0;

    // If user has passkeys, prepare authentication options
    let passkey_options = if has_passkeys {
        match prepare_passkey_auth_options(&state, auth_user.user_id).await {
            Ok(options) => Some(options),
            Err(e) => {
                tracing::error!("Failed to prepare passkey options: {}", e);
                None
            }
        }
    } else {
        None
    };

    Ok(Json(SensitiveChangeRequirements {
        requires_2fa: has_totp || has_passkeys,
        has_passkeys,
        has_totp,
        passkey_options,
    }))
}

/// Prepare passkey authentication options for sensitive change verification
async fn prepare_passkey_auth_options(
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<serde_json::Value, String> {
    use crate::utils::webauthn_config::get_webauthn;
    use webauthn_rs::prelude::*;

    let credentials = state
        .webauthn_repository
        .get_credentials_by_user(user_id)
        .map_err(|e| format!("Failed to get credentials: {:?}", e))?;

    if credentials.is_empty() {
        return Err("No passkeys registered".to_string());
    }

    // Deserialize credentials back to Passkey objects
    let passkeys: Vec<Passkey> = credentials
        .iter()
        .filter_map(|c| {
            let decrypted = state.webauthn_repository.get_decrypted_public_key(c).ok()?;
            serde_json::from_str(&decrypted).ok()
        })
        .collect();

    if passkeys.is_empty() {
        return Err("Failed to load credentials".to_string());
    }

    let webauthn = get_webauthn();

    // Start authentication
    let (rcr, auth_state) = webauthn
        .start_passkey_authentication(&passkeys)
        .map_err(|e| format!("Failed to start authentication: {:?}", e))?;

    // Store authentication state with "sensitive_change" context
    let state_json = serde_json::to_string(&auth_state)
        .map_err(|e| format!("Failed to serialize auth state: {:?}", e))?;

    state
        .webauthn_repository
        .create_challenge(
            user_id,
            &state_json,
            "sensitive_change",
            Some("profile_update".to_string()),
            300, // 5 minute TTL
        )
        .map_err(|e| format!("Failed to store challenge: {:?}", e))?;

    // Return the options for the frontend
    Ok(serde_json::json!({ "options": rcr }))
}

/// Verify TOTP code for sensitive changes
fn verify_totp_code(state: &Arc<AppState>, user_id: i32, code: &str) -> Result<bool, String> {
    use totp_rs::{Algorithm, Secret, TOTP};

    let secret_opt = state
        .totp_repository
        .get_secret(user_id)
        .map_err(|e| format!("Database error: {:?}", e))?;

    let secret_base32 = secret_opt.ok_or("TOTP not configured")?;

    // Get user email
    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|e| format!("Database error: {:?}", e))?
        .ok_or("User not found")?;

    let secret = Secret::Encoded(secret_base32);
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().unwrap(),
        Some("Lightfriend".to_string()),
        user.email,
    )
    .map_err(|e| format!("TOTP creation error: {:?}", e))?;

    Ok(totp.check_current(code).unwrap_or(false))
}

/// Verify passkey response for sensitive changes
async fn verify_passkey_response(
    state: &Arc<AppState>,
    user_id: i32,
    response: &serde_json::Value,
) -> Result<bool, String> {
    use crate::utils::webauthn_config::get_webauthn;
    use webauthn_rs::prelude::*;

    // Get the stored authentication state
    let challenge = state
        .webauthn_repository
        .get_valid_challenge(user_id, "sensitive_change")
        .map_err(|e| format!("Failed to get challenge: {:?}", e))?
        .ok_or("No pending authentication")?;

    // Deserialize authentication state
    let auth_state: PasskeyAuthentication = serde_json::from_str(&challenge.challenge)
        .map_err(|e| format!("Failed to deserialize auth state: {:?}", e))?;

    // Parse the response
    let pk_credential: PublicKeyCredential = serde_json::from_value(response.clone())
        .map_err(|e| format!("Failed to parse passkey response: {:?}", e))?;

    let webauthn = get_webauthn();

    // Finish authentication
    let auth_result = webauthn
        .finish_passkey_authentication(&pk_credential, &auth_state)
        .map_err(|e| format!("Authentication failed: {:?}", e))?;

    // Update the credential counter
    let credential_id = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        auth_result.cred_id().as_ref(),
    );
    let _ = state
        .webauthn_repository
        .update_counter(&credential_id, auth_result.counter() as i32);

    // Delete the challenge
    let _ = state
        .webauthn_repository
        .delete_challenges_by_type(user_id, "sensitive_change");

    Ok(true)
}

pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(update_req): Json<UpdateProfileRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!(
        "Updating profile with notification type: {:?}",
        update_req.notification_type
    );
    use regex::Regex;
    let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    if !email_regex.is_match(&update_req.email) {
        tracing::debug!("Invalid email format: {}", update_req.email);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid email format"})),
        ));
    }

    let phone_regex = Regex::new(r"^\+[1-9]\d{1,14}$").unwrap();
    if !phone_regex.is_match(&update_req.phone_number) {
        tracing::debug!("Invalid phone number format: {}", update_req.phone_number);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Phone number must be in E.164 format (e.g., +1234567890)"})),
        ));
    }
    // Validate agent language
    let allowed_languages = ["en", "fi", "de"];
    if !allowed_languages.contains(&update_req.agent_language.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid agent language. Must be 'en', 'fi', or 'de'"})),
        ));
    }

    // Get user's current data BEFORE updating (for credit recalculation and 2FA check)
    let current_user = state
        .user_core
        .find_by_id(auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;

    // Check if email or phone is changing
    let email_changing = current_user.email != update_req.email;
    let phone_changing = current_user.phone_number != update_req.phone_number;

    // If sensitive fields are changing, verify 2FA if user has it enabled
    if email_changing || phone_changing {
        let has_totp = state
            .totp_repository
            .is_totp_enabled(auth_user.user_id)
            .unwrap_or(false);
        let passkey_count = state
            .webauthn_repository
            .get_passkey_count(auth_user.user_id)
            .unwrap_or(0);
        let has_passkeys = passkey_count > 0;

        if has_totp || has_passkeys {
            // User has 2FA enabled, require verification
            let mut verified = false;

            // Try passkey verification first (if provided)
            if let Some(ref passkey_response) = update_req.passkey_response {
                match verify_passkey_response(&state, auth_user.user_id, passkey_response).await {
                    Ok(true) => {
                        verified = true;
                        tracing::info!("Passkey verification successful for sensitive change");
                    }
                    Ok(false) => {
                        return Err((
                            StatusCode::UNAUTHORIZED,
                            Json(json!({"error": "Passkey verification failed"})),
                        ));
                    }
                    Err(e) => {
                        tracing::error!("Passkey verification error: {}", e);
                        return Err((
                            StatusCode::UNAUTHORIZED,
                            Json(json!({"error": format!("Passkey verification error: {}", e)})),
                        ));
                    }
                }
            }

            // Try TOTP verification (if provided and not already verified)
            if !verified {
                if let Some(ref totp_code) = update_req.totp_code {
                    match verify_totp_code(&state, auth_user.user_id, totp_code) {
                        Ok(true) => {
                            verified = true;
                            tracing::info!("TOTP verification successful for sensitive change");
                        }
                        Ok(false) => {
                            // Also try as backup code
                            let backup_valid = state
                                .totp_repository
                                .verify_backup_code(auth_user.user_id, totp_code)
                                .unwrap_or(false);
                            if backup_valid {
                                verified = true;
                                tracing::info!(
                                    "Backup code verification successful for sensitive change"
                                );
                            } else {
                                return Err((
                                    StatusCode::UNAUTHORIZED,
                                    Json(json!({"error": "Invalid verification code"})),
                                ));
                            }
                        }
                        Err(e) => {
                            tracing::error!("TOTP verification error: {}", e);
                            return Err((
                                StatusCode::UNAUTHORIZED,
                                Json(json!({"error": format!("TOTP verification error: {}", e)})),
                            ));
                        }
                    }
                }
            }

            // If neither verification method was provided, return error with requirements
            if !verified {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": "2FA verification required",
                        "requires_2fa": true,
                        "has_passkeys": has_passkeys,
                        "has_totp": has_totp
                    })),
                ));
            }
        }
    }

    // Re-fetch current user for credit recalculation (already fetched above)
    let current_user = state
        .user_core
        .find_by_id(auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;
    // Detect old country from phone number
    let old_country = get_country_code_from_phone(&current_user.phone_number);
    let old_credits_left = current_user.credits_left;

    // Check for duplicate phone number (unless user is keeping their current phone)
    if !update_req.phone_number.is_empty() && update_req.phone_number != current_user.phone_number {
        if let Ok(Some(_)) = state
            .user_core
            .find_by_phone_number(&update_req.phone_number)
        {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({"error": "This phone number is already in use by another account"})),
            ));
        }
    }

    match state.user_core.update_profile(UpdateProfileParams {
        user_id: auth_user.user_id,
        email: &update_req.email,
        phone_number: &update_req.phone_number,
        nickname: &update_req.nickname,
        info: &update_req.info,
        timezone: &update_req.timezone,
        timezone_auto: &update_req.timezone_auto,
        notification_type: update_req.notification_type.as_deref(),
        save_context: update_req.save_context,
        location: &update_req.location,
        nearby_places: &update_req.nearby_places,
        preferred_number: update_req.preferred_number.as_deref(),
    }) {
        Ok(_) => {
            if let Err(e) = state
                .user_core
                .update_agent_language(auth_user.user_id, &update_req.agent_language)
            {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to update agent language: {}", e)})),
                ));
            }
            // Detect new country from updated phone number
            let new_country = get_country_code_from_phone(&update_req.phone_number);

            // Update preferred Lightfriend number if country changed
            if old_country != new_country {
                if let Some(ref country) = new_country {
                    // Only update if user doesn't have BYOT (bring your own Twilio)
                    if !state.user_core.is_byot_user(auth_user.user_id) {
                        if let Err(e) = state
                            .user_core
                            .set_preferred_number_for_country(auth_user.user_id, country)
                        {
                            tracing::error!(
                                "Failed to update preferred number for country {}: {}",
                                country,
                                e
                            );
                        }
                    }
                }
            }

            // Recalculate credits if country changed and user has credits_left
            if old_country != new_country
                && old_credits_left > 0.0
                && current_user.sub_tier.is_some()
            {
                if let Err(e) = recalculate_credits_for_country_change(
                    &state,
                    auth_user.user_id,
                    old_country.as_deref(),
                    new_country.as_deref(),
                    old_credits_left,
                    current_user.plan_type.as_deref(),
                )
                .await
                {
                    tracing::error!("Failed to recalculate credits after country change: {}", e);
                    // Continue anyway, user keeps their credits
                }
            }
        }
        Err(DieselError::NotFound) => {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({"error": "Email already exists"})),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            ));
        }
    }
    Ok(Json(json!({
        "message": "Profile updated successfully"
    })))
}

use crate::utils::tool_exec::get_nearby_towns;
use axum::extract::Query;

#[derive(Deserialize)]
pub struct GetNearbyPlacesQuery {
    pub location: String,
}

pub async fn get_nearby_places(
    State(_state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Query(query): Query<GetNearbyPlacesQuery>,
) -> Result<Json<Vec<String>>, (StatusCode, Json<serde_json::Value>)> {
    match get_nearby_towns(&query.location).await {
        Ok(places) => Ok(Json(places)),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )),
    }
}

#[derive(Deserialize)]
pub struct UpdateCriticalRequest {
    #[serde(default, deserialize_with = "deserialize_double_option")]
    enabled: Option<Option<String>>,
    call_notify: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    action_on_critical_message: Option<Option<String>>,
}

// Custom deserializer for Option<Option<T>> to handle {"field": null} correctly
fn deserialize_double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Ok(Some(Option::deserialize(deserializer)?))
}

pub async fn update_critical_settings(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<UpdateCriticalRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!(
        "Received update_critical_settings request: enabled={:?}, call_notify={:?}, action={:?}",
        request.enabled,
        request.call_notify,
        request.action_on_critical_message
    );

    if let Some(enabled) = request.enabled {
        tracing::debug!("Updating critical_enabled to: {:?}", enabled);
        if let Err(e) = state
            .user_core
            .update_critical_enabled(auth_user.user_id, enabled)
        {
            tracing::error!("Failed to update critical enabled setting: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update critical enabled setting: {}", e)})),
            ));
        }
    }
    if let Some(call_notify) = request.call_notify {
        if let Err(e) = state
            .user_core
            .update_call_notify(auth_user.user_id, call_notify)
        {
            tracing::error!("Failed to update call notify setting: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update call notify setting: {}", e)})),
            ));
        }
    }
    if let Some(action) = request.action_on_critical_message {
        if let Err(e) = state
            .user_core
            .update_action_on_critical_message(auth_user.user_id, action)
        {
            tracing::error!("Failed to update action on critical message setting: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    json!({"error": format!("Failed to update action on critical message setting: {}", e)}),
                ),
            ));
        }
    }
    Ok(Json(json!({
        "message": "Critical settings updated successfully"
    })))
}

#[derive(Serialize, Deserialize)]
pub struct CriticalNotificationInfo {
    pub enabled: Option<String>,
    pub average_critical_per_day: f32,
    pub estimated_monthly_price: f32,
    pub call_notify: bool,
    pub action_on_critical_message: Option<String>,
}

pub async fn get_critical_settings(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<CriticalNotificationInfo>, (StatusCode, Json<serde_json::Value>)> {
    match state
        .user_core
        .get_critical_notification_info(auth_user.user_id)
    {
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

// Quiet Mode endpoints
#[derive(Serialize)]
pub struct QuietModeStatus {
    pub is_quiet: bool,
    pub until: Option<i32>,
    pub until_display: Option<String>,
    pub rules: Vec<QuietRuleInfo>,
}

#[derive(Serialize)]
pub struct QuietRuleInfo {
    pub id: i32,
    pub rule_type: String,
    pub platform: Option<String>,
    pub sender: Option<String>,
    pub topic: Option<String>,
    pub until: Option<i32>,
    pub until_display: Option<String>,
}

#[derive(Deserialize)]
pub struct SetQuietModeRequest {
    pub until: Option<i32>, // None = disable quiet mode, 0 = indefinite, timestamp = until
}

#[derive(Deserialize)]
pub struct AddQuietRuleRequest {
    pub until: i32,
    pub rule_type: String, // "suppress" or "allow"
    pub platform: Option<String>,
    pub sender: Option<String>,
    pub topic: Option<String>,
    pub description: Option<String>,
}

// parse_rule_info_from_item removed - items system has been removed

/// GET /api/profile/quiet-mode
pub async fn get_quiet_mode(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<QuietModeStatus>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_core.get_quiet_mode(auth_user.user_id) {
        Ok(quiet_until) => {
            let (is_quiet, until_display) = match quiet_until {
                None => (false, None),
                Some(0) => (true, Some("indefinitely".to_string())),
                Some(ts) => {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i32;

                    if ts <= now {
                        // Quiet mode expired, clear it
                        let _ = state.user_core.set_quiet_mode(auth_user.user_id, None);
                        (false, None)
                    } else {
                        // Still in quiet mode - format the display time
                        let display = format_quiet_until_display(ts, auth_user.user_id, &state);
                        (true, Some(display))
                    }
                }
            };

            // Items system removed - quiet rules are no longer stored as items
            let rules: Vec<QuietRuleInfo> = Vec::new();

            // is_quiet should also be true if there are active rules
            let effective_quiet = is_quiet || !rules.is_empty();

            Ok(Json(QuietModeStatus {
                is_quiet: effective_quiet,
                until: if is_quiet { quiet_until } else { None },
                until_display,
                rules,
            }))
        }
        Err(e) => {
            tracing::error!("Failed to get quiet mode: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get quiet mode: {}", e)})),
            ))
        }
    }
}

/// POST /api/profile/quiet-mode
pub async fn set_quiet_mode(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<SetQuietModeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    match state
        .user_core
        .set_quiet_mode(auth_user.user_id, request.until)
    {
        Ok(_) => {
            let message = match request.until {
                None => "Quiet mode disabled",
                Some(0) => "Quiet mode enabled indefinitely",
                Some(_) => "Quiet mode enabled until specified time",
            };
            Ok(Json(json!({ "message": message })))
        }
        Err(e) => {
            tracing::error!("Failed to set quiet mode: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to set quiet mode: {}", e)})),
            ))
        }
    }
}

/// POST /api/profile/quiet-rules
pub async fn add_quiet_rule(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<AddQuietRuleRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if request.rule_type != "suppress" && request.rule_type != "allow" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "rule_type must be 'suppress' or 'allow'"})),
        ));
    }

    let description = request.description.as_deref().unwrap_or("Quiet rule");

    match state.user_core.add_quiet_rule(
        auth_user.user_id,
        request.until,
        &request.rule_type,
        request.platform.as_deref(),
        request.sender.as_deref(),
        request.topic.as_deref(),
        description,
    ) {
        Ok(rule_id) => Ok(Json(json!({ "id": rule_id, "message": "Rule added" }))),
        Err(e) => {
            tracing::error!("Failed to add quiet rule: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to add quiet rule: {}", e)})),
            ))
        }
    }
}

/// DELETE /api/profile/quiet-rules
pub async fn delete_quiet_rules(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Delete all quiet_mode items (rules + global). This is the same as disabling quiet mode.
    match state.user_core.set_quiet_mode(auth_user.user_id, None) {
        Ok(_) => Ok(Json(json!({ "message": "All quiet rules cleared" }))),
        Err(e) => {
            tracing::error!("Failed to clear quiet rules: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to clear quiet rules: {}", e)})),
            ))
        }
    }
}

fn format_quiet_until_display(timestamp: i32, user_id: i32, state: &Arc<AppState>) -> String {
    use chrono::TimeZone;

    // Get user timezone
    let tz_str = state
        .user_core
        .get_user_info(user_id)
        .ok()
        .and_then(|info| info.timezone)
        .unwrap_or_else(|| "UTC".to_string());

    let tz: chrono_tz::Tz = tz_str.parse().unwrap_or(chrono_tz::UTC);

    let now = chrono::Utc::now();
    let _now_ts = now.timestamp() as i32;

    let target_dt = chrono::Utc
        .timestamp_opt(timestamp as i64, 0)
        .single()
        .map(|t| t.with_timezone(&tz));

    let now_local = now.with_timezone(&tz);

    match target_dt {
        Some(target) => {
            let now_date = now_local.date_naive();
            let target_date = target.date_naive();
            let days_diff = (target_date - now_date).num_days();

            let time_str = target.format("%l:%M%P").to_string().trim().to_string();

            if days_diff == 0 {
                format!("{} today", time_str)
            } else if days_diff == 1 {
                format!("{} tomorrow", time_str)
            } else {
                let day_name = target.format("%A").to_string();
                format!("{} {}", time_str, day_name)
            }
        }
        None => "unknown time".to_string(),
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
            Json(json!({"error": "You can only delete your own account unless you're an admin"})),
        ));
    }

    // First verify the user exists
    match state.user_core.find_by_id(user_id) {
        Ok(Some(_)) => {
            tracing::debug!("user exists");
            // User exists, proceed with deletion
            match state.user_core.delete_user(user_id) {
                Ok(_) => {
                    tracing::info!("Successfully deleted user {}", user_id);
                    Ok(Json(json!({"message": "User deleted successfully"})))
                }
                Err(e) => {
                    tracing::error!("Failed to delete user {}: {}", user_id, e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Failed to delete user: {}", e)})),
                    ))
                }
            }
        }
        Ok(None) => {
            tracing::warn!("Attempted to delete non-existent user {}", user_id);
            Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            ))
        }
        Err(e) => {
            tracing::error!("Database error while checking user {}: {}", user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            ))
        }
    }
}

// Web Chat - allows users to test the AI assistant through the dashboard
const WEB_CHAT_COST_EUR: f32 = 0.01; // €0.01 per message for Euro countries
const WEB_CHAT_COST_US: f32 = 0.5; // 0.5 messages for US/CA (uses credits_left as message count)

#[derive(Deserialize)]
pub struct WebChatRequest {
    pub message: String,
}

/// Media result from AI tool calls (YouTube, etc.)
#[derive(Serialize, Clone)]
pub struct MediaResult {
    pub platform: String,
    pub video_id: String,
    pub title: String,
    pub thumbnail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
}

#[derive(Serialize)]
pub struct WebChatResponse {
    pub message: String,
    pub credits_charged: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<Vec<MediaResult>>,
    /// ID of item created during this chat (if any) - for auto-showing item preview
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_item_id: Option<i32>,
}

pub async fn web_chat(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<WebChatRequest>,
) -> Result<Json<WebChatResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get the user
    let user = state
        .user_core
        .find_by_id(auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;

    // Check subscription - only subscribed users can use web chat
    if user.sub_tier.is_none() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Please subscribe to use the web chat feature"})),
        ));
    }

    // Determine cost based on region (US/CA uses message count, others use euro value)
    let is_us_or_ca = user.phone_number.starts_with("+1");
    let (credits_left_cost, credits_cost) = if is_us_or_ca {
        (WEB_CHAT_COST_US, WEB_CHAT_COST_EUR) // US: 0.5 from credits_left (message count), or €0.01 from credits
    } else {
        (WEB_CHAT_COST_EUR, WEB_CHAT_COST_EUR) // Euro: €0.01 from either
    };

    // Check if user has sufficient credits
    let has_credits = user.credits_left >= credits_left_cost || user.credits >= credits_cost;
    if !has_credits {
        return Err((
            StatusCode::PAYMENT_REQUIRED,
            Json(json!({"error": "Insufficient credits. Please add more credits to continue."})),
        ));
    }

    // Deduct credits (prefer credits_left, then credits)
    let charged_amount = if user.credits_left >= credits_left_cost {
        let new_credits_left = user.credits_left - credits_left_cost;
        state
            .user_repository
            .update_user_credits_left(auth_user.user_id, new_credits_left)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to charge credits: {}", e)})),
                )
            })?;
        credits_left_cost
    } else {
        let new_credits = user.credits - credits_cost;
        state
            .user_repository
            .update_user_credits(auth_user.user_id, new_credits)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to charge credits: {}", e)})),
                )
            })?;
        credits_cost
    };

    // Log the usage
    let _ = state.user_repository.log_usage(LogUsageParams {
        user_id: auth_user.user_id,
        sid: None,
        activity_type: "web_chat".to_string(),
        credits: Some(charged_amount),
        time_consumed: None,
        success: Some(true),
        reason: None,
        status: None,
        recharge_threshold_timestamp: None,
        zero_credits_timestamp: None,
    });

    // Create a mock Twilio payload to reuse existing SMS processing logic
    let mock_payload = crate::api::twilio_sms::TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: user
            .preferred_number
            .unwrap_or_else(|| "+0987654321".to_string()),
        body: request.message,
        num_media: None,
        media_url0: None,
        media_content_type0: None,
        message_sid: "".to_string(),
    };

    // Process using existing SMS handler (skip Twilio, credits handled above)
    let (status, _, response) = crate::api::twilio_sms::process_sms(
        &state,
        mock_payload,
        crate::api::twilio_sms::ProcessSmsOptions::web_chat(),
    )
    .await;

    if status == StatusCode::OK {
        // Extract media results from response if present
        let mut message = response.message.clone();
        let mut media: Option<Vec<MediaResult>> = None;

        // Debug: Log response to check for media tags
        tracing::debug!(
            "web_chat response message (first 500 chars): {}",
            &message.chars().take(500).collect::<String>()
        );
        tracing::debug!(
            "web_chat response contains [MEDIA_RESULTS]: {}",
            message.contains("[MEDIA_RESULTS]")
        );

        // Check for embedded media results from YouTube tool
        if let Some(start_idx) = message.find("[MEDIA_RESULTS]") {
            if let Some(end_idx) = message.find("[/MEDIA_RESULTS]") {
                let json_str = &message[start_idx + 15..end_idx];
                if let Ok(youtube_result) = serde_json::from_str::<
                    crate::tool_call_utils::youtube::YouTubeToolResult,
                >(json_str)
                {
                    let video_count = youtube_result.videos.len();
                    media = Some(
                        youtube_result
                            .videos
                            .into_iter()
                            .map(|v| MediaResult {
                                platform: "youtube".to_string(),
                                video_id: v.video_id,
                                title: v.title,
                                thumbnail: v.thumbnail,
                                duration: v.duration,
                                channel: Some(v.channel),
                            })
                            .collect(),
                    );
                    // Replace verbose text list with clean message when showing visual results
                    message = format!(
                        "Here are {} video{} I found:",
                        video_count,
                        if video_count == 1 { "" } else { "s" }
                    );
                }
            }
        }

        Ok(Json(WebChatResponse {
            message,
            credits_charged: charged_amount,
            media,
            created_item_id: response.created_item_id,
        }))
    } else {
        // No refund - credits are consumed on attempt
        Err((
            status,
            Json(json!({
                "error": "Failed to process message",
                "details": response.message
            })),
        ))
    }
}

/// SSE streaming web chat endpoint.
/// Streams status updates (thinking, tool calls, retries) then the final response.
pub async fn web_chat_stream(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Query(request): axum::extract::Query<WebChatRequest>,
) -> axum::response::sse::Sse<
    impl futures::stream::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>,
> {
    use crate::api::twilio_sms::{ChatStatus, ProcessSmsOptions};

    let stream = async_stream::stream! {
        // --- Auth & credit checks (same as web_chat) ---
        let user = match state.user_core.find_by_id(auth_user.user_id) {
            Ok(Some(u)) => u,
            Ok(None) => {
                yield Ok(axum::response::sse::Event::default().data(
                    serde_json::json!({"step": "error", "message": "User not found"}).to_string(),
                ));
                return;
            }
            Err(e) => {
                yield Ok(axum::response::sse::Event::default().data(
                    serde_json::json!({"step": "error", "message": format!("Database error: {}", e)}).to_string(),
                ));
                return;
            }
        };

        if user.sub_tier.is_none() {
            yield Ok(axum::response::sse::Event::default().data(
                serde_json::json!({"step": "error", "message": "Please subscribe to use the web chat feature"}).to_string(),
            ));
            return;
        }

        let is_us_or_ca = user.phone_number.starts_with("+1");
        let (credits_left_cost, credits_cost) = if is_us_or_ca {
            (WEB_CHAT_COST_US, WEB_CHAT_COST_EUR)
        } else {
            (WEB_CHAT_COST_EUR, WEB_CHAT_COST_EUR)
        };

        let has_credits = user.credits_left >= credits_left_cost || user.credits >= credits_cost;
        if !has_credits {
            yield Ok(axum::response::sse::Event::default().data(
                serde_json::json!({"step": "error", "message": "Insufficient credits. Please add more credits to continue."}).to_string(),
            ));
            return;
        }

        // Deduct credits
        let charged_amount = if user.credits_left >= credits_left_cost {
            let new_credits_left = user.credits_left - credits_left_cost;
            if let Err(e) = state.user_repository.update_user_credits_left(auth_user.user_id, new_credits_left) {
                yield Ok(axum::response::sse::Event::default().data(
                    serde_json::json!({"step": "error", "message": format!("Failed to charge credits: {}", e)}).to_string(),
                ));
                return;
            }
            credits_left_cost
        } else {
            let new_credits = user.credits - credits_cost;
            if let Err(e) = state.user_repository.update_user_credits(auth_user.user_id, new_credits) {
                yield Ok(axum::response::sse::Event::default().data(
                    serde_json::json!({"step": "error", "message": format!("Failed to charge credits: {}", e)}).to_string(),
                ));
                return;
            }
            credits_cost
        };

        let _ = state.user_repository.log_usage(crate::repositories::user_repository::LogUsageParams {
            user_id: auth_user.user_id,
            sid: None,
            activity_type: "web_chat".to_string(),
            credits: Some(charged_amount),
            time_consumed: None,
            success: Some(true),
            reason: None,
            status: None,
            recharge_threshold_timestamp: None,
            zero_credits_timestamp: None,
        });

        // Send initial thinking status
        yield Ok(axum::response::sse::Event::default().data(
            serde_json::json!({"step": "thinking", "message": "Thinking..."}).to_string(),
        ));

        // Create status channel
        let (status_tx, mut status_rx) = tokio::sync::mpsc::channel::<ChatStatus>(32);

        // Create mock Twilio payload
        let mock_payload = crate::api::twilio_sms::TwilioWebhookPayload {
            from: user.phone_number.clone(),
            to: user.preferred_number.unwrap_or_else(|| "+0987654321".to_string()),
            body: request.message.clone(),
            num_media: None,
            media_url0: None,
            media_content_type0: None,
            message_sid: "".to_string(),
        };

        // Spawn process_sms as a task
        let state_clone = state.clone();
        let mut process_handle = tokio::spawn(async move {
            crate::api::twilio_sms::process_sms(
                &state_clone,
                mock_payload,
                ProcessSmsOptions::web_chat_streaming(status_tx),
            )
            .await
        });

        // Stream status updates from the channel until process_sms completes
        #[allow(unused_assignments)]
        let mut task_result = None;
        loop {
            tokio::select! {
                status = status_rx.recv() => {
                    match status {
                        Some(ChatStatus::Thinking) => {
                            yield Ok(axum::response::sse::Event::default().data(
                                serde_json::json!({"step": "thinking", "message": "Thinking..."}).to_string(),
                            ));
                        }
                        Some(ChatStatus::ToolCall { name }) => {
                            let display = match name.as_str() {
                                "ask_perplexity" => "Searching the web...".to_string(),
                                "create_task" => "Creating item...".to_string(),
                                "send_sms" | "send_email" => "Preparing message...".to_string(),
                                "create_event" | "update_event" => "Managing events...".to_string(),
                                other => format!("Using {}...", other.replace('_', " ")),
                            };
                            yield Ok(axum::response::sse::Event::default().data(
                                serde_json::json!({"step": "tool_call", "message": display}).to_string(),
                            ));
                        }
                        Some(ChatStatus::Reasoning { snippet }) => {
                            yield Ok(axum::response::sse::Event::default().data(
                                serde_json::json!({"step": "reasoning", "message": snippet}).to_string(),
                            ));
                        }
                        Some(ChatStatus::Retrying { attempt, max }) => {
                            yield Ok(axum::response::sse::Event::default().data(
                                serde_json::json!({"step": "retry", "message": format!("Provider error, retrying... (attempt {}/{})", attempt, max)}).to_string(),
                            ));
                        }
                        Some(ChatStatus::RetryingFollowup { attempt, max }) => {
                            yield Ok(axum::response::sse::Event::default().data(
                                serde_json::json!({"step": "retry", "message": format!("Provider error, retrying... (attempt {}/{})", attempt, max)}).to_string(),
                            ));
                        }
                        None => {
                            // Channel closed - process_sms dropped the sender, task is finishing
                            task_result = Some(process_handle.await);
                            break;
                        }
                    }
                }
                result = &mut process_handle => {
                    // Small yield to let bridge tasks flush final events
                    tokio::task::yield_now().await;
                    // process_sms task completed before channel drained - drain remaining
                    while let Ok(status) = status_rx.try_recv() {
                        let msg = match status {
                            ChatStatus::Thinking => serde_json::json!({"step": "thinking", "message": "Thinking..."}),
                            ChatStatus::Reasoning { snippet } => serde_json::json!({"step": "reasoning", "message": snippet}),
                            ChatStatus::ToolCall { name } => serde_json::json!({"step": "tool_call", "message": format!("Using {}...", name.replace('_', " "))}),
                            ChatStatus::Retrying { attempt, max } => serde_json::json!({"step": "retry", "message": format!("Retrying... ({}/{})", attempt, max)}),
                            ChatStatus::RetryingFollowup { attempt, max } => serde_json::json!({"step": "retry", "message": format!("Retrying... ({}/{})", attempt, max)}),
                        };
                        yield Ok(axum::response::sse::Event::default().data(msg.to_string()));
                    }
                    task_result = Some(result);
                    break;
                }
            }
        }

        // Send the final complete/error event
        let final_result = match task_result {
            Some(Ok(r)) => Ok(r),
            Some(Err(e)) => Err(format!("Task panicked: {}", e)),
            None => Err("Task did not produce a result".to_string()),
        };
        match final_result {
            Ok((status, _, response)) => {
                if status == StatusCode::OK {
                    // Extract media results (same as web_chat)
                    let mut message = response.message.clone();
                    let mut media: Option<Vec<MediaResult>> = None;

                    if let Some(start_idx) = message.find("[MEDIA_RESULTS]") {
                        if let Some(end_idx) = message.find("[/MEDIA_RESULTS]") {
                            let json_str = &message[start_idx + 15..end_idx];
                            if let Ok(youtube_result) = serde_json::from_str::<
                                crate::tool_call_utils::youtube::YouTubeToolResult,
                            >(json_str) {
                                let video_count = youtube_result.videos.len();
                                media = Some(
                                    youtube_result.videos.into_iter().map(|v| MediaResult {
                                        platform: "youtube".to_string(),
                                        video_id: v.video_id,
                                        title: v.title,
                                        thumbnail: v.thumbnail,
                                        duration: v.duration,
                                        channel: Some(v.channel),
                                    }).collect(),
                                );
                                message = format!(
                                    "Here are {} video{} I found:",
                                    video_count,
                                    if video_count == 1 { "" } else { "s" }
                                );
                            }
                        }
                    }

                    let mut event_data = serde_json::json!({
                        "step": "complete",
                        "message": message,
                        "credits_charged": charged_amount,
                    });
                    if let Some(media) = media {
                        event_data["media"] = serde_json::to_value(media).unwrap_or_default();
                    }
                    if let Some(item_id) = response.created_item_id {
                        event_data["created_item_id"] = serde_json::json!(item_id);
                    }

                    yield Ok(axum::response::sse::Event::default().data(event_data.to_string()));
                } else {
                    let error_message = if response.message.trim().is_empty() {
                        "Failed to process message".to_string()
                    } else {
                        response.message.clone()
                    };
                    yield Ok(axum::response::sse::Event::default().data(
                        serde_json::json!({"step": "error", "message": error_message}).to_string(),
                    ));
                }
            }
            Err(e) => {
                yield Ok(axum::response::sse::Event::default().data(
                    serde_json::json!({"step": "error", "message": format!("Processing error: {}", e)}).to_string(),
                ));
            }
        }
    };

    axum::response::sse::Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keep-alive"),
    )
}

/// Web Chat with Image support - allows users to send messages with images through the dashboard
pub async fn web_chat_with_image(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    mut multipart: axum::extract::Multipart,
) -> Result<Json<WebChatResponse>, (StatusCode, Json<serde_json::Value>)> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    // Parse multipart form data
    let mut message = String::new();
    let mut image_data_url: Option<String> = None;
    let mut image_content_type: Option<String> = None;

    const MAX_IMAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB limit

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("Failed to process form data: {}", e)})),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();

        tracing::debug!("Processing multipart field: {}", name);
        match name.as_str() {
            "message" => {
                message = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Failed to read message: {}", e)})),
                    )
                })?;
                tracing::debug!("Received message text: '{}'", message);
            }
            "image" => {
                let content_type = field
                    .content_type()
                    .map(|ct| ct.to_string())
                    .unwrap_or_else(|| "image/png".to_string());

                // Validate it's an image
                if !content_type.starts_with("image/") {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": "Only image files are allowed"})),
                    ));
                }

                let data = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": format!("Failed to read image data: {}", e)})),
                    )
                })?;

                // Check file size
                if data.len() > MAX_IMAGE_SIZE {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(json!({"error": "Image size exceeds 10MB limit"})),
                    ));
                }

                // Convert to base64 data URL
                let base64 = STANDARD.encode(&data);
                image_data_url = Some(format!("data:{};base64,{}", content_type, base64));
                image_content_type = Some(content_type);
            }
            _ => continue,
        }
    }

    // Get the user
    let user = state
        .user_core
        .find_by_id(auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;

    // Check subscription - only subscribed users can use web chat
    if user.sub_tier.is_none() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Please subscribe to use the web chat feature"})),
        ));
    }

    // Determine cost based on region
    let is_us_or_ca = user.phone_number.starts_with("+1");
    let (credits_left_cost, credits_cost) = if is_us_or_ca {
        (WEB_CHAT_COST_US, WEB_CHAT_COST_EUR)
    } else {
        (WEB_CHAT_COST_EUR, WEB_CHAT_COST_EUR)
    };

    // Check if user has sufficient credits
    let has_credits = user.credits_left >= credits_left_cost || user.credits >= credits_cost;
    if !has_credits {
        return Err((
            StatusCode::PAYMENT_REQUIRED,
            Json(json!({"error": "Insufficient credits. Please add more credits to continue."})),
        ));
    }

    // Deduct credits
    let charged_amount = if user.credits_left >= credits_left_cost {
        let new_credits_left = user.credits_left - credits_left_cost;
        state
            .user_repository
            .update_user_credits_left(auth_user.user_id, new_credits_left)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to charge credits: {}", e)})),
                )
            })?;
        credits_left_cost
    } else {
        let new_credits = user.credits - credits_cost;
        state
            .user_repository
            .update_user_credits(auth_user.user_id, new_credits)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to charge credits: {}", e)})),
                )
            })?;
        credits_cost
    };

    // Log the usage
    let _ = state.user_repository.log_usage(LogUsageParams {
        user_id: auth_user.user_id,
        sid: None,
        activity_type: "web_chat".to_string(),
        credits: Some(charged_amount),
        time_consumed: None,
        success: Some(true),
        reason: if image_data_url.is_some() {
            Some("Web chat with image".to_string())
        } else {
            None
        },
        status: None,
        recharge_threshold_timestamp: None,
        zero_credits_timestamp: None,
    });

    // Create mock Twilio payload with image support
    // If there's an image but no text, provide a default prompt
    tracing::info!(
        "web_chat_with_image - message: '{}', has_image: {}",
        message,
        image_data_url.is_some()
    );
    let body = if message.trim().is_empty() && image_data_url.is_some() {
        "What's in this image?".to_string()
    } else {
        message
    };
    tracing::info!("web_chat_with_image - final body: '{}'", body);

    let mock_payload = crate::api::twilio_sms::TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: user
            .preferred_number
            .unwrap_or_else(|| "+0987654321".to_string()),
        body,
        num_media: image_data_url.as_ref().map(|_| "1".to_string()),
        media_url0: image_data_url,
        media_content_type0: image_content_type,
        message_sid: "".to_string(),
    };

    // Process using existing SMS handler (skip Twilio, credits handled above)
    let (status, _, response) = crate::api::twilio_sms::process_sms(
        &state,
        mock_payload,
        crate::api::twilio_sms::ProcessSmsOptions::web_chat(),
    )
    .await;

    if status == StatusCode::OK {
        // Extract media results from response if present (for web_chat_with_image)
        let mut message = response.message.clone();
        let mut media: Option<Vec<MediaResult>> = None;

        if let Some(start_idx) = message.find("[MEDIA_RESULTS]") {
            if let Some(end_idx) = message.find("[/MEDIA_RESULTS]") {
                let json_str = &message[start_idx + 15..end_idx];
                if let Ok(youtube_result) = serde_json::from_str::<
                    crate::tool_call_utils::youtube::YouTubeToolResult,
                >(json_str)
                {
                    media = Some(
                        youtube_result
                            .videos
                            .into_iter()
                            .map(|v| MediaResult {
                                platform: "youtube".to_string(),
                                video_id: v.video_id,
                                title: v.title,
                                thumbnail: v.thumbnail,
                                duration: v.duration,
                                channel: Some(v.channel),
                            })
                            .collect(),
                    );
                }
                message = format!("{}{}", &message[..start_idx], &message[end_idx + 16..])
                    .trim()
                    .to_string();
            }
        }

        Ok(Json(WebChatResponse {
            message,
            credits_charged: charged_amount,
            media,
            created_item_id: response.created_item_id,
        }))
    } else {
        // No refund - credits are consumed on attempt
        Err((
            status,
            Json(json!({
                "error": "Failed to process message",
                "details": response.message
            })),
        ))
    }
}
