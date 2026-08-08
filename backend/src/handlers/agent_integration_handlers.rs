//! Write-only API for local AI-agent clients.
//!
//! This boundary intentionally has no read/list/search endpoint. A credential
//! can only create a one-shot reminder or arm a sender-scoped email reply
//! watch. Responses never contain reminder text, contacts, message content, or
//! provider identifiers.

use crate::handlers::auth_middleware::AuthUser;
use crate::models::ontology_models::NewOntEvent;
use crate::repositories::agent_integration_repository::{
    AgentIntegrationRepository, CredentialClaim, CredentialIssue, IdempotencyClaim, PairingPoll,
};
use crate::{AppState, UserCoreOps};
use axum::extract::{OriginalUri, Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use governor::{Quota, RateLimiter};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::num::NonZeroU32;
use std::sync::Arc;

const DEVICE_PREFIX: &str = "lfpair_";
const TOKEN_PREFIX: &str = "lfagent_";
const PAIRING_TTL_SECONDS: i32 = 10 * 60;
const CREDENTIAL_TTL_SECONDS: i32 = 90 * 24 * 60 * 60;
const MAX_REMINDER_SECONDS: i64 = 365 * 24 * 60 * 60;
const MAX_ACTIVE_REPLY_WATCHES: i64 = 5;
const IDEMPOTENCY_HEADER: &str = "idempotency-key";

#[derive(Deserialize)]
pub struct StartPairingRequest {
    #[serde(default = "default_client_name")]
    client_name: String,
}

#[derive(Serialize)]
pub struct StartPairingResponse {
    status: &'static str,
    device_code: String,
    user_code: String,
    verification_path: &'static str,
    expires_in: i32,
    poll_interval: i32,
}

#[derive(Deserialize)]
pub struct PollPairingRequest {
    device_code: String,
}

#[derive(Serialize)]
pub struct PollPairingResponse {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<i32>,
}

#[derive(Deserialize)]
pub struct ApprovePairingRequest {
    user_code: String,
}

#[derive(Serialize)]
pub struct CredentialSummary {
    id: i32,
    label: String,
    token_prefix: String,
    scopes: Vec<&'static str>,
    daily_cap: i32,
    daily_used: i32,
    expires_at: i32,
    created_at: i32,
    last_used_at: Option<i32>,
}

#[derive(Deserialize)]
pub struct CreateReminderRequest {
    message: String,
    at: String,
}

#[derive(Deserialize)]
pub struct CreateReplyWatchRequest {
    email: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default = "default_watch_seconds")]
    expires_in_seconds: i32,
}

#[derive(Serialize)]
pub struct ActionResponse {
    status: &'static str,
}

pub async fn start_pairing(
    State(state): State<Arc<AppState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Json(request): Json<StartPairingRequest>,
) -> Response {
    if uri.query().is_some() {
        return minimal(StatusCode::BAD_REQUEST, "rejected");
    }
    if !check_rate_limit(&state, &headers, "agent-pairing-start", 6) {
        return minimal(StatusCode::TOO_MANY_REQUESTS, "rejected");
    }
    let Some(client_name) = printable(&request.client_name, 64) else {
        return minimal(StatusCode::BAD_REQUEST, "rejected");
    };
    let device_code = format!("{DEVICE_PREFIX}{}", random_hex(32));
    let user_code = random_user_code();
    let now = now_unix();
    let repository = AgentIntegrationRepository::new(state.pg_pool.clone());
    if let Err(error) = repository.create_pairing(
        &hash_secret(&device_code),
        &hash_secret(&normalize_user_code(&user_code)),
        &client_name,
        now,
        now + PAIRING_TTL_SECONDS,
    ) {
        tracing::error!(error = %error, "agent pairing creation failed");
        return minimal(StatusCode::INTERNAL_SERVER_ERROR, "failed");
    }
    no_store(
        Json(StartPairingResponse {
            status: "accepted",
            device_code,
            user_code,
            verification_path: "/",
            expires_in: PAIRING_TTL_SECONDS,
            poll_interval: 3,
        })
        .into_response(),
    )
}

