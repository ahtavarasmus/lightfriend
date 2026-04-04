use crate::repositories::user_repository::LogUsageParams;
use crate::AppState;
use crate::UserCoreOps;
use std::sync::Arc;

use chrono::{NaiveDate, NaiveDateTime};

/// Parse an ISO datetime string to a unix timestamp (seconds).
///
/// Handles:
/// - `2026-02-28T09:00:00Z` (full datetime with Z)
/// - `2026-02-28T09:00:00` (full datetime, assumed UTC)
/// - `2026-02-28` (date only, noon UTC)
///
/// Returns `None` on parse failure.
pub fn parse_iso_to_timestamp(s: &str) -> Option<i32> {
    let s = s.trim();
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp() as i32);
    }
    if s.contains('T') && !s.ends_with('Z') {
        let with_z = format!("{}Z", s);
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&with_z) {
            return Some(dt.timestamp() as i32);
        }
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
            return Some(ndt.and_utc().timestamp() as i32);
        }
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
            return Some(ndt.and_utc().timestamp() as i32);
        }
    }
    if let Ok(nd) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return nd
            .and_hms_opt(12, 0, 0)
            .map(|ndt| ndt.and_utc().timestamp() as i32);
    }
    None
}

/// Metadata for contextual quiet-mode rule matching.
pub struct NotificationMeta {
    pub platform: Option<String>,
    pub sender: Option<String>,
    pub content: Option<String>,
}

/// Extract platform name from a content_type string like "whatsapp_profile_sms".
pub fn extract_platform_from_content_type(ct: &str) -> Option<String> {
    let ct_lower = ct.to_lowercase();
    for prefix in &["whatsapp", "telegram", "signal", "email", "tesla"] {
        if ct_lower.starts_with(prefix) {
            return Some(prefix.to_string());
        }
    }
    None
}

pub async fn send_notification(
    state: &Arc<AppState>,
    user_id: i32,
    notification: &str,
    content_type: String,
    first_message: Option<String>,
) {
    send_notification_with_context(
        state,
        user_id,
        notification,
        content_type,
        first_message,
        None,
    )
    .await;
}

