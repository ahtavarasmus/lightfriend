use crate::api::twilio_client::{TwilioClient, TwilioCredentials};
use crate::handlers::auth_middleware::AuthUser;
use crate::services::byot_setup::{
    configure_and_verify, safe_twilio_error, verify_live_configuration, ByotWebhookEndpoints,
};
use crate::{AppState, ByotRepository, UserCoreOps};
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

type ApiError = (StatusCode, Json<serde_json::Value>);

#[derive(Deserialize)]
pub struct UpdateTwilioPhoneRequest {
    twilio_phone: String,
}

#[derive(Deserialize)]
pub struct UpdateTwilioCredsRequest {
    account_sid: String,
    auth_token: String,
}

#[derive(Deserialize)]
pub struct UpdateOwnTwilioEnabledRequest {
    enabled: bool,
}

fn api_error(status: StatusCode, message: &str) -> ApiError {
    (status, Json(json!({ "error": message })))
}

fn valid_e164(phone: &str) -> bool {
    phone.len() >= 3
        && phone.len() <= 16
        && phone.starts_with('+')
        && phone.as_bytes().get(1).is_some_and(u8::is_ascii_digit)
        && phone[1..]
            .chars()
            .all(|character| character.is_ascii_digit())
        && !phone.starts_with("+0")
}

fn valid_account_sid(account_sid: &str) -> bool {
    account_sid.len() == 34
        && account_sid.starts_with("AC")
        && account_sid
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

fn valid_auth_token(auth_token: &str) -> bool {
    auth_token.len() >= 20
        && auth_token.len() <= 128
        && !auth_token.chars().any(char::is_whitespace)
}

fn webhook_endpoints() -> Result<ByotWebhookEndpoints, ApiError> {
    let server_url = std::env::var("SERVER_URL").unwrap_or_default();
    ByotWebhookEndpoints::from_server_url(&server_url).map_err(|error| {
        tracing::error!(code = error.code, "BYOT webhook base URL is invalid");
        api_error(StatusCode::SERVICE_UNAVAILABLE, error.user_message)
    })
}

fn current_phone(state: &AppState, user_id: i32) -> Result<String, ApiError> {
    state
        .user_core
        .find_by_id(user_id)
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch user"))?
        .and_then(|user| user.preferred_number)
        .filter(|phone| !phone.is_empty())
        .ok_or_else(|| {
            api_error(
                StatusCode::BAD_REQUEST,
                "Add your Twilio phone number before enabling own Twilio mode",
            )
        })
}

pub async fn update_twilio_phone(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<UpdateTwilioPhoneRequest>,
) -> Result<StatusCode, ApiError> {
    let phone = req.twilio_phone.trim();
    if !valid_e164(phone) {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Phone number must be in E.164 format (for example, +14155551234)",
        ));
    }

    let byot = ByotRepository::new(state.pg_pool.clone());
    byot.update_phone_and_invalidate(auth_user.user_id, phone)
        .map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update Twilio phone number",
            )
        })?;
    Ok(StatusCode::OK)
}

pub async fn update_twilio_creds(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<UpdateTwilioCredsRequest>,
) -> Result<StatusCode, ApiError> {
    if !valid_account_sid(req.account_sid.trim()) || !valid_auth_token(req.auth_token.trim()) {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "Enter a valid Twilio Account SID and Auth Token",
        ));
    }

    let phone = current_phone(&state, auth_user.user_id).unwrap_or_default();
    let byot = ByotRepository::new(state.pg_pool.clone());
    let encrypted_account_sid =
        crate::utils::encryption::encrypt(req.account_sid.trim()).map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to store Twilio credentials",
            )
        })?;
    let encrypted_auth_token =
        crate::utils::encryption::encrypt(req.auth_token.trim()).map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to store Twilio credentials",
            )
        })?;
    byot.replace_credentials_and_invalidate(
        auth_user.user_id,
        &phone,
        &encrypted_account_sid,
        &encrypted_auth_token,
    )
    .map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to store Twilio credentials",
        )
    })?;
    Ok(StatusCode::OK)
}