pub async fn poll_pairing(
    State(state): State<Arc<AppState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Json(request): Json<PollPairingRequest>,
) -> Response {
    if uri.query().is_some() || !valid_prefixed_hex(&request.device_code, DEVICE_PREFIX, 64) {
        return minimal(StatusCode::UNAUTHORIZED, "rejected");
    }
    if !check_rate_limit(&state, &headers, "agent-pairing-poll", 30) {
        return minimal(StatusCode::TOO_MANY_REQUESTS, "rejected");
    }
    let now = now_unix();
    let device_hash = hash_secret(&request.device_code);
    let repository = AgentIntegrationRepository::new(state.pg_pool.clone());
    match repository.poll_pairing(&device_hash, now) {
        Ok(PairingPoll::Pending) => no_store(
            (
                StatusCode::ACCEPTED,
                Json(PollPairingResponse {
                    status: "pending",
                    token: None,
                    expires_at: None,
                }),
            )
                .into_response(),
        ),
        Ok(PairingPoll::Approved { user_id, label }) => {
            if !has_active_subscription(&state, user_id) {
                return minimal(StatusCode::FORBIDDEN, "rejected");
            }
            let raw_token = format!("{TOKEN_PREFIX}{}", random_hex(32));
            let token_prefix = raw_token.chars().take(16).collect::<String>();
            let expires_at = now + CREDENTIAL_TTL_SECONDS;
            match repository.consume_pairing(
                &device_hash,
                CredentialIssue {
                    user_id,
                    token_hash: &hash_secret(&raw_token),
                    token_prefix: &token_prefix,
                    label: &label,
                    issued_at: now,
                    expires_at,
                },
            ) {
                Ok(Some(_)) => no_store(
                    Json(PollPairingResponse {
                        status: "accepted",
                        token: Some(raw_token),
                        expires_at: Some(expires_at),
                    })
                    .into_response(),
                ),
                Ok(None) => minimal(StatusCode::CONFLICT, "rejected"),
                Err(error) => {
                    tracing::error!(error = %error, "agent pairing consumption failed");
                    minimal(StatusCode::INTERNAL_SERVER_ERROR, "failed")
                }
            }
        }
        Ok(PairingPoll::Invalid) => minimal(StatusCode::UNAUTHORIZED, "rejected"),
        Err(error) => {
            tracing::error!(error = %error, "agent pairing poll failed");
            minimal(StatusCode::INTERNAL_SERVER_ERROR, "failed")
        }
    }
}

pub async fn approve_pairing(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<ApprovePairingRequest>,
) -> Response {
    if !has_active_subscription(&state, auth_user.user_id) {
        return minimal(StatusCode::FORBIDDEN, "rejected");
    }
    let normalized = normalize_user_code(&request.user_code);
    if !valid_user_code(&normalized) {
        return minimal(StatusCode::BAD_REQUEST, "rejected");
    }
    let repository = AgentIntegrationRepository::new(state.pg_pool.clone());
    match repository.approve_pairing(auth_user.user_id, &hash_secret(&normalized), now_unix()) {
        Ok(Some(_)) => minimal(StatusCode::OK, "accepted"),
        Ok(None) => minimal(StatusCode::BAD_REQUEST, "rejected"),
        Err(error) => {
            tracing::error!(error = %error, "agent pairing approval failed");
            minimal(StatusCode::INTERNAL_SERVER_ERROR, "failed")
        }
    }
}

pub async fn list_credentials(State(state): State<Arc<AppState>>, auth_user: AuthUser) -> Response {
    let repository = AgentIntegrationRepository::new(state.pg_pool.clone());
    match repository.list_credentials(auth_user.user_id) {
        Ok(credentials) => no_store(
            Json(
                credentials
                    .into_iter()
                    .map(|credential| CredentialSummary {
                        id: credential.id,
                        label: credential.label,
                        token_prefix: credential.token_prefix,
                        scopes: vec!["create reminders", "watch email replies"],
                        daily_cap: credential.daily_cap,
                        daily_used: credential.daily_used,
                        expires_at: credential.expires_at,
                        created_at: credential.created_at,
                        last_used_at: credential.last_used_at,
                    })
                    .collect::<Vec<_>>(),
            )
            .into_response(),
        ),
        Err(error) => {
            tracing::error!(error = %error, "agent credential list failed");
            minimal(StatusCode::INTERNAL_SERVER_ERROR, "failed")
        }
    }
}

