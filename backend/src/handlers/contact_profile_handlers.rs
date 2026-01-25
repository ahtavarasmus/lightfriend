use crate::UserCoreOps;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::{
    handlers::auth_middleware::AuthUser,
    models::user_models::{ContactProfile, ContactProfileException, NewContactProfile},
    repositories::user_repository::UpdateContactProfileParams,
    AppState,
};

// Request DTOs
#[derive(Deserialize)]
pub struct ExceptionRequest {
    pub platform: String,          // "whatsapp", "telegram", "signal", "email"
    pub notification_mode: String, // "all", "critical", "digest"
    pub notification_type: String, // "sms", "call", "call_sms"
    pub notify_on_call: bool,
}

#[derive(Deserialize)]
pub struct CreateContactProfileRequest {
    pub nickname: String,
    pub whatsapp_chat: Option<String>,
    pub telegram_chat: Option<String>,
    pub signal_chat: Option<String>,
    pub email_addresses: Option<String>,
    pub notification_mode: String, // "all", "critical", "digest"
    pub notification_type: String, // "sms", "call", "call_sms"
    pub notify_on_call: bool,
    pub exceptions: Option<Vec<ExceptionRequest>>,
}

#[derive(Deserialize)]
pub struct UpdateContactProfileRequest {
    pub nickname: String,
    pub whatsapp_chat: Option<String>,
    pub telegram_chat: Option<String>,
    pub signal_chat: Option<String>,
    pub email_addresses: Option<String>,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: bool,
    pub exceptions: Option<Vec<ExceptionRequest>>,
}

