//! Sinch SMS webhook handlers.
//!
//! Sinch's REST API delivers two webhook types to us:
//!   - `POST /api/sinch/inbound`  → Mobile Originated (MO) — a US user
//!     texted our `SINCH_US_FROM_NUMBER`. We turn it into the same
//!     `TwilioWebhookPayload` shape `process_sms` already consumes.
//!   - `POST /api/sinch/status`   → Delivery Report (DLR) — Sinch is
//!     telling us about an outbound message we sent. We upsert the row
//!     in `message_status_log` (since `SinchChannel::send` doesn't write
//!     it like `TwilioMessageService` does), then deduct credits on
//!     final status using a flat per-message price.
//!
//! Both routes are gated by `validate_sinch_auth` middleware, which
//! requires `Authorization: Bearer $SINCH_CALLBACK_SECRET`. If that env
//! var is unset, every request 401s — that's the on/off switch the
//! operator flips to enable Sinch.
//!
//! US-only enforcement: `sinch_inbound` rejects messages from users
//! whose phone country isn't US. The Sinch number is provisioned as
//! US-only; non-US users texting it would get a reply via Twilio
//! (their country's normal channel), which is confusing — better to
//! drop the message and let the user retry through their actual
//! channel.

use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use diesel::prelude::*;
use serde::Deserialize;
use std::sync::Arc;

use crate::api::twilio_sms::{self, TwilioWebhookPayload};
use crate::models::user_models::NewMessageStatusLog;
use crate::pg_schema::message_status_log;
use crate::utils::country::get_country_code_from_phone;
use crate::AppState;
use crate::UserCoreOps;

/// Sinch Mobile Originated message payload.
/// Sinch sends `from`/`to` in E.164 *without* the leading `+` (e.g. `15551234567`).
/// We re-add the `+` before doing user lookup since users are stored E.164-with-`+`.
#[derive(Debug, Deserialize)]
pub struct SinchInboundPayload {
    #[serde(rename = "type")]
    pub message_type: String,
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub body: Option<String>,
}

/// Sinch delivery report payload (per-recipient form).
/// Sinch also supports a batched form with `statuses[]`; we only handle
/// the per-recipient form here. The batched form requires extra dashboard
/// config that we don't enable.
#[derive(Debug, Deserialize)]
pub struct SinchDeliveryReport {
    #[serde(rename = "type")]
    pub report_type: String,
    pub batch_id: String,
    pub status: String,
    #[serde(default)]
    pub code: Option<i32>,
    #[serde(default)]
    pub to: Option<String>,
}

/// Normalize a phone number to E.164 with `+` prefix.
pub fn normalize_phone(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with('+') {
        trimmed.to_string()
    } else {
        format!("+{}", trimmed)
    }
}

/// Map Sinch's status string to Twilio's taxonomy so the rest of our
/// code (admin alerts, message_status_log, credit deduction predicates)
/// keeps working with one vocabulary.
///
/// Sinch states: Queued, Dispatched, Delivered, Failed, Rejected, Aborted, Expired, Unknown.
/// Twilio states: queued, sending, sent, delivered, failed, undelivered.
pub fn map_sinch_status(sinch_status: &str) -> &'static str {
    match sinch_status {
        "Queued" => "queued",
        "Dispatched" => "sent",
        "Delivered" => "delivered",
        "Failed" | "Rejected" => "failed",
        "Aborted" | "Expired" => "undelivered",
        _ => "queued",
    }
}

/// Whether a Twilio-taxonomy status is final (no more updates expected).
pub fn is_final_status(status: &str) -> bool {
    matches!(status, "delivered" | "failed" | "undelivered")
}

/// Read the configured per-message Sinch price in USD, used to deduct
/// credits on final status. Defaults to $0.0079 if `SINCH_USD_PER_MESSAGE`
/// is unset — a reasonable rate for US Sinch SMS that operators can
/// override as their actual contract dictates.
fn sinch_price_usd_per_message() -> f32 {
    std::env::var("SINCH_USD_PER_MESSAGE")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0079)
}

pub async fn sinch_inbound(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SinchInboundPayload>,
) -> StatusCode {
    if payload.message_type != "mo_text" {
        tracing::warn!(
            "Sinch inbound: ignoring non-text type {} (id={})",
            payload.message_type,
            payload.id
        );
        return StatusCode::OK;
    }

    let from = normalize_phone(&payload.from);
    let to = normalize_phone(&payload.to);
    let body = payload.body.unwrap_or_default();

    tracing::debug!(
        "Sinch inbound from={} to={} id={} body_len={}",
        from,
        to,
        payload.id,
        body.len()
    );

    let user = match state.user_core.find_by_phone_number(&from) {
        Ok(Some(u)) => u,
        Ok(None) => {
            tracing::warn!("Sinch inbound: no user found for phone {}", from);
            return StatusCode::OK;
        }
        Err(e) => {
            tracing::error!("Sinch inbound: db error finding user {}: {}", from, e);
            return StatusCode::OK;
        }
    };

    // US-only gate: Sinch number is provisioned for US, so non-US users
    // texting it would get replies via Twilio (their normal channel) —
    // confusing. Bypass the gate when the user is explicitly pinned to
    // Sinch via `preferred_sms_provider` (admin verification path).
    let pinned_to_sinch = user.preferred_sms_provider.as_deref() == Some("sinch");
    let country = get_country_code_from_phone(&user.phone_number);
    if country.as_deref() != Some("US") && !pinned_to_sinch {
        tracing::warn!(
            "Sinch inbound: user {} is not US ({:?}) and not pinned to sinch, dropping",
            user.id,
            country
        );
        return StatusCode::OK;
    }

    if body.trim().eq_ignore_ascii_case("STOP") {
        if let Err(e) = state.user_core.update_notify(user.id, false) {
            tracing::error!("Failed to update notify for STOP: {}", e);
        }
        return StatusCode::OK;
    }

    // Re-shape into the Twilio payload `process_sms` expects. The
    // `message_sid` we pass through here gets used by `process_sms`
    // for an inbound-message-deletion call against Twilio's API at
    // the tail. That call will 404 for a `sinch_*` SID — the error
    // is logged and ignored, mirroring the existing TextBee path.
    let twilio_payload = TwilioWebhookPayload {
        from,
        to,
        body,
        num_media: None,
        media_url0: None,
        media_content_type0: None,
        message_sid: format!("sinch_{}", payload.id),
    };

    let state_clone = state.clone();
    tokio::spawn(async move {
        let result = twilio_sms::process_sms(
            &state_clone,
            twilio_payload,
            twilio_sms::ProcessSmsOptions::default(),
        )
        .await;
        if result.0 != StatusCode::OK {
            tracing::error!("Sinch inbound process_sms failed: {:?}", result.0);
        }
    });

    StatusCode::OK
}