pub async fn revoke_credential(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(id): Path<i32>,
) -> Response {
    let repository = AgentIntegrationRepository::new(state.pg_pool.clone());
    match repository.revoke_credential(auth_user.user_id, id, now_unix()) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => minimal(StatusCode::NOT_FOUND, "rejected"),
        Err(error) => {
            tracing::error!(error = %error, "agent credential revocation failed");
            minimal(StatusCode::INTERNAL_SERVER_ERROR, "failed")
        }
    }
}

pub async fn revoke_current_credential(
    State(state): State<Arc<AppState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    if uri.query().is_some() {
        return minimal(StatusCode::BAD_REQUEST, "rejected");
    }
    let Some(raw_token) =
        bearer(&headers).filter(|token| valid_prefixed_hex(token, TOKEN_PREFIX, 64))
    else {
        return minimal(StatusCode::UNAUTHORIZED, "rejected");
    };
    let repository = AgentIntegrationRepository::new(state.pg_pool.clone());
    match repository.revoke_by_token_hash(&hash_secret(raw_token), now_unix()) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => minimal(StatusCode::UNAUTHORIZED, "rejected"),
        Err(error) => {
            tracing::error!(error = %error, "agent self-revocation failed");
            minimal(StatusCode::INTERNAL_SERVER_ERROR, "failed")
        }
    }
}

pub async fn create_reminder(
    State(state): State<Arc<AppState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Json(request): Json<CreateReminderRequest>,
) -> Response {
    if uri.query().is_some() {
        return minimal(StatusCode::BAD_REQUEST, "rejected");
    }
    let Some(message) = printable(&request.message, 280) else {
        return minimal(StatusCode::BAD_REQUEST, "rejected");
    };
    let Ok(parsed_at) = chrono::DateTime::parse_from_rfc3339(request.at.trim()) else {
        return minimal(StatusCode::BAD_REQUEST, "rejected");
    };
    let now = now_unix();
    let remind_at = parsed_at.timestamp();
    if remind_at < i64::from(now) + 60 || remind_at > i64::from(now) + MAX_REMINDER_SECONDS {
        return minimal(StatusCode::BAD_REQUEST, "rejected");
    }
    let Some(idempotency_key) = idempotency_key(&headers) else {
        return minimal(StatusCode::BAD_REQUEST, "rejected");
    };
    let Some(raw_token) =
        bearer(&headers).filter(|token| valid_prefixed_hex(token, TOKEN_PREFIX, 64))
    else {
        return minimal(StatusCode::UNAUTHORIZED, "rejected");
    };
    let repository = AgentIntegrationRepository::new(state.pg_pool.clone());
    let token_hash = hash_secret(raw_token);
    let Some(preflight) = repository
        .authenticate_credential(&token_hash, "reminders", now)
        .ok()
        .flatten()
    else {
        return minimal(StatusCode::UNAUTHORIZED, "rejected");
    };
    let reservation = match repository.reserve_idempotency(
        preflight.id,
        "reminder",
        &hash_secret(&idempotency_key),
        now,
    ) {
        Ok(IdempotencyClaim::Replayed(outcome)) => {
            return minimal(StatusCode::OK, static_outcome(&outcome));
        }
        Ok(IdempotencyClaim::InFlight) => return minimal(StatusCode::CONFLICT, "rejected"),
        Ok(IdempotencyClaim::Fresh(id)) => id,
        Err(error) => {
            tracing::error!(error = %error, "agent reminder idempotency failed");
            return minimal(StatusCode::INTERNAL_SERVER_ERROR, "failed");
        }
    };
    let credential = match repository.claim_credential(&token_hash, "reminders", now) {
        Ok(CredentialClaim::Accepted(credential)) if credential.id == preflight.id => credential,
        Ok(CredentialClaim::OverCap) => {
            let _ = repository.clear_idempotency(reservation);
            return minimal(StatusCode::TOO_MANY_REQUESTS, "rejected");
        }
        _ => {
            let _ = repository.clear_idempotency(reservation);
            return minimal(StatusCode::UNAUTHORIZED, "rejected");
        }
    };
    if !has_active_subscription(&state, credential.user_id) {
        let _ = repository.complete_idempotency(reservation, "rejected");
        let _ = repository.audit(
            credential.id,
            credential.user_id,
            "reminder",
            "rejected",
            now,
        );
        return minimal(StatusCode::FORBIDDEN, "rejected");
    }
    let event = NewOntEvent {
        user_id: credential.user_id,
        description: message,
        remind_at: Some(remind_at as i32),
        due_at: Some(remind_at as i32),
        status: "active".to_string(),
        created_at: now,
        updated_at: now,
    };
    match state.ontology_repository.create_reminder(&event, "UTC") {
        Ok(_) => finish_action(
            &repository,
            reservation,
            &credential,
            "reminder",
            "accepted",
            now,
        ),
        Err(error) => {
            tracing::error!(credential_id = credential.id, error = %error, "agent reminder create failed");
            let _ = repository.clear_idempotency(reservation);
            let _ = repository.audit(credential.id, credential.user_id, "reminder", "failed", now);
            minimal(StatusCode::INTERNAL_SERVER_ERROR, "failed")
        }
    }
}