pub async fn send_notification_with_context(
    state: &Arc<AppState>,
    user_id: i32,
    notification: &str,
    content_type: String,
    first_message: Option<String>,
    meta: Option<NotificationMeta>,
) {
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::error!("User {} not found for notification", user_id);
            return;
        }
        Err(e) => {
            tracing::error!("Failed to get user {}: {}", user_id, e);
            return;
        }
    };

    let user_settings = match state.user_core.get_user_settings(user_id) {
        Ok(settings) => settings,
        Err(e) => {
            tracing::error!("Failed to get settings for user {}: {}", user_id, e);
            return;
        }
    };

    // Check quiet mode
    let inferred_platform = extract_platform_from_content_type(&content_type);
    let check_platform = meta
        .as_ref()
        .and_then(|m| m.platform.as_deref())
        .or(inferred_platform.as_deref());
    let check_sender = meta.as_ref().and_then(|m| m.sender.as_deref());
    let check_content = meta.as_ref().and_then(|m| m.content.as_deref());

    match state.user_core.check_quiet_with_context(
        user_id,
        check_platform,
        check_sender,
        check_content,
    ) {
        Ok(true) => {
            tracing::debug!(
                "Suppressed notification for user {} by quiet rule (platform={:?}, sender={:?})",
                user_id,
                check_platform,
                check_sender,
            );
            return;
        }
        Ok(false) => {}
        Err(e) => {
            tracing::error!(
                "Quiet mode check failed for user {}: {} - proceeding with notification",
                user_id,
                e
            );
        }
    }

    let _user_info = match state.user_core.get_user_info(user_id) {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("Failed to get info for user {}: {}", user_id, e);
            return;
        }
    };

    let notification_type = if content_type.contains("critical") {
        user_settings.critical_enabled.as_deref().unwrap_or("sms")
    } else if content_type == "digest" {
        "sms" // Digests are always SMS - never call for informational summaries
    } else if content_type.contains("_call") {
        "call"
    } else if content_type.contains("_sms") {
        "sms"
    } else {
        user_settings.notification_type.as_deref().unwrap_or("sms")
    };

    match notification_type {
        "call" => {
            if let Err(e) =
                crate::utils::usage::check_user_credits(state, &user, "noti_msg", None).await
            {
                tracing::warn!(
                    "User {} has insufficient credits for call notification: {}",
                    user.id,
                    e
                );
                return;
            }

            if crate::utils::usage::check_user_credits(state, &user, "noti_call", None)
                .await
                .is_ok()
            {
                // Build the greeting: use first_message if provided, otherwise the notification
                let greeting = first_message
                    .clone()
                    .unwrap_or_else(|| notification.to_string());

                match crate::api::voice_pipeline::make_notification_call(state, &user, &greeting)
                    .await
                {
                    Ok(call_sid) => {
                        tracing::info!("Call initiated for user {} (SID: {})", user_id, call_sid);
                        if let Err(e) = state.user_repository.log_usage(LogUsageParams {
                            user_id,
                            sid: Some(call_sid),
                            activity_type: format!("{}_call_conditional", content_type),
                            credits: None,
                            time_consumed: None,
                            success: None,
                            reason: None,
                            status: Some("ongoing".to_string()),
                            recharge_threshold_timestamp: None,
                            zero_credits_timestamp: None,
                        }) {
                            tracing::error!("Failed to log call notification usage: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to initiate call for user {}: {}", user_id, e);
                    }
                }
            }

            match state
                .twilio_message_service
                .send_sms(notification, None, &user)
                .await
            {
                Ok(response_sid) => {
                    tracing::info!("SMS sent for call notification user {}", user_id);
                    let entry = crate::pg_models::NewPgMessageHistory {
                        user_id: user.id,
                        role: "assistant".to_string(),
                        encrypted_content: notification.to_string(),
                        tool_name: None,
                        tool_call_id: None,
                        tool_calls_json: None,
                        created_at: current_time,
                        conversation_id: "".to_string(),
                    };
                    if let Err(e) = state.user_repository.create_message_history(&entry) {
                        tracing::error!("Failed to store call notification in history: {}", e);
                    }
                    if let Err(e) = state.user_repository.log_usage(LogUsageParams {
                        user_id,
                        sid: Some(response_sid),
                        activity_type: format!("{}_sms", content_type),
                        credits: None,
                        time_consumed: None,
                        success: Some(true),
                        reason: None,
                        status: Some("delivered".to_string()),
                        recharge_threshold_timestamp: None,
                        zero_credits_timestamp: None,
                    }) {
                        tracing::error!("Failed to log call notification SMS usage: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to send SMS for user {}: {}", user_id, e);
                }
            }
        }
        _ => {
            if let Err(e) =
                crate::utils::usage::check_user_credits(state, &user, "noti_msg", None).await
            {
                tracing::warn!("User {} has insufficient credits: {}", user.id, e);
                return;
            }
            match state
                .twilio_message_service
                .send_sms(notification, None, &user)
                .await
            {
                Ok(response_sid) => {
                    tracing::info!("Sent notification to user {}", user_id);
                    let entry = crate::pg_models::NewPgMessageHistory {
                        user_id: user.id,
                        role: "assistant".to_string(),
                        encrypted_content: notification.to_string(),
                        tool_name: None,
                        tool_call_id: None,
                        tool_calls_json: None,
                        created_at: current_time,
                        conversation_id: "".to_string(),
                    };
                    if let Err(e) = state.user_repository.create_message_history(&entry) {
                        tracing::error!("Failed to store notification in history: {}", e);
                    }
                    if let Err(e) = state.user_repository.log_usage(LogUsageParams {
                        user_id,
                        sid: Some(response_sid),
                        activity_type: content_type,
                        credits: None,
                        time_consumed: None,
                        success: Some(true),
                        reason: None,
                        status: Some("delivered".to_string()),
                        recharge_threshold_timestamp: None,
                        zero_credits_timestamp: None,
                    }) {
                        tracing::error!("Failed to log SMS notification usage: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to send notification: {}", e);
                    if let Err(log_err) = state.user_repository.log_usage(LogUsageParams {
                        user_id,
                        sid: None,
                        activity_type: content_type,
                        credits: None,
                        time_consumed: None,
                        success: Some(false),
                        reason: Some(format!("Failed to send SMS: {}", e)),
                        status: Some("failed".to_string()),
                        recharge_threshold_timestamp: None,
                        zero_credits_timestamp: None,
                    }) {
                        tracing::error!("Failed to log failed SMS notification: {}", log_err);
                    }
                }
            }
        }
    }
}
