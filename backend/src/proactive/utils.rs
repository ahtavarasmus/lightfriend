use crate::repositories::user_repository::LogUsageParams;
use crate::AppState;
use crate::UserCoreOps;
use std::sync::Arc;

use chrono::{NaiveDate, NaiveDateTime, Offset};

/// Get the user's timezone offset in seconds from their stored timezone name.
pub fn user_tz_offset_secs(state: &AppState, user_id: i32) -> i32 {
    state
        .user_core
        .get_user_info(user_id)
        .ok()
        .and_then(|info| {
            let tz_name = info.timezone.as_deref().unwrap_or("UTC");
            tz_name.parse::<chrono_tz::Tz>().ok().map(|tz| {
                chrono::Utc::now()
                    .with_timezone(&tz)
                    .offset()
                    .fix()
                    .local_minus_utc()
            })
        })
        .unwrap_or(0)
}

/// Parse an ISO datetime string to a unix timestamp (seconds).
///
/// `tz_offset_secs`: the user's timezone offset in seconds (e.g. 10800 for UTC+3).
/// Naive datetimes (no Z or offset) are interpreted as the user's local time.
/// Datetimes with an explicit offset or Z are converted directly.
///
/// Handles:
/// - `2026-02-28T09:00:00+03:00` (explicit offset - used as-is)
/// - `2026-02-28T09:00:00Z` (UTC - used as-is)
/// - `2026-02-28T09:00:00` (naive - interpreted as user's local time)
/// - `2026-02-28T09:00` (naive short form)
/// - `2026-02-28` (date only, noon in user's local time)
///
/// Returns `None` on parse failure.
pub fn parse_iso_to_timestamp(s: &str, tz_offset_secs: i32) -> Option<i32> {
    let s = s.trim();
    // Explicit offset or Z - use as-is
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp() as i32);
    }
    if s.contains('T') && !s.ends_with('Z') {
        // Try appending Z for RFC3339 compat, but only if it has an offset already
        // (e.g. "2026-02-28T09:00+03:00" missing seconds)
        if s.contains('+') || s.matches('-').count() > 2 {
            let with_z = format!("{}:00{}", &s[..16], &s[16..]);
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&with_z) {
                return Some(dt.timestamp() as i32);
            }
        }
        // Naive datetime - interpret in user's timezone
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
            return Some(ndt.and_utc().timestamp() as i32 - tz_offset_secs);
        }
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
            return Some(ndt.and_utc().timestamp() as i32 - tz_offset_secs);
        }
    }
    if s.ends_with('Z') {
        let with_z = s.to_string();
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&with_z) {
            return Some(dt.timestamp() as i32);
        }
    }
    if let Ok(nd) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return nd
            .and_hms_opt(12, 0, 0)
            .map(|ndt| ndt.and_utc().timestamp() as i32 - tz_offset_secs);
    }
    None
}

/// Metadata for notification context.
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

    // Notify activity feed SSE subscribers after any notification attempt
    state.notify_activity_feed(user_id);
}