pub async fn create_reply_watch(
    State(state): State<Arc<AppState>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Json(request): Json<CreateReplyWatchRequest>,
) -> Response {
    if uri.query().is_some() {
        return minimal(StatusCode::BAD_REQUEST, "rejected");
    }
    let email = request.email.trim().to_ascii_lowercase();
    if !valid_email(&email) || !(900..=86_400).contains(&request.expires_in_seconds) {
        return minimal(StatusCode::BAD_REQUEST, "rejected");
    }
    let label = match request.label.as_deref() {
        Some(label) => match printable(label, 64) {
            Some(label) => label,
            None => return minimal(StatusCode::BAD_REQUEST, "rejected"),
        },
        None => email.clone(),
    };
    let Some(idempotency_key) = idempotency_key(&headers) else {
        return minimal(StatusCode::BAD_REQUEST, "rejected");
    };
    let Some(raw_token) =
        bearer(&headers).filter(|token| valid_prefixed_hex(token, TOKEN_PREFIX, 64))
    else {
        return minimal(StatusCode::UNAUTHORIZED, "rejected");
    };
    let now = now_unix();
    let repository = AgentIntegrationRepository::new(state.pg_pool.clone());
    let token_hash = hash_secret(raw_token);
    let Some(preflight) = repository
        .authenticate_credential(&token_hash, "reply_watch_email", now)
        .ok()
        .flatten()
    else {
        return minimal(StatusCode::UNAUTHORIZED, "rejected");
    };
    let reservation = match repository.reserve_idempotency(
        preflight.id,
        "reply_watch_email",
        &hash_secret(&idempotency_key),
        now,
    ) {
        Ok(IdempotencyClaim::Replayed(outcome)) => {
            return minimal(StatusCode::OK, static_outcome(&outcome));
        }
        Ok(IdempotencyClaim::InFlight) => return minimal(StatusCode::CONFLICT, "rejected"),
        Ok(IdempotencyClaim::Fresh(id)) => id,
        Err(error) => {
            tracing::error!(error = %error, "agent reply-watch idempotency failed");
            return minimal(StatusCode::INTERNAL_SERVER_ERROR, "failed");
        }
    };
    let credential = match repository.claim_credential(&token_hash, "reply_watch_email", now) {
        Ok(CredentialClaim::Accepted(credential)) if credential.id == preflight.id => credential,
        Ok(CredentialClaim::OverCap) => {
            let _ = repository.clear_idempotency(reservation);
            return minimal(StatusCode::TOO_MANY_REQUESTS, "rejected");
        }
        _ => {
            let _ = repository.clear_idempotency(reservation);
            return minimal(StatusCode::UNAUTHORIZED, "rejected");
        }
    };
    if !has_active_subscription(&state, credential.user_id) {
        let _ = repository.complete_idempotency(reservation, "rejected");
        let _ = repository.audit(
            credential.id,
            credential.user_id,
            "reply_watch_email",
            "rejected",
            now,
        );
        return minimal(StatusCode::FORBIDDEN, "rejected");
    }
    let connection_ids = repository
        .active_imap_connection_ids(credential.user_id)
        .unwrap_or_default();
    let expires_at = now + request.expires_in_seconds;
    match repository.create_email_reply_watches(
        credential.user_id,
        &connection_ids,
        &email,
        &label,
        now,
        expires_at,
        MAX_ACTIVE_REPLY_WATCHES,
    ) {
        Ok(false) => {
            let _ = repository.complete_idempotency(reservation, "rejected");
            let _ = repository.audit(
                credential.id,
                credential.user_id,
                "reply_watch_email",
                "rejected",
                now,
            );
            return minimal(StatusCode::CONFLICT, "rejected");
        }
        Err(error) => {
            tracing::error!(credential_id = credential.id, error = %error, "agent reply watch create failed");
            let _ = repository.clear_idempotency(reservation);
            let _ = repository.audit(
                credential.id,
                credential.user_id,
                "reply_watch_email",
                "failed",
                now,
            );
            return minimal(StatusCode::INTERNAL_SERVER_ERROR, "failed");
        }
        Ok(true) => {}
    }
    finish_action(
        &repository,
        reservation,
        &credential,
        "reply_watch_email",
        "accepted",
        now,
    )
}