pub async fn sinch_status(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SinchDeliveryReport>,
) -> StatusCode {
    if payload.report_type != "delivery_report_sms" {
        tracing::warn!(
            "Sinch status: ignoring non-DLR type {} (batch_id={})",
            payload.report_type,
            payload.batch_id
        );
        return StatusCode::OK;
    }

    let mapped_status = map_sinch_status(&payload.status);
    tracing::info!(
        "Sinch status: batch_id={} sinch_status={} mapped={} code={:?}",
        payload.batch_id,
        payload.status,
        mapped_status,
        payload.code
    );

    let to_phone = payload.to.as_deref().map(normalize_phone);
    let user_id = match to_phone.as_deref() {
        Some(phone) => match state.user_core.find_by_phone_number(phone) {
            Ok(Some(u)) => Some(u.id),
            Ok(None) => {
                tracing::warn!("Sinch status: no user found for to={}", phone);
                None
            }
            Err(e) => {
                tracing::error!("Sinch status: db error looking up to={}: {}", phone, e);
                None
            }
        },
        None => None,
    };

    // Upsert message_status_log: SinchChannel::send doesn't write the
    // initial row (it's pg-agnostic by design), so the first DLR may
    // be the row's first appearance. We need user_id resolved to make
    // the row useful; if `to` is missing or unmapped we still record
    // the status under user_id 0 as a marker.
    if let Some(uid) = user_id {
        upsert_status_row(
            &state,
            &payload.batch_id,
            uid,
            to_phone.as_deref().unwrap_or(""),
            mapped_status,
            payload.code.map(|c| c.to_string()).as_deref(),
        );
    } else {
        tracing::warn!(
            "Sinch status: skipping row write for batch {} (no user)",
            payload.batch_id
        );
    }

    // Credit deduction on delivered. Sinch DLRs don't carry per-
    // message price (Sinch bills off-platform), so we use a configured
    // flat rate. Same margin treatment as Twilio prices. Only deduct
    // on `delivered` to avoid charging for failed/undelivered messages
    // — match Twilio's effective behavior (Twilio reports $0 price on
    // failed sends).
    if mapped_status == "delivered" {
        if let Some(uid) = user_id {
            let price = sinch_price_usd_per_message();
            match crate::utils::usage::deduct_from_twilio_price(
                &state,
                uid,
                price,
                Some(&payload.batch_id),
                "sinch",
            ) {
                Ok(cost) => {
                    tracing::info!(
                        "Sinch deducted {:.4} credits for user {} (batch: {})",
                        cost,
                        uid,
                        payload.batch_id
                    );
                    if let Err(e) = state
                        .user_repository
                        .update_usage_log_credits(&payload.batch_id, cost)
                    {
                        tracing::error!(
                            "Failed to update usage log credits for batch {}: {}",
                            payload.batch_id,
                            e
                        );
                    }
                }
                Err(e) => tracing::error!("Sinch credit deduction failed for user {}: {}", uid, e),
            }
        }
    }

    StatusCode::OK
}

/// Insert a new `message_status_log` row, or update the existing one
/// if `message_sid` already exists. Errors are logged, not returned —
/// status webhooks must always 2xx so Sinch doesn't retry.
fn upsert_status_row(
    state: &Arc<AppState>,
    sid: &str,
    user_id: i32,
    to_number: &str,
    status: &str,
    error_code: Option<&str>,
) {
    let mut conn = match state.pg_pool.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Sinch status: db pool get failed: {}", e);
            return;
        }
    };

    let now = Utc::now().timestamp() as i32;
    let new_row = NewMessageStatusLog {
        message_sid: sid.to_string(),
        user_id,
        direction: "outbound".to_string(),
        to_number: to_number.to_string(),
        from_number: std::env::var("SINCH_US_FROM_NUMBER").ok(),
        status: status.to_string(),
        error_code: error_code.map(|s| s.to_string()),
        error_message: None,
        created_at: now,
        updated_at: now,
        price: None,
        price_unit: None,
        encrypted_body: None,
    };

    let result = diesel::insert_into(message_status_log::table)
        .values(&new_row)
        .on_conflict(message_status_log::message_sid)
        .do_update()
        .set((
            message_status_log::status.eq(status),
            message_status_log::error_code.eq(error_code),
            message_status_log::updated_at.eq(now),
        ))
        .execute(&mut conn);

    match result {
        Ok(n) => tracing::debug!("Sinch status: upserted {} row(s) for sid {}", n, sid),
        Err(e) => tracing::error!("Sinch status: upsert failed for sid {}: {}", sid, e),
    }
}
