//! Webhook-triggered SMS to the authenticated user's own phone number.
//!
//! Three management endpoints (JWT-authed) let the user mint, list, and
//! revoke tokens. One public endpoint (`POST /api/webhook/sms`) accepts
//! `Authorization: Bearer lf_<hex>` and sends a single SMS to the
//! owner's `phone_number` via the existing channel router.
//!
//! Security shape:
//! - Token format is `lf_` + 32 hex chars (128 bits of entropy from
//!   the thread CSPRNG). DB stores only the SHA-256 hash; raw is shown
//!   once.
//! - The send destination is hardcoded to the owning user's
//!   `phone_number` — a leaked token cannot become an open SMS relay.
//! - Cap enforcement is a single atomic UPDATE (see
//!   `WebhookTokensRepository::claim_send_slot`), so concurrent requests
//!   cannot both pass the boundary.
//! - All credit / tier / phone-service-active gating reuses
//!   `check_user_credits`, so this path has the same abuse posture as
//!   every other outbound SMS.

use crate::handlers::auth_middleware::AuthUser;
use crate::models::user_models::NewWebhookToken;
use crate::repositories::webhook_tokens_repository::{ClaimResult, IdempotencyResult};
use crate::{AppState, UserCoreOps};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Twilio SMS bodies max out at 1600 chars (10 concatenated segments).
/// We reserve room for two prefixes the body picks up downstream:
///
/// - The `[label] ` provenance prefix added here (≤67 chars: label cap
///   is 64 + brackets + space).
/// - The `[Lightfriend backup] ` prefix prepended by the channel router
///   on fallback (21 chars).
///
/// 1600 - 67 - 21 = 1512; cap at 1500 to leave a tiny slack and keep the
/// boundary number easy to remember.
const MAX_BODY_LEN: usize = 1500;
const DEFAULT_DAILY_CAP: i32 = 50;
const TOKEN_PREFIX: &str = "lf_";
const TOKEN_BYTES: usize = 16; // 128 bits, rendered as 32 hex chars
/// Header name used by the public webhook endpoint to dedupe retried
/// requests. Stripe-style: client sends `Idempotency-Key: <opaque>`
/// and we replay the cached response for any second hit within 24h.
const IDEMPOTENCY_HEADER: &str = "Idempotency-Key";
const MAX_IDEMPOTENCY_KEY_LEN: usize = 64;

// ============================================================================
// Management endpoints (JWT-authed, mounted under protected_routes)
// ============================================================================

#[derive(Deserialize)]
pub struct CreateTokenRequest {
    pub label: String,
    /// Optional daily cap override. Clamped to [1, 500].
    pub daily_cap: Option<i32>,
}

#[derive(Serialize)]
pub struct CreateTokenResponse {
    pub id: i32,
    pub label: String,
    pub token_prefix: String,
    pub daily_cap: i32,
    /// Plaintext token. Shown to the user exactly once; the DB stores
    /// only the SHA-256 hash. Subsequent GETs never expose this field.
    pub token: String,
    pub created_at: i32,
}

#[derive(Serialize)]
pub struct WebhookTokenSummary {
    pub id: i32,
    pub label: String,
    pub token_prefix: String,
    pub daily_cap: i32,
    pub daily_sent: i32,
    pub daily_reset_at: i32,
    pub last_used_at: Option<i32>,
    pub created_at: i32,
}

