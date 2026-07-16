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
    pub history_annotation: Option<String>,
}

pub fn notification_meta_from_snapshot(snap: &serde_json::Value) -> Option<NotificationMeta> {
    Some(NotificationMeta {
        platform: snap
            .get("platform")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        sender: snap
            .get("sender_name")
            .or_else(|| snap.get("sender"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        content: snap
            .get("content")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        history_annotation: email_ref_annotation_from_snapshot(snap),
    })
}

pub fn compact_email_notification(message: &str, snap: &serde_json::Value) -> String {
    let is_email = snap
        .get("platform")
        .and_then(|v| v.as_str())
        .map(|platform| platform.eq_ignore_ascii_case("email"))
        .unwrap_or(false);
    if !is_email {
        return message.to_string();
    }

    let Some(account) = snap
        .get("email_account")
        .and_then(|v| v.as_str())
        .filter(|account| !account.trim().is_empty())
    else {
        return message.to_string();
    };

    let trimmed = message.trim();
    if trimmed.starts_with(&format!("{}:", account)) {
        return trimmed.to_string();
    }

    let lower = trimmed.to_lowercase();
    let sender = snap
        .get("sender_name")
        .or_else(|| snap.get("sender"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let sender_on_email = format!("{} on email:", sender).to_lowercase();

    let body = if !sender.is_empty() && lower.starts_with(&sender_on_email) {
        trimmed
            .split_once(':')
            .map(|(_, tail)| tail.trim())
            .unwrap_or(trimmed)
    } else if lower.starts_with("critical email") || lower.starts_with("email from") {
        trimmed
            .split_once(':')
            .map(|(_, tail)| tail.trim())
            .unwrap_or(trimmed)
    } else {
        trimmed
    };

    if body.is_empty() {
        account.to_string()
    } else {
        format!("{}: {}", account, body)
    }
}

/// Resolve the delivery route for an automatic `now`-urgency notification.
///
/// Known contacts on chat platforms retain the escalation behavior that
/// forces a call. Every other message, including email, follows the user's
/// configured notification type. Returning an explicit content type keeps
/// `send_notification_with_context` from having to infer this later.
pub fn resolve_system_important_content_type(
    default_notification_type: Option<&str>,
    is_known_contact: bool,
    platform: &str,
) -> &'static str {
    if (is_known_contact && platform != "email") || default_notification_type == Some("call") {
        "system_important_call"
    } else {
        "system_important_sms"
    }
}

fn email_ref_annotation_from_snapshot(snap: &serde_json::Value) -> Option<String> {
    let is_email = snap
        .get("platform")
        .and_then(|v| v.as_str())
        .map(|platform| platform.eq_ignore_ascii_case("email"))
        .unwrap_or(false);
    if !is_email {
        return None;
    }

    let uid = snap
        .get("email_uid")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            snap.get("room_id")
                .and_then(|v| v.as_str())
                .and_then(crate::handlers::imap_handlers::parse_email_room_id)
                .map(|identity| identity.uid)
        })?;

    let mailbox = snap
        .get("mailbox")
        .and_then(|v| v.as_str())
        .unwrap_or("INBOX");

    let mut parts = Vec::new();
    if let Some(account_id) = snap.get("imap_connection_id").and_then(|v| v.as_i64()) {
        parts.push(format!("account_id={}", account_id));
    }
    if let Some(account) = snap.get("email_account").and_then(|v| v.as_str()) {
        parts.push(format!("account={}", account));
    }
    parts.push(format!("mailbox={}", mailbox));
    parts.push(format!("uid={}", uid));
    if let Some(message_id) = snap.get("message_id").and_then(|v| v.as_i64()) {
        parts.push(format!("ont_message_id={}", message_id));
    }

    Some(format!("[email_ref {}]", parts.join(" ")))
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
) -> bool {
    send_notification_with_context(
        state,
        user_id,
        notification,
        content_type,
        first_message,
        None,
    )
    .await
}

pub async fn send_notification_with_context(
    state: &Arc<AppState>,
    user_id: i32,
    notification: &str,
    content_type: String,
    first_message: Option<String>,
    meta: Option<NotificationMeta>,
) -> bool {
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::error!("User {} not found for notification", user_id);
            return false;
        }
        Err(e) => {
            tracing::error!("Failed to get user {}: {}", user_id, e);
            return false;
        }
    };

    let user_settings = match state.user_core.get_user_settings(user_id) {
        Ok(settings) => settings,
        Err(e) => {
            tracing::error!("Failed to get settings for user {}: {}", user_id, e);
            return false;
        }
    };

    let _user_info = match state.user_core.get_user_info(user_id) {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("Failed to get info for user {}: {}", user_id, e);
            return false;
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

    let mut sms_delivered = false;
    let outbound_notification = if content_type.starts_with("system_important") {
        format!(
            "{}\n\nReply 1=worth it, 2=should wait.",
            notification.trim_end()
        )
    } else {
        notification.to_string()
    };
    let history_notification = meta
        .as_ref()
        .and_then(|m| m.history_annotation.as_deref())
        .filter(|annotation| !annotation.trim().is_empty())
        .map(|annotation| format!("{} {}", outbound_notification, annotation.trim()))
        .unwrap_or_else(|| outbound_notification.clone());

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
                return false;
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
                        attempt_us_voice_fallback(state, &user, &greeting).await;
                    }
                }
            }

            match state
                .channel_router
                .send_to_user(&user, &outbound_notification, None)
                .await
            {
                Ok(response_sid) => {
                    let response_sid = response_sid.into_inner();
                    sms_delivered = true;
                    tracing::info!("SMS sent for call notification user {}", user_id);
                    let entry = crate::pg_models::NewPgMessageHistory {
                        user_id: user.id,
                        role: "assistant".to_string(),
                        encrypted_content: history_notification.clone(),
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
                return false;
            }
            match state
                .channel_router
                .send_to_user(&user, &outbound_notification, None)
                .await
            {
                Ok(response_sid) => {
                    let response_sid = response_sid.into_inner();
                    sms_delivered = true;
                    tracing::info!("Sent notification to user {}", user_id);
                    let entry = crate::pg_models::NewPgMessageHistory {
                        user_id: user.id,
                        role: "assistant".to_string(),
                        encrypted_content: history_notification.clone(),
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
    sms_delivered
}

/// Send SMS to an arbitrary phone (not necessarily a Lightfriend user).
/// Credits are deducted from `from_user`. Used for the accountability-friend
/// nudge, where the recipient has no account.
pub async fn send_sms_to_external_phone(
    state: &Arc<AppState>,
    from_user: &crate::models::user_models::User,
    phone: &str,
    body: &str,
) -> bool {
    let body = crate::utils::sms_sanitizer::apply_sms_url_filter(body);
    if body.trim().is_empty() {
        tracing::error!(
            "Refused to send empty accountability nudge from user {}",
            from_user.id
        );
        return false;
    }

    if std::env::var("ENVIRONMENT").unwrap_or_default() == "development" {
        tracing::info!(
            "DEV: skipping accountability nudge from user {} to {}",
            from_user.id,
            phone
        );
        return true;
    }

    if let Err(e) =
        crate::utils::usage::check_user_credits(state, from_user, "noti_msg", None).await
    {
        tracing::warn!(
            "User {} has insufficient credits to nudge accountability friend: {}",
            from_user.id,
            e
        );
        return false;
    }

    let channel_id = state.channel_router.pick_channel_for(from_user);
    match state
        .channel_router
        .notify(from_user, channel_id, phone, &body)
        .await
    {
        Ok(sid) => {
            let sid = sid.into_inner();
            tracing::info!(
                "Accountability nudge sent from user {} to friend phone (SID: {})",
                from_user.id,
                sid
            );
            if let Err(e) = state.user_repository.log_usage(LogUsageParams {
                user_id: from_user.id,
                sid: Some(sid),
                activity_type: "accountability_friend_sms".to_string(),
                credits: None,
                time_consumed: None,
                success: Some(true),
                reason: None,
                status: Some("delivered".to_string()),
                recharge_threshold_timestamp: None,
                zero_credits_timestamp: None,
            }) {
                tracing::error!("Failed to log accountability nudge usage: {}", e);
            }
            true
        }
        Err(e) => {
            tracing::error!(
                "Failed to send accountability nudge from user {}: {}",
                from_user.id,
                e
            );
            false
        }
    }
}

/// Try a voice fallback when an SMS notification failed for a US recipient.
///
/// Best-effort: errors are logged and swallowed so a fallback failure does
/// not propagate. No-op when the user is not US or the env var is unset.
async fn attempt_us_voice_fallback(
    state: &Arc<AppState>,
    user: &crate::models::user_models::User,
    notification_message: &str,
) {
    match crate::api::voice_pipeline::place_us_fallback_voice_call(
        state,
        user,
        notification_message,
    )
    .await
    {
        Ok(Some(sid)) => {
            tracing::info!(
                "Placed US voice fallback for user {} (SID: {})",
                user.id,
                sid
            );
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("US voice fallback failed for user {}: {}", user.id, e);
        }
    }
}