/// Read the live Twilio state and detect configuration drift. This endpoint
/// performs no provider mutation; a mismatch immediately disables BYOT.
pub async fn verify_byot_setup(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user_id = auth_user.user_id;
    let phone_number = current_phone(&state, user_id)?;
    let endpoints = webhook_endpoints()?;
    let (account_sid, auth_token) = state
        .user_repository
        .get_twilio_credentials(user_id)
        .map_err(|_| {
            api_error(
                StatusCode::BAD_REQUEST,
                "Add your Twilio credentials before verifying this number",
            )
        })?;
    let credentials = TwilioCredentials::new(account_sid, auth_token);
    let byot = ByotRepository::new(state.pg_pool.clone());

    let live = match state
        .twilio_client
        .fetch_incoming_phone_number(&credentials, &phone_number)
        .await
    {
        Ok(config) => config,
        Err(provider_error) => {
            let safe = safe_twilio_error(&provider_error);
            let _ = byot.mark_drifted(user_id, safe.code);
            tracing::warn!(user_id, code = safe.code, "BYOT verification failed");
            return Ok(Json(json!({
                "ok": false,
                "status": "error",
                "error_code": safe.code,
                "error": safe.user_message,
            })));
        }
    };

    match verify_live_configuration(&live, &phone_number, &endpoints) {
        Ok(()) => {
            let remains_enabled = byot.mark_checked(user_id).map_err(|_| {
                api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to persist Twilio verification",
                )
            })?;
            if !remains_enabled || !state.user_core.is_byot_user(user_id) {
                return Ok(Json(json!({
                    "ok": false,
                    "status": "configuration_valid",
                    "error_code": "reconnect_required",
                    "error": "The live callbacks are valid, but this number must be reconnected before routing is enabled.",
                })));
            }
            Ok(Json(json!({
                "ok": true,
                "status": "verified",
                "phone_number": phone_number,
                "sms_webhook": { "ok": true, "method": "POST" },
                "voice_webhook": { "ok": true, "method": "POST" },
            })))
        }
        Err(error) => {
            byot.mark_drifted(user_id, error.code).map_err(|_| {
                api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to persist Twilio drift state",
                )
            })?;
            tracing::warn!(
                user_id,
                code = error.code,
                "BYOT configuration drift detected"
            );
            Ok(Json(json!({
                "ok": false,
                "status": "drifted",
                "error_code": error.code,
                "error": error.user_message,
            })))
        }
    }
}

pub async fn update_own_twilio_enabled(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<UpdateOwnTwilioEnabledRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user_id = auth_user.user_id;
    if !req.enabled {
        state
            .user_core
            .update_own_twilio_enabled(user_id, false)
            .map_err(|_| {
                api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to disable own Twilio mode",
                )
            })?;
        if let Some(user) = state.user_core.find_by_id(user_id).ok().flatten() {
            if user.sub_tier.as_deref() == Some("tier 2") {
                if let Err(error) =
                    crate::utils::usage::ensure_current_included_usage_window(&state, &user)
                {
                    tracing::error!(user_id, %error, "Failed to refresh included usage");
                }
            }
        }
        return Ok(Json(json!({ "enabled": false })));
    }

    let phone_number = current_phone(&state, user_id)?;
    let endpoints = webhook_endpoints()?;
    let (account_sid, auth_token) = state
        .user_repository
        .get_twilio_credentials(user_id)
        .map_err(|_| {
            api_error(
                StatusCode::BAD_REQUEST,
                "Add your Twilio credentials before enabling own Twilio mode",
            )
        })?;
    let credentials = TwilioCredentials::new(account_sid, auth_token);
    let byot = ByotRepository::new(state.pg_pool.clone());
    let attempt_id = byot.start_attempt(user_id, &phone_number).map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to start Twilio verification",
        )
    })?;

    let verified = match configure_and_verify(
        state.twilio_client.as_ref(),
        &credentials,
        &phone_number,
        &endpoints,
    )
    .await
    {
        Ok(config) => config,
        Err(error) => {
            let _ = byot.fail_if_current(user_id, &attempt_id, error.code);
            tracing::warn!(user_id, code = error.code, "BYOT setup verification failed");
            return Err(api_error(StatusCode::BAD_REQUEST, error.user_message));
        }
    };

    let activated = byot
        .activate_if_current(user_id, &attempt_id, &verified.sid)
        .map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to persist Twilio verification",
            )
        })?;
    if !activated {
        return Err(api_error(
            StatusCode::CONFLICT,
            "Twilio settings changed while they were being verified. Please retry.",
        ));
    }

    if let Err(error) = state.user_repository.clear_included_usage_window(user_id) {
        tracing::error!(user_id, %error, "Failed to clear included usage after BYOT activation");
    }
    Ok(Json(json!({
        "enabled": true,
        "verification_status": "verified",
        "phone_number": phone_number,
    })))
}

/// Clear BYOT Twilio credentials (manual removal).
pub async fn clear_twilio_creds(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<StatusCode, ApiError> {
    let phone = current_phone(&state, auth_user.user_id).unwrap_or_default();
    let byot = ByotRepository::new(state.pg_pool.clone());
    byot.clear_credentials_and_invalidate(auth_user.user_id, &phone)
        .map_err(|_| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to disable own Twilio mode",
            )
        })?;
    Ok(StatusCode::OK)
}
