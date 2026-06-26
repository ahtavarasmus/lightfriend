use super::{MessageChannel, SmsProcessResponse, TwilioResponse, TwilioWebhookPayload};
use crate::channels::traits::MediaRef;
use crate::models::user_models::User;
use crate::repositories::user_repository::LogUsageParams;
use crate::AppState;
use axum::{http::StatusCode, Json};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

thread_local! {
    static MEDIA_SID_MAP: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

pub(super) struct DeliverSmsResponseInput<'a> {
    pub state: &'a Arc<AppState>,
    pub user: &'a User,
    pub payload: &'a TwilioWebhookPayload,
    pub channel: MessageChannel,
    pub response_for_delivery: String,
    pub history_for_storage: String,
    pub created_item_id: Option<i32>,
    pub processing_time_secs: u64,
}

pub(super) async fn deliver_sms_response(input: DeliverSmsResponseInput<'_>) -> SmsProcessResponse {
    persist_assistant_history(input.state, input.user.id, &input.history_for_storage);

    if !input.channel.sends_sms() {
        log_web_chat_usage(input.state, input.user.id, input.processing_time_secs);
        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            Json(TwilioResponse {
                message: input.response_for_delivery,
                created_item_id: input.created_item_id,
            }),
        );
    }

    deliver_real_sms(input).await
}

fn persist_assistant_history(state: &Arc<AppState>, user_id: i32, history_for_storage: &str) {
    if let Err(e) = state.user_repository.delete_old_message_history(user_id) {
        tracing::error!("Failed to clean up old message history: {}", e);
    }

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // History storage uses the verifier's history version: tag markers are
    // preserved and the SMS footer/truncation are excluded.
    let assistant_message = crate::pg_models::NewPgMessageHistory {
        user_id,
        role: "assistant".to_string(),
        encrypted_content: history_for_storage.to_string(),
        tool_name: None,
        tool_call_id: None,
        tool_calls_json: None,
        created_at: current_time,
        conversation_id: "".to_string(),
    };

    if let Err(e) = state
        .user_repository
        .create_message_history(&assistant_message)
    {
        tracing::error!("Failed to store assistant message in history: {}", e);
    }
}

fn log_web_chat_usage(state: &Arc<AppState>, user_id: i32, processing_time_secs: u64) {
    if let Err(e) = state.user_repository.log_usage(LogUsageParams {
        user_id,
        sid: None,
        activity_type: "sms_test".to_string(),
        credits: None,
        time_consumed: Some(processing_time_secs as i32),
        success: None,
        reason: None,
        status: None,
        recharge_threshold_timestamp: None,
        zero_credits_timestamp: None,
    }) {
        tracing::error!("Failed to log test SMS usage: {}", e);
    }
}

async fn deliver_real_sms(input: DeliverSmsResponseInput<'_>) -> SmsProcessResponse {
    let (clean_response, media_sids) = extract_media_sids(&input.response_for_delivery);
    delete_incoming_twilio_message(input.state, input.user, &input.payload.message_sid);

    let media_ref = media_sids.first().map(|s| MediaRef::Url(s.clone()));
    match input
        .state
        .channel_router
        .send_to_user(input.user, &clean_response, media_ref)
        .await
    {
        Ok(message_sid) => {
            let message_sid = message_sid.into_inner();
            log_sms_usage(
                input.state,
                input.user.id,
                Some(message_sid),
                input.processing_time_secs,
                None,
            );

            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                Json(TwilioResponse {
                    message: "Message sent successfully".to_string(),
                    created_item_id: None,
                }),
            )
        }
        Err(e) => {
            tracing::error!("Failed to send conversation message: {}", e);
            let error_status = format!("failed to send: {}", e);
            log_sms_usage(
                input.state,
                input.user.id,
                None,
                input.processing_time_secs,
                Some(error_status),
            );

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                Json(TwilioResponse {
                    message: "Failed to send message".to_string(),
                    created_item_id: None,
                }),
            )
        }
    }
}

fn extract_media_sids(response: &str) -> (String, Vec<String>) {
    let mut media_sids = Vec::new();
    let clean_response = response
        .lines()
        .filter_map(|line| {
            MEDIA_SID_MAP.with(|map| {
                let map = map.borrow();
                for (filename, media_sid) in map.iter() {
                    if line.contains(filename) {
                        media_sids.push(media_sid.clone());
                        return None;
                    }
                }
                Some(line.to_string())
            })
        })
        .collect::<Vec<String>>()
        .join("\n");

    (clean_response, media_sids)
}

fn delete_incoming_twilio_message(state: &Arc<AppState>, user: &User, message_sid: &str) {
    let state_clone = state.clone();
    let msg_sid = message_sid.to_string();
    let user_clone = user.clone();

    tracing::debug!("going into deleting the incoming message handler");
    tokio::spawn(async move {
        if let Err(e) = state_clone
            .twilio_message_service
            .delete_message_with_retry(&user_clone, &msg_sid)
            .await
        {
            tracing::error!("Failed to delete incoming message {}: {}", msg_sid, e);
        }
    });
}

fn log_sms_usage(
    state: &Arc<AppState>,
    user_id: i32,
    sid: Option<String>,
    processing_time_secs: u64,
    error_status: Option<String>,
) {
    let success = if error_status.is_some() {
        Some(false)
    } else {
        None
    };

    if let Err(e) = state.user_repository.log_usage(LogUsageParams {
        user_id,
        sid,
        activity_type: "sms".to_string(),
        credits: None,
        time_consumed: Some(processing_time_secs as i32),
        success,
        reason: None,
        status: error_status,
        recharge_threshold_timestamp: None,
        zero_credits_timestamp: None,
    }) {
        if success == Some(false) {
            tracing::error!("Failed to log SMS usage after send error: {}", e);
        } else {
            tracing::error!("Failed to log SMS usage: {}", e);
        }
    }
}
