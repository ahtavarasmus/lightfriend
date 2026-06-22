use super::{MessageChannel, SmsProcessResponse, SmsResult};
use crate::models::user_models::User;
use crate::repositories::user_repository::LogUsageParams;
use crate::AppState;
use crate::UserCoreOps;
use std::sync::Arc;

pub(super) fn resolve_sms_user(state: &Arc<AppState>, from: &str) -> Result<User, SmsResult> {
    match state.user_core.find_by_phone_number(from) {
        Ok(Some(user)) => Ok(user),
        Ok(None) => {
            tracing::error!("No user found for phone number: {}", from);
            Err(SmsResult::user_not_found())
        }
        Err(e) => {
            tracing::error!(
                "Database error while finding user for phone number {}: {}",
                from,
                e
            );
            Err(SmsResult::database_error(&e.to_string()))
        }
    }
}

pub(super) async fn check_sms_access(state: &Arc<AppState>, user: &User) -> Result<(), SmsResult> {
    if let Err(e) = crate::utils::usage::check_user_credits(state, user, "message", None).await {
        let result = if e.contains("deactivated") {
            tracing::warn!("User {} phone service is deactivated", user.id);
            SmsResult::phone_deactivated()
        } else if e.contains("subscription") {
            tracing::warn!("User {} has no active subscription", user.id);
            SmsResult::no_subscription()
        } else {
            tracing::warn!("User {} has insufficient credits: {}", user.id, e);
            SmsResult::insufficient_credits()
        };
        return Err(result);
    }

    Ok(())
}

fn send_early_reply_if_needed(
    state: &Arc<AppState>,
    user: &User,
    reply: &str,
    channel: MessageChannel,
    failure_context: &'static str,
) {
    if !channel.sends_sms() {
        return;
    }

    let state_clone = state.clone();
    let user_clone = user.clone();
    let reply_clone = reply.to_string();
    tokio::spawn(async move {
        if let Err(e) = state_clone
            .channel_router
            .send_to_user(&user_clone, &reply_clone, None)
            .await
        {
            tracing::error!(
                "Failed to send {} to user {}: {}",
                failure_context,
                user_clone.id,
                e
            );
        }
    });
}

fn send_cancel_reply_in_background(
    state: &Arc<AppState>,
    user: &User,
    response_msg: &str,
    start_time: std::time::Instant,
) {
    let state_clone = state.clone();
    let user_clone = user.clone();
    let response_msg_clone = response_msg.to_string();

    tokio::spawn(async move {
        match state_clone
            .channel_router
            .send_to_user(&user_clone, &response_msg_clone, None)
            .await
        {
            Ok(message_sid) => {
                let message_sid = message_sid.into_inner();
                let processing_time_secs = start_time.elapsed().as_secs();
                if let Err(e) = state_clone.user_repository.log_usage(LogUsageParams {
                    user_id: user_clone.id,
                    sid: Some(message_sid.clone()),
                    activity_type: "sms".to_string(),
                    credits: None,
                    time_consumed: Some(processing_time_secs as i32),
                    success: Some(true),
                    reason: Some("cancel handling".to_string()),
                    status: None,
                    recharge_threshold_timestamp: None,
                    zero_credits_timestamp: None,
                }) {
                    tracing::error!("Failed to log SMS usage for cancel: {}", e);
                }
                // SMS credits deducted at Twilio status callback
            }
            Err(e) => {
                tracing::error!("Failed to send cancel response message: {}", e);
                let processing_time_secs = start_time.elapsed().as_secs();
                let error_status = format!("failed to send: {}", e);
                if let Err(log_err) = state_clone.user_repository.log_usage(LogUsageParams {
                    user_id: user_clone.id,
                    sid: None,
                    activity_type: "sms".to_string(),
                    credits: None,
                    time_consumed: Some(processing_time_secs as i32),
                    success: Some(false),
                    reason: Some("cancel handling".to_string()),
                    status: Some(error_status),
                    recharge_threshold_timestamp: None,
                    zero_credits_timestamp: None,
                }) {
                    tracing::error!(
                        "Failed to log SMS usage after send error for cancel: {}",
                        log_err
                    );
                }
            }
        }
    });
}

async fn handle_commitment_reply(
    state: &Arc<AppState>,
    user: &User,
    body: &str,
    channel: MessageChannel,
) -> Option<SmsProcessResponse> {
    let reply = crate::proactive::commitment_replies::try_handle_reply(state, user, body).await?;
    send_early_reply_if_needed(
        state,
        user,
        &reply,
        channel,
        "commitment-reply confirmation",
    );
    Some(SmsResult::Success { response: reply }.into_response())
}

async fn handle_alert_feedback_reply(
    state: &Arc<AppState>,
    user: &User,
    body: &str,
    channel: MessageChannel,
) -> Option<SmsProcessResponse> {
    let reply = crate::proactive::alert_feedback::try_handle_reply(state, user, body).await?;
    send_early_reply_if_needed(state, user, &reply, channel, "alert-feedback confirmation");
    Some(SmsResult::Success { response: reply }.into_response())
}

async fn handle_cancel_reply(
    state: &Arc<AppState>,
    user: &User,
    body: &str,
    channel: MessageChannel,
    start_time: &std::time::Instant,
) -> Option<SmsProcessResponse> {
    if body.trim().to_lowercase() != "c" {
        return None;
    }

    match crate::tool_call_utils::utils::cancel_pending_message(state, user.id).await {
        Ok(canceled) => {
            let response_msg = if canceled {
                "The message got discarded.".to_string()
            } else {
                "Couldn't find a message to cancel".to_string()
            };

            if channel.sends_sms() {
                send_cancel_reply_in_background(
                    state,
                    user,
                    &response_msg,
                    std::clone::Clone::clone(start_time),
                );
            }

            Some(
                SmsResult::Cancelled {
                    message: response_msg,
                }
                .into_response(),
            )
        }
        Err(e) => {
            tracing::error!("Failed to cancel pending message: {}", e);
            Some(
                SmsResult::SystemError {
                    log_msg: format!("Failed to cancel pending message: {}", e),
                }
                .into_response(),
            )
        }
    }
}

pub(super) async fn handle_sms_early_response(
    state: &Arc<AppState>,
    user: &User,
    body: &str,
    channel: MessageChannel,
    start_time: &std::time::Instant,
) -> Option<SmsProcessResponse> {
    // Commitment replies get priority over alert feedback because both use
    // short numeric replies and commitment prompts have their own pending state.
    if let Some(response) = handle_commitment_reply(state, user, body, channel).await {
        return Some(response);
    }

    if let Some(response) = handle_alert_feedback_reply(state, user, body, channel).await {
        return Some(response);
    }

    handle_cancel_reply(state, user, body, channel, start_time).await
}