#[derive(Deserialize)]
pub struct UpdateDefaultModeRequest {
    pub mode: Option<String>,      // "critical", "digest", "ignore"
    pub noti_type: Option<String>, // "sms", "call", "call_sms"
    pub notify_on_call: Option<bool>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

// Response DTOs
#[derive(Serialize, Clone)]
pub struct ExceptionResponse {
    pub platform: String,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: bool,
}

impl From<ContactProfileException> for ExceptionResponse {
    fn from(e: ContactProfileException) -> Self {
        ExceptionResponse {
            platform: e.platform,
            notification_mode: e.notification_mode,
            notification_type: e.notification_type,
            notify_on_call: e.notify_on_call != 0,
        }
    }
}

#[derive(Serialize)]
pub struct ContactProfileResponse {
    pub id: i32,
    pub nickname: String,
    pub whatsapp_chat: Option<String>,
    pub telegram_chat: Option<String>,
    pub signal_chat: Option<String>,
    pub email_addresses: Option<String>,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: bool,
    pub exceptions: Vec<ExceptionResponse>,
}

impl ContactProfileResponse {
    pub fn from_profile_with_exceptions(
        p: ContactProfile,
        exceptions: Vec<ContactProfileException>,
    ) -> Self {
        ContactProfileResponse {
            id: p.id.unwrap_or(0),
            nickname: p.nickname,
            whatsapp_chat: p.whatsapp_chat,
            telegram_chat: p.telegram_chat,
            signal_chat: p.signal_chat,
            email_addresses: p.email_addresses,
            notification_mode: p.notification_mode,
            notification_type: p.notification_type,
            notify_on_call: p.notify_on_call != 0,
            exceptions: exceptions
                .into_iter()
                .map(ExceptionResponse::from)
                .collect(),
        }
    }
}

// Handlers

/// GET /api/contact-profiles
pub async fn get_contact_profiles(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    match state
        .user_repository
        .get_contact_profiles(auth_user.user_id)
    {
        Ok(profiles) => {
            let mut responses: Vec<ContactProfileResponse> = Vec::new();

            for profile in profiles {
                let profile_id = profile.id.unwrap_or(0);
                let exceptions = state
                    .user_repository
                    .get_profile_exceptions(profile_id)
                    .unwrap_or_default();
                responses.push(ContactProfileResponse::from_profile_with_exceptions(
                    profile, exceptions,
                ));
            }

            let default_mode = state
                .user_core
                .get_default_notification_mode(auth_user.user_id)
                .unwrap_or_else(|_| "critical".to_string());

            let default_noti_type = state
                .user_core
                .get_default_notification_type(auth_user.user_id)
                .unwrap_or_else(|_| "sms".to_string());

            let default_notify_on_call = state
                .user_core
                .get_default_notify_on_call(auth_user.user_id)
                .unwrap_or(true);

            Ok(Json(json!({
                "profiles": responses,
                "default_mode": default_mode,
                "default_noti_type": default_noti_type,
                "default_notify_on_call": default_notify_on_call
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get contact profiles: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to get contact profiles" })),
            ))
        }
    }
}

/// POST /api/contact-profiles
pub async fn create_contact_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<CreateContactProfileRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Validate nickname doesn't contain @ (to distinguish from email addresses)
    if request.nickname.contains('@') {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "error": "Nickname cannot contain '@'. Use only names like 'Mom' or 'Boss'." }),
            ),
        ));
    }

    // Validate notification_mode
    if !["all", "critical", "digest"].contains(&request.notification_mode.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "error": "Invalid notification_mode. Must be 'all', 'critical', or 'digest'" }),
            ),
        ));
    }

    // Validate notification_type
    if !["sms", "call", "call_sms"].contains(&request.notification_type.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "error": "Invalid notification_type. Must be 'sms', 'call', or 'call_sms'" }),
            ),
        ));
    }

    // Validate that at least one platform is connected
    if request.whatsapp_chat.is_none()
        && request.telegram_chat.is_none()
        && request.signal_chat.is_none()
        && request.email_addresses.is_none()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "At least one platform must be connected" })),
        ));
    }

    let new_profile = NewContactProfile {
        user_id: auth_user.user_id,
        nickname: request.nickname,
        whatsapp_chat: request.whatsapp_chat,
        telegram_chat: request.telegram_chat,
        signal_chat: request.signal_chat,
        email_addresses: request.email_addresses,
        notification_mode: request.notification_mode,
        notification_type: request.notification_type,
        notify_on_call: if request.notify_on_call { 1 } else { 0 },
        created_at: Utc::now().timestamp() as i32,
    };

    match state.user_repository.create_contact_profile(&new_profile) {
        Ok(profile) => {
            let profile_id = profile.id.unwrap_or(0);

            // Save exceptions if provided
            if let Some(exceptions) = request.exceptions {
                for exc in &exceptions {
                    // Validate exception fields
                    if !["whatsapp", "telegram", "signal", "email"].contains(&exc.platform.as_str())
                    {
                        continue;
                    }
                    if !["all", "critical", "digest", "ignore"]
                        .contains(&exc.notification_mode.as_str())
                    {
                        continue;
                    }
                    if !["sms", "call", "call_sms"].contains(&exc.notification_type.as_str()) {
                        continue;
                    }

                    if let Err(e) = state.user_repository.set_profile_exception(
                        profile_id,
                        &exc.platform,
                        &exc.notification_mode,
                        &exc.notification_type,
                        if exc.notify_on_call { 1 } else { 0 },
                    ) {
                        tracing::warn!("Failed to save exception for {}: {:?}", exc.platform, e);
                    }
                }
            }

            // Fetch exceptions to return in response
            let saved_exceptions = state
                .user_repository
                .get_profile_exceptions(profile_id)
                .unwrap_or_default();

            Ok(Json(json!({
                "success": true,
                "profile": ContactProfileResponse::from_profile_with_exceptions(profile, saved_exceptions)
            })))
        }
        Err(e) => {
            tracing::error!("Failed to create contact profile: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to create contact profile" })),
            ))
        }
    }
}