pub async fn create_token(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<CreateTokenRequest>,
) -> Result<Json<CreateTokenResponse>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;

    // Gate on tier 2 — same bar as every other outbound path. We check
    // here rather than relying on `check_user_credits` later so the user
    // can't create a token they can never use.
    let user = state.user_core.find_by_id(user_id).map_err(db_err)?;
    let user = user.ok_or_else(|| not_found("User not found"))?;
    if user.sub_tier.as_deref() != Some("tier 2") {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Active subscription required"})),
        ));
    }

    let label = req.label.trim();
    if label.is_empty() || label.len() > 64 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "label must be 1..=64 chars"})),
        ));
    }

    let daily_cap = req.daily_cap.unwrap_or(DEFAULT_DAILY_CAP).clamp(1, 500);

    // Mint token: 16 random bytes (128 bits) → hex → prefix.
    let raw_token = generate_token();
    let token_hash = hash_token(&raw_token);
    let token_prefix = raw_token.chars().take(8).collect::<String>(); // e.g. "lf_abc12"

    let now = now_unix();
    let row = NewWebhookToken {
        user_id,
        token_hash,
        token_prefix: token_prefix.clone(),
        label: label.to_string(),
        daily_cap,
        daily_sent: 0,
        daily_reset_at: next_utc_midnight(now),
        created_at: now,
    };

    let inserted = state
        .webhook_tokens_repository
        .create(&row)
        .map_err(db_err)?;

    Ok(Json(CreateTokenResponse {
        id: inserted.id,
        label: inserted.label,
        token_prefix: inserted.token_prefix,
        daily_cap: inserted.daily_cap,
        token: raw_token,
        created_at: inserted.created_at,
    }))
}

pub async fn list_tokens(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<WebhookTokenSummary>>, (StatusCode, Json<serde_json::Value>)> {
    let rows = state
        .webhook_tokens_repository
        .list_for_user(auth_user.user_id)
        .map_err(db_err)?;
    let out = rows
        .into_iter()
        .map(|r| WebhookTokenSummary {
            id: r.id,
            label: r.label,
            token_prefix: r.token_prefix,
            daily_cap: r.daily_cap,
            daily_sent: r.daily_sent,
            daily_reset_at: r.daily_reset_at,
            last_used_at: r.last_used_at,
            created_at: r.created_at,
        })
        .collect();
    Ok(Json(out))
}

pub async fn revoke_token(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(token_id): Path<i32>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let revoked = state
        .webhook_tokens_repository
        .revoke(auth_user.user_id, token_id)
        .map_err(db_err)?;
    if !revoked {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "token not found"})),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Public webhook endpoint (Bearer-authed, mounted as a standalone route)
// ============================================================================

#[derive(Deserialize)]
pub struct WebhookSmsRequest {
    pub message: String,
}

#[derive(Serialize)]
pub struct WebhookSmsResponse {
    pub status: &'static str,
    pub sid: String,
}

