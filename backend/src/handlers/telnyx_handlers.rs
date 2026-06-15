//! Telnyx SMS webhook handlers.
//!
//! Telnyx delivers webhooks for two relevant event_types:
//!   - `message.received`  → Mobile Originated (MO) — a US user texted our
//!     `TELNYX_US_FROM_NUMBER`. We turn it into the same
//!     `TwilioWebhookPayload` shape `process_sms` already consumes.
//!   - `message.sent` / `message.finalized` → Delivery status. We upsert
//!     the row in `message_status_log` (since `TelnyxChannel::send` doesn't
//!     write the initial row), and on `message.finalized` + delivered we
//!     deduct credits using a flat per-message price.
//!
//! Both routes are gated by `validate_telnyx_signature` middleware, which
//! verifies Ed25519 signatures using `TELNYX_PUBLIC_KEY`. If that env var
//! is unset, every request 401s — that's the on/off switch the operator
//! flips to enable Telnyx end-to-end.
//!
//! US-only enforcement: `telnyx_inbound` rejects messages from users
//! whose phone country isn't US. Bypass per-user with
//! `preferred_sms_provider = "telnyx"` (admin verification path).

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

/// Telnyx wraps every webhook payload in `{"data": {"event_type": ..., "payload": {...}}}`.
/// `P` is the inner-payload shape (different fields for inbound vs status).
#[derive(Debug, Deserialize)]
pub struct TelnyxWebhook<P> {
    pub data: TelnyxData<P>,
}

#[derive(Debug, Deserialize)]
pub struct TelnyxData<P> {
    pub event_type: String,
    #[serde(default)]
    pub id: String,
    pub payload: P,
}

/// Inner address shape used for both `from` and `to[]` entries. Telnyx
/// sends E.164 with the leading `+` already, so normalization is mostly
/// a whitespace trim, but we keep `normalize_phone` defensive.
#[derive(Debug, Deserialize)]
pub struct TelnyxAddress {
    pub phone_number: String,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TelnyxMedia {
    pub url: String,
    #[serde(default)]
    pub content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TelnyxInboundPayload {
    pub id: String,
    pub from: TelnyxAddress,
    pub to: Vec<TelnyxAddress>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub media: Vec<TelnyxMedia>,
}

#[derive(Debug, Deserialize)]
pub struct TelnyxStatusPayload {
    pub id: String,
    pub from: TelnyxAddress,
    pub to: Vec<TelnyxAddress>,
}

/// Normalize a phone number to E.164 with `+` prefix. Telnyx already
/// sends `+`-prefixed E.164 in practice, but be defensive.
pub fn normalize_phone(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with('+') {
        trimmed.to_string()
    } else {
        format!("+{}", trimmed)
    }
}

/// Map Telnyx's per-recipient status string to Twilio's taxonomy so the
/// rest of our code (admin alerts, message_status_log queries, credit
/// predicates) keeps working with one vocabulary.
///
/// Telnyx states: queued, sending, sent, delivering, delivered,
/// delivery_unconfirmed, sending_failed, delivery_failed.
/// Twilio states: queued, sending, sent, delivered, failed, undelivered.
pub fn map_telnyx_status(s: &str) -> &'static str {
    match s {
        "queued" => "queued",
        "sending" | "delivering" => "sending",
        "sent" | "delivery_unconfirmed" => "sent",
        "delivered" => "delivered",
        "sending_failed" | "delivery_failed" => "failed",
        _ => "queued",
    }
}

/// Read configured per-message Telnyx price in USD. Defaults to $0.004
/// (rough US Telnyx SMS list price). Operators override via env when
/// their actual contract dictates a different rate.
fn telnyx_price_usd_per_message() -> f32 {
    std::env::var("TELNYX_USD_PER_MESSAGE")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.004)
}

pub async fn telnyx_inbound(
    State(state): State<Arc<AppState>>,
    Json(webhook): Json<TelnyxWebhook<TelnyxInboundPayload>>,
) -> StatusCode {
    let event_type = webhook.data.event_type;
    if event_type != "message.received" {
        tracing::warn!(
            "Telnyx inbound: ignoring non-received event_type {}",
            event_type
        );
        return StatusCode::OK;
    }

    let payload = webhook.data.payload;
    let from = normalize_phone(&payload.from.phone_number);
    let to = payload
        .to
        .first()
        .map(|t| normalize_phone(&t.phone_number))
        .unwrap_or_default();
    let body = payload.text.clone().unwrap_or_default();

    tracing::debug!(
        "Telnyx inbound from={} to={} id={} body_len={}",
        from,
        to,
        payload.id,
        body.len()
    );

    let user = match state.user_core.find_by_phone_number(&from) {
        Ok(Some(u)) => u,
        Ok(None) => {
            tracing::warn!("Telnyx inbound: no user found for phone {}", from);
            return StatusCode::OK;
        }
        Err(e) => {
            tracing::error!("Telnyx inbound: db error finding user {}: {}", from, e);
            return StatusCode::OK;
        }
    };

    // US-only gate: Telnyx number is provisioned for US, so non-US users
    // texting it would get replies via Twilio (their normal channel) —
    // confusing. Bypass the gate when the user is explicitly pinned to
    // Telnyx via `preferred_sms_provider` (admin verification path).
    let pinned_to_telnyx = user.preferred_sms_provider.as_deref() == Some("telnyx");
    let country = get_country_code_from_phone(&user.phone_number);
    if country.as_deref() != Some("US") && !pinned_to_telnyx {
        tracing::warn!(
            "Telnyx inbound: user {} is not US ({:?}) and not pinned to telnyx, dropping",
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
    // the tail. That call will 404 for a `telnyx_*` SID — the error
    // is logged and ignored.
    let first_media = payload.media.first();
    let media_count = payload.media.len();
    let twilio_payload = TwilioWebhookPayload {
        from,
        to,
        body,
        num_media: if media_count > 0 {
            Some(media_count.to_string())
        } else {
            None
        },
        media_url0: first_media.map(|m| m.url.clone()),
        media_content_type0: first_media.and_then(|m| m.content_type.clone()),
        message_sid: format!("telnyx_{}", payload.id),
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
            tracing::error!("Telnyx inbound process_sms failed: {:?}", result.0);
        }
    });

    StatusCode::OK
}