/// PUT /api/contact-profiles/:id
pub async fn update_contact_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(profile_id): Path<i32>,
    Json(request): Json<UpdateContactProfileRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Validate nickname doesn't contain @ (to distinguish from email addresses)
    if request.nickname.contains('@') {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "error": "Nickname cannot contain '@'. Use only names like 'Mom' or 'Boss'." }),
            ),
        ));
    }

    // Validate notification_mode
    if !["all", "critical", "digest"].contains(&request.notification_mode.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid notification_mode" })),
        ));
    }

    // Validate notification_type
    if !["sms", "call", "call_sms"].contains(&request.notification_type.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid notification_type" })),
        ));
    }

    match state
        .user_repository
        .update_contact_profile(UpdateContactProfileParams {
            user_id: auth_user.user_id,
            profile_id,
            nickname: request.nickname.clone(),
            whatsapp_chat: request.whatsapp_chat,
            telegram_chat: request.telegram_chat,
            signal_chat: request.signal_chat,
            email_addresses: request.email_addresses,
            notification_mode: request.notification_mode.clone(),
            notification_type: request.notification_type.clone(),
            notify_on_call: if request.notify_on_call { 1 } else { 0 },
        }) {
        Ok(()) => {
            // Handle exceptions if provided
            if let Some(exceptions) = request.exceptions {
                // Delete existing exceptions first, then add new ones
                if let Err(e) = state
                    .user_repository
                    .delete_all_profile_exceptions(profile_id)
                {
                    tracing::warn!("Failed to clear old exceptions: {:?}", e);
                }

                for exc in &exceptions {
                    // Validate exception fields
                    if !["whatsapp", "telegram", "signal", "email"].contains(&exc.platform.as_str())
                    {
                        continue;
                    }
                    if !["all", "critical", "digest", "ignore"]
                        .contains(&exc.notification_mode.as_str())
                    {
                        continue;
                    }
                    if !["sms", "call", "call_sms"].contains(&exc.notification_type.as_str()) {
                        continue;
                    }

                    if let Err(e) = state.user_repository.set_profile_exception(
                        profile_id,
                        &exc.platform,
                        &exc.notification_mode,
                        &exc.notification_type,
                        if exc.notify_on_call { 1 } else { 0 },
                    ) {
                        tracing::warn!("Failed to save exception for {}: {:?}", exc.platform, e);
                    }
                }
            }

            Ok(Json(json!({ "success": true })))
        }
        Err(e) => {
            tracing::error!("Failed to update contact profile: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to update contact profile" })),
            ))
        }
    }
}

/// DELETE /api/contact-profiles/:id
pub async fn delete_contact_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(profile_id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    match state
        .user_repository
        .delete_contact_profile(auth_user.user_id, profile_id)
    {
        Ok(()) => Ok(Json(json!({ "success": true }))),
        Err(e) => {
            tracing::error!("Failed to delete contact profile: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to delete contact profile" })),
            ))
        }
    }
}

/// PUT /api/contact-profiles/default-mode
pub async fn update_default_mode(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<UpdateDefaultModeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Update mode if provided
    if let Some(ref mode) = request.mode {
        if !["critical", "digest", "ignore"].contains(&mode.as_str()) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Invalid mode. Must be 'critical', 'digest', or 'ignore'" })),
            ));
        }
        if let Err(e) = state
            .user_core
            .set_default_notification_mode(auth_user.user_id, mode)
        {
            tracing::error!("Failed to update default notification mode: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to update default mode" })),
            ));
        }
    }

    // Update notification type if provided
    if let Some(ref noti_type) = request.noti_type {
        if !["sms", "call", "call_sms"].contains(&noti_type.as_str()) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Invalid noti_type. Must be 'sms', 'call', or 'call_sms'" })),
            ));
        }
        if let Err(e) = state
            .user_core
            .set_default_notification_type(auth_user.user_id, noti_type)
        {
            tracing::error!("Failed to update default notification type: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to update notification type" })),
            ));
        }
    }

    // Update notify on call if provided
    if let Some(notify_on_call) = request.notify_on_call {
        if let Err(e) = state
            .user_core
            .set_default_notify_on_call(auth_user.user_id, notify_on_call)
        {
            tracing::error!("Failed to update default notify on call: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to update call setting" })),
            ));
        }
    }

    Ok(Json(json!({ "success": true })))
}

/// GET /api/contact-profiles/search/:service?q=query
/// Searches for chats on the specified service (reuses existing bridge room search)
pub async fn search_chats(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(service): Path<String>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Validate service
    if !["whatsapp", "telegram", "signal"].contains(&service.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "error": "Invalid service. Must be 'whatsapp', 'telegram', or 'signal'" }),
            ),
        ));
    }

    // Use existing bridge room search functionality
    match crate::utils::bridge::search_bridge_rooms(&service, &state, auth_user.user_id, &query.q)
        .await
    {
        Ok(rooms) => {
            let results: Vec<serde_json::Value> = rooms
                .iter()
                .map(|room| {
                    json!({
                        "display_name": room.display_name,
                        "last_activity_formatted": room.last_activity_formatted
                    })
                })
                .collect();

            Ok(Json(json!({ "results": results })))
        }
        Err(e) => {
            tracing::error!("Failed to search chats: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to search: {}", e) })),
            ))
        }
    }
}