/// `POST /api/webhook/sms`
///
/// Bearer-authenticated; sends one SMS to the owning user's phone.
/// Supports `Idempotency-Key` header so retried requests don't duplicate
/// the SMS.
///
/// Failure modes:
/// - 401: missing/malformed/unknown/revoked bearer
/// - 400: empty/oversized body, or malformed Idempotency-Key
/// - 402: insufficient credits / inactive subscription / phone-service-off
///   (collapsed into 402 so the client sees a single "fix billing" code)
/// - 409: a prior request with this Idempotency-Key is still in flight
/// - 429: daily cap exhausted
/// - 502: provider failure
pub async fn webhook_sms(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<WebhookSmsRequest>,
) -> Result<Json<WebhookSmsResponse>, (StatusCode, Json<serde_json::Value>)> {
    // 1. Extract bearer.
    let raw = extract_bearer(&headers).ok_or_else(unauthorized)?;
    if !raw.starts_with(TOKEN_PREFIX) {
        return Err(unauthorized());
    }

    // 2. Hash and look up. We do NOT distinguish missing-row from
    //    revoked-row in the response to keep the 401 path uniform.
    let token_hash = hash_token(raw);
    let token_row = state
        .webhook_tokens_repository
        .find_by_hash(&token_hash)
        .map_err(db_err)?
        .ok_or_else(unauthorized)?;
    if token_row.revoked_at.is_some() {
        return Err(unauthorized());
    }

    // 3. Validate body before doing any send-side work.
    let body = req.message.trim();
    if body.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "message must not be empty"})),
        ));
    }
    if body.len() > MAX_BODY_LEN {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("message exceeds {} chars", MAX_BODY_LEN),
            })),
        ));
    }
    let body = body.to_string();

    // 4. Idempotency-Key handling (optional). Resolved here, BEFORE the
    //    credit check and cap claim, so a replay short-circuits without
    //    burning the daily quota or the credit-warning side effects in
    //    `check_user_credits`.
    let idempotency_key = extract_idempotency_key(&headers)?.unwrap_or_default();
    let idempotency_row_id: Option<i32> = if !idempotency_key.is_empty() {
        let res = state
            .webhook_tokens_repository
            .reserve_idempotency_key(token_row.id, &idempotency_key)
            .map_err(db_err)?;
        match res {
            IdempotencyResult::Replayed { sid } => {
                return Ok(Json(WebhookSmsResponse {
                    status: "sent",
                    sid,
                }));
            }
            IdempotencyResult::InFlight => {
                return Err((
                    StatusCode::CONFLICT,
                    Json(json!({"error": "idempotency key in flight"})),
                ));
            }
            IdempotencyResult::Fresh { id } => Some(id),
        }
    } else {
        None
    };

    // 5. Load user. The token row references a real user via FK, but the
    //    user could be soft-deleted between token mint and now.
    let user = match state
        .user_core
        .find_by_id(token_row.user_id)
        .map_err(db_err)?
    {
        Some(u) => u,
        None => {
            release_idempotency(&state, idempotency_row_id);
            return Err(unauthorized());
        }
    };

    // 6. Credit / tier / phone-service-active gate. Same call all other
    //    outbound paths use — keeps abuse posture identical and reuses
    //    the existing "out of credits" SMS warning behavior.
    if let Err(msg) = crate::utils::usage::check_user_credits(&state, &user, "noti_msg", None).await
    {
        release_idempotency(&state, idempotency_row_id);
        return Err((StatusCode::PAYMENT_REQUIRED, Json(json!({"error": msg}))));
    }

    // 7. Atomic daily-cap claim.
    let claim = match state.webhook_tokens_repository.claim_send_slot(&token_hash) {
        Ok(c) => c,
        Err(e) => {
            release_idempotency(&state, idempotency_row_id);
            return Err(db_err(e));
        }
    };
    let token = match claim {
        ClaimResult::Ok { token } => token,
        ClaimResult::Revoked => {
            release_idempotency(&state, idempotency_row_id);
            return Err(unauthorized());
        }
        ClaimResult::OverCap {
            daily_cap,
            daily_sent,
        } => {
            release_idempotency(&state, idempotency_row_id);
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({
                    "error": "daily cap reached",
                    "daily_cap": daily_cap,
                    "daily_sent": daily_sent,
                })),
            ));
        }
    };

    // 8. Prepend provenance so a leaked token cannot impersonate the AI
    //    assistant. The label is user-set and bounded to 64 chars at
    //    creation; we additionally sanitize for control characters so a
    //    malicious label can't fake a multi-message handoff.
    let safe_label = sanitize_label(&token.label);
    let outbound = format!("[{}] {}", safe_label, body);

    // 9. Send. The slot is already consumed; we count attempts, not
    //    successes, so a provider failure still burns one of the daily
    //    quota. That keeps the cap from being a free retry loop on
    //    provider outages. Idempotency rows ARE released on failure so
    //    the client can retry with the same key.
    match state
        .channel_router
        .send_to_user(&user, &outbound, None)
        .await
    {
        Ok(sid) => {
            let sid = sid.into_inner();
            if let Some(id) = idempotency_row_id {
                if let Err(e) = state
                    .webhook_tokens_repository
                    .complete_idempotency(id, &sid)
                {
                    tracing::error!("Failed to complete idempotency row {}: {}", id, e);
                }
            }
            if let Err(e) = state.user_repository.log_usage(
                crate::repositories::user_repository::LogUsageParams {
                    user_id: user.id,
                    sid: Some(sid.clone()),
                    activity_type: "webhook_sms".to_string(),
                    credits: None,
                    time_consumed: None,
                    success: Some(true),
                    reason: Some(format!("token_id={}", token.id)),
                    status: Some("sent".to_string()),
                    recharge_threshold_timestamp: None,
                    zero_credits_timestamp: None,
                },
            ) {
                tracing::error!("Failed to log webhook_sms usage: {}", e);
            }
            Ok(Json(WebhookSmsResponse {
                status: "sent",
                sid,
            }))
        }
        Err(e) => {
            tracing::error!(
                "webhook_sms send failed for user {} token {}: {}",
                user.id,
                token.id,
                e
            );
            release_idempotency(&state, idempotency_row_id);
            Err((
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "send failed"})),
            ))
        }
    }
}