fn finish_action(
    repository: &AgentIntegrationRepository,
    reservation: i32,
    credential: &crate::models::agent_integration_models::AgentCredential,
    action_kind: &str,
    outcome: &'static str,
    now: i32,
) -> Response {
    if let Err(error) = repository.complete_idempotency(reservation, outcome) {
        tracing::warn!(error = %error, "agent idempotency completion failed");
    }
    if let Err(error) =
        repository.audit(credential.id, credential.user_id, action_kind, outcome, now)
    {
        tracing::warn!(error = %error, "agent action audit failed");
    }
    minimal(StatusCode::OK, outcome)
}

fn has_active_subscription(state: &Arc<AppState>, user_id: i32) -> bool {
    matches!(
        state.user_core.find_by_id(user_id),
        Ok(Some(user)) if user.sub_tier.as_deref() == Some("tier 2")
    )
}

fn check_rate_limit(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    namespace: &str,
    per_minute: u32,
) -> bool {
    let client = headers
        .get("cf-connecting-ip")
        .or_else(|| headers.get("x-real-ip"))
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(',').next().unwrap_or(value).trim())
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    let key = format!("{namespace}:{client}");
    let quota = Quota::per_minute(NonZeroU32::new(per_minute).unwrap())
        .allow_burst(NonZeroU32::new(per_minute.min(6)).unwrap());
    let limiter = state
        .api_rate_limiter
        .entry(key.clone())
        .or_insert_with(|| RateLimiter::keyed(quota));
    limiter.check_key(&key).is_ok()
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

fn minimal(status: StatusCode, outcome: &'static str) -> Response {
    no_store((status, Json(ActionResponse { status: outcome })).into_response())
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn idempotency_key(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(IDEMPOTENCY_HEADER)?.to_str().ok()?.trim();
    if value.is_empty() || value.len() > 64 || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
        None
    } else {
        Some(value.to_string())
    }
}

fn valid_prefixed_hex(value: &str, prefix: &str, hex_len: usize) -> bool {
    let Some(hex) = value.strip_prefix(prefix) else {
        return false;
    };
    hex.len() == hex_len
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn printable(value: &str, max_chars: usize) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_chars || value.chars().any(char::is_control)
    {
        None
    } else {
        Some(value.to_string())
    }
}

fn valid_email(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 254
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return false;
    }
    let mut pieces = value.split('@');
    let (Some(local), Some(domain), None) = (pieces.next(), pieces.next(), pieces.next()) else {
        return false;
    };
    !local.is_empty()
        && local.len() <= 64
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
}

fn normalize_user_code(value: &str) -> String {
    value
        .chars()
        .filter(|character| *character != '-' && !character.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect()
}

fn valid_user_code(value: &str) -> bool {
    const ALPHABET: &str = "23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    value.len() == 12 && value.chars().all(|character| ALPHABET.contains(character))
}

fn random_user_code() -> String {
    const ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut random = [0_u8; 12];
    OsRng.fill_bytes(&mut random);
    let raw: String = random
        .into_iter()
        .map(|byte| ALPHABET[(byte & 31) as usize] as char)
        .collect();
    format!("{}-{}-{}", &raw[0..4], &raw[4..8], &raw[8..12])
}

fn random_hex(bytes: usize) -> String {
    let mut random = vec![0_u8; bytes];
    OsRng.fill_bytes(&mut random);
    hex::encode(random)
}

fn hash_secret(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn now_unix() -> i32 {
    chrono::Utc::now().timestamp() as i32
}

fn static_outcome(outcome: &str) -> &'static str {
    match outcome {
        "accepted" => "accepted",
        "rejected" => "rejected",
        _ => "failed",
    }
}

fn default_client_name() -> String {
    "Local AI agent".to_string()
}

fn default_watch_seconds() -> i32 {
    24 * 60 * 60
}