pub async fn telnyx_status(
    State(state): State<Arc<AppState>>,
    Json(webhook): Json<TelnyxWebhook<TelnyxStatusPayload>>,
) -> StatusCode {
    let event_type = webhook.data.event_type;
    let is_final = event_type == "message.finalized";
    if event_type != "message.sent" && !is_final {
        tracing::debug!("Telnyx status: ignoring event_type {}", event_type);
        return StatusCode::OK;
    }

    let payload = webhook.data.payload;
    let recipient = match payload.to.first() {
        Some(r) => r,
        None => {
            tracing::warn!("Telnyx status: empty to[] for id {}", payload.id);
            return StatusCode::OK;
        }
    };

    let to_phone = normalize_phone(&recipient.phone_number);
    let raw_status = recipient.status.as_deref().unwrap_or("");
    let mapped_status = map_telnyx_status(raw_status);

    tracing::info!(
        "Telnyx status: event={} id={} telnyx_status={} mapped={}",
        event_type,
        payload.id,
        raw_status,
        mapped_status
    );

    let user_id = match state.user_core.find_by_phone_number(&to_phone) {
        Ok(Some(u)) => Some(u.id),
        Ok(None) => {
            tracing::warn!("Telnyx status: no user found for to={}", to_phone);
            None
        }
        Err(e) => {
            tracing::error!("Telnyx status: db error looking up to={}: {}", to_phone, e);
            None
        }
    };

    // Upsert message_status_log: TelnyxChannel::send doesn't write the
    // initial row, so the first webhook may be the row's first appearance.
    // Skip the row write entirely when we can't resolve a user — the row
    // would be useless without one.
    if let Some(uid) = user_id {
        upsert_status_row(&state, &payload.id, uid, &to_phone, mapped_status);
    } else {
        tracing::warn!(
            "Telnyx status: skipping row write for id {} (no user)",
            payload.id
        );
    }

    // Credit deduction only on the final delivered status. Telnyx fires
    // both message.sent (intermediate, when carrier accepts) and
    // message.finalized (terminal). Charging on either would double-count
    // or charge for failed deliveries, so guard on event_type AND
    // mapped status.
    if is_final && mapped_status == "delivered" {
        if let Some(uid) = user_id {
            let price = telnyx_price_usd_per_message();
            match crate::utils::usage::deduct_from_twilio_price(&state, uid, price) {
                Ok(cost) => {
                    tracing::info!(
                        "Telnyx deducted {:.4} credits for user {} (id: {})",
                        cost,
                        uid,
                        payload.id
                    );
                    if let Err(e) = state
                        .user_repository
                        .update_usage_log_credits(&payload.id, cost)
                    {
                        tracing::error!(
                            "Failed to update usage log credits for id {}: {}",
                            payload.id,
                            e
                        );
                    }
                }
                Err(e) => {
                    tracing::error!("Telnyx credit deduction failed for user {}: {}", uid, e)
                }
            }
        }
    }

    StatusCode::OK
}

/// Insert a new `message_status_log` row, or update the existing one if
/// `message_sid` already exists. Errors are logged, not returned: status
/// webhooks must always 2xx so Telnyx doesn't retry.
fn upsert_status_row(
    state: &Arc<AppState>,
    sid: &str,
    user_id: i32,
    to_number: &str,
    status: &str,
) {
    let mut conn = match state.pg_pool.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Telnyx status: db pool get failed: {}", e);
            return;
        }
    };

    let now = Utc::now().timestamp() as i32;
    let new_row = NewMessageStatusLog {
        message_sid: sid.to_string(),
        user_id,
        direction: "outbound".to_string(),
        to_number: to_number.to_string(),
        from_number: std::env::var("TELNYX_US_FROM_NUMBER").ok(),
        status: status.to_string(),
        error_code: None,
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
            message_status_log::updated_at.eq(now),
        ))
        .execute(&mut conn);

    match result {
        Ok(n) => tracing::debug!("Telnyx status: upserted {} row(s) for sid {}", n, sid),
        Err(e) => tracing::error!("Telnyx status: upsert failed for sid {}: {}", sid, e),
    }
}