// ============================================================================
// Internal helpers
// ============================================================================

fn extract_bearer(headers: &HeaderMap) -> Option<&str> {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    raw.strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Pull and validate an optional `Idempotency-Key` header.
///
/// - Missing header → Ok(None) (idempotency disabled for this request)
/// - Present but malformed → Err 400
///
/// Accepts ASCII printable chars only, length 1..=64. That covers UUIDs,
/// ULIDs, and human-typed keys without giving the client room to smuggle
/// nulls or control characters into a downstream log line.
fn extract_idempotency_key(
    headers: &HeaderMap,
) -> Result<Option<String>, (StatusCode, Json<serde_json::Value>)> {
    let Some(raw) = headers.get(IDEMPOTENCY_HEADER) else {
        return Ok(None);
    };
    let s = raw.to_str().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Idempotency-Key contains non-ASCII bytes"})),
        )
    })?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        // Header explicitly set to empty/whitespace — treat as absent.
        return Ok(None);
    }
    if trimmed.len() > MAX_IDEMPOTENCY_KEY_LEN {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!(
                    "Idempotency-Key exceeds {} chars",
                    MAX_IDEMPOTENCY_KEY_LEN
                ),
            })),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_graphic() || c == ' ' || c == '-' || c == '_')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Idempotency-Key contains invalid characters",
            })),
        ));
    }
    Ok(Some(trimmed.to_string()))
}

/// Best-effort cleanup of an in-flight idempotency row when the request
/// fails before completing. Errors are logged but not propagated — the
/// row will naturally expire after 24h and treat the next retry as fresh.
fn release_idempotency(state: &Arc<AppState>, row_id: Option<i32>) {
    if let Some(id) = row_id {
        if let Err(e) = state.webhook_tokens_repository.clear_idempotency(id) {
            tracing::error!("Failed to clear in-flight idempotency row {}: {}", id, e);
        }
    }
}

fn generate_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("{}{}", TOKEN_PREFIX, hex::encode(bytes))
}

fn hash_token(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}

fn now_unix() -> i32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32
}

fn next_utc_midnight(now: i32) -> i32 {
    let day = 86_400;
    ((now / day) + 1) * day
}

fn unauthorized() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": "invalid or revoked token"})),
    )
}

fn db_err(e: diesel::result::Error) -> (StatusCode, Json<serde_json::Value>) {
    tracing::error!("webhook_sms db error: {}", e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "internal error"})),
    )
}

fn not_found(msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::NOT_FOUND, Json(json!({"error": msg})))
}

/// Strip control characters from the label so it can't break out of the
/// `[label]` prefix on display. Already length-bounded at creation; we
/// only need to defuse newlines and other formatting tricks.
fn sanitize_label(label: &str) -> String {
    let cleaned: String = label
        .chars()
        .filter(|c| !c.is_control() && *c != '[' && *c != ']')
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "webhook".to_string()
    } else {
        cleaned.to_string()
    }
}
