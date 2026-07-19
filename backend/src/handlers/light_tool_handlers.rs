use crate::{
    handlers::{
        auth_middleware::AuthUser,
        light_tool_auth::{optional_bearer, LightToolDeviceAuth},
    },
    repositories::{
        light_tool_devices_repository::LightToolDevicesRepository,
        light_tool_pairing_repository::{LightToolPairingRepository, PairingConsumption},
        light_tool_push_repository::LightToolPushRepository,
        light_tool_runs_repository::{
            AccountRunCreation, AnonymousTrialRunCreation, LightToolRunRecord,
            LightToolRunsRepository, LightToolRunsRepositoryError,
        },
    },
    services::{
        light_tool_bootstrap::{
            LightToolBootstrapError, LightToolBootstrapService, TRIAL_MESSAGE_LIMIT,
        },
        light_tool_identity::LightToolIdentityError,
        light_tool_pairing::{
            LightToolPairingError, LightToolPairingService, LightToolPairingStatus,
        },
        light_tool_push_delivery::LightToolPushDeliveryService,
        light_tool_run_dispatcher::{dispatch_light_tool_run, LightToolDispatchOutcome},
    },
    AppState, UserCoreOps,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::SecondsFormat;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::IpAddr;
use std::sync::Arc;
use url::{Host, Url};
use uuid::Uuid;

const MAX_MESSAGE_TEXT_BYTES: usize = 8_000;
const MESSAGE_HISTORY_RUN_LIMIT: i64 = 50;
const MAX_PUSH_ENDPOINT_BYTES: usize = 2_048;

#[derive(Deserialize)]
pub struct BootstrapRequest {
    pub installation_id: String,
}

#[derive(Serialize)]
pub struct BootstrapResponse {
    pub device_token: String,
    pub can_send: bool,
    pub trial: TrialQuotaResponse,
    pub account: Option<ConnectedAccountResponse>,
}

#[derive(Serialize)]
pub struct TrialQuotaResponse {
    pub messages_remaining: i32,
    pub expires_at: String,
}

#[derive(Serialize)]
pub struct ConnectedAccountResponse {
    pub email: String,
}

#[derive(Serialize)]
pub struct PairingOfferResponse {
    pub pairing_uri: String,
    pub expires_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PairingStatusResponseKind {
    None,
    Pending,
    Connected,
    Expired,
}

#[derive(Serialize)]
pub struct PairingStatusResponse {
    pub status: PairingStatusResponseKind,
}

#[derive(Deserialize)]
pub struct ConsumePairingRequest {
    pub pairing_uri: String,
}

#[derive(Serialize)]
pub struct ConsumePairingResponse {
    pub account: ConnectedAccountResponse,
}

#[derive(Deserialize)]
pub struct RegisterPushEndpointRequest {
    pub endpoint: String,
}

#[derive(Deserialize)]
pub struct SendMessageRequest {
    pub client_message_id: String,
    pub text: String,
}

#[derive(Serialize)]
pub struct SendMessageResponse {
    pub run_id: String,
}

#[derive(Serialize)]
pub struct UserMessageResponse {
    pub id: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct MessageHistoryRunResponse {
    pub run_id: String,
    pub state: RunStateResponse,
    pub user_message: UserMessageResponse,
    pub activity_text: Option<String>,
    pub assistant_message: Option<AssistantMessageResponse>,
    pub error_message: Option<String>,
}

#[derive(Serialize)]
pub struct MessageHistoryResponse {
    pub runs: Vec<MessageHistoryRunResponse>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStateResponse {
    Queued,
    Running,
    Completed,
    Failed,
}

#[derive(Serialize)]
pub struct AssistantMessageResponse {
    pub id: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct RunStatusResponse {
    pub run_id: String,
    pub state: RunStateResponse,
    pub activity_text: Option<String>,
    pub assistant_message: Option<AssistantMessageResponse>,
    pub error_message: Option<String>,
}

type ApiError = (StatusCode, Json<Value>);

pub async fn bootstrap(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<BootstrapRequest>,
) -> Result<Json<BootstrapResponse>, ApiError> {
    if request.installation_id.len() > 64 {
        return Err(bad_request("installation_id must be a UUID"));
    }

    let existing_device_token = optional_bearer(&headers).map_err(|_| unauthorized())?;
    let service =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()));
    let now = chrono::Utc::now().timestamp() as i32;
    let session = service
        .bootstrap(&request.installation_id, existing_device_token, now)
        .map_err(map_bootstrap_error)?;
    let expires_at = chrono::DateTime::from_timestamp(session.trial_expires_at as i64, 0)
        .ok_or_else(|| internal_error("invalid trial expiration timestamp"))?
        .to_rfc3339_opts(SecondsFormat::Secs, true);

    let account = match session.account_user_id {
        Some(user_id) => {
            let user = state
                .user_core
                .find_by_id(user_id)
                .map_err(|error| {
                    tracing::error!(
                        user_id,
                        "Light Tool bootstrap account lookup failed: {error}"
                    );
                    internal_error("bootstrap failed")
                })?
                .ok_or_else(|| internal_error("connected account not found"))?;
            Some(ConnectedAccountResponse { email: user.email })
        }
        None => None,
    };

    Ok(Json(BootstrapResponse {
        device_token: session.device_token,
        can_send: session.can_send,
        trial: TrialQuotaResponse {
            messages_remaining: session.trial_messages_remaining,
            expires_at,
        },
        account,
    }))
}

pub async fn create_pairing_offer(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<PairingOfferResponse>, ApiError> {
    let now = chrono::Utc::now().timestamp() as i32;
    let offer =
        LightToolPairingService::new(LightToolPairingRepository::new(state.pg_pool.clone()))
            .create_offer(auth_user.user_id, now)
            .map_err(map_pairing_error)?;

    Ok(Json(PairingOfferResponse {
        pairing_uri: offer.pairing_uri,
        expires_at: format_timestamp(offer.expires_at)?,
    }))
}

pub async fn get_pairing_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<PairingStatusResponse>, ApiError> {
    let now = chrono::Utc::now().timestamp() as i32;
    let status =
        LightToolPairingService::new(LightToolPairingRepository::new(state.pg_pool.clone()))
            .status_for_user(auth_user.user_id, now)
            .map_err(map_pairing_error)?;
    let status = match status {
        LightToolPairingStatus::None => PairingStatusResponseKind::None,
        LightToolPairingStatus::Pending => PairingStatusResponseKind::Pending,
        LightToolPairingStatus::Connected => PairingStatusResponseKind::Connected,
        LightToolPairingStatus::Expired => PairingStatusResponseKind::Expired,
    };

    Ok(Json(PairingStatusResponse { status }))
}

pub async fn consume_pairing_offer(
    State(state): State<Arc<AppState>>,
    auth: LightToolDeviceAuth,
    Json(request): Json<ConsumePairingRequest>,
) -> Result<Json<ConsumePairingResponse>, ApiError> {
    let now = chrono::Utc::now().timestamp() as i32;
    let consumption =
        LightToolPairingService::new(LightToolPairingRepository::new(state.pg_pool.clone()))
            .consume_uri(request.pairing_uri.trim(), auth.device.id, now)
            .map_err(map_pairing_error)?;
    let user_id = match consumption {
        PairingConsumption::Linked { user_id, .. } => user_id,
        PairingConsumption::InvalidOrExpired => {
            return Err(bad_request("pairing code is invalid or expired"));
        }
        PairingConsumption::DeviceUnavailable => return Err(unauthorized()),
        PairingConsumption::DeviceAlreadyLinked => {
            return Err(conflict(
                "disconnect the current account before pairing another",
            ));
        }
    };
    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|error| {
            tracing::error!(user_id, "Paired Light Tool account lookup failed: {error}");
            internal_error("pairing failed")
        })?
        .ok_or_else(|| internal_error("paired account not found"))?;

    Ok(Json(ConsumePairingResponse {
        account: ConnectedAccountResponse { email: user.email },
    }))
}

pub async fn disconnect_account(
    State(state): State<Arc<AppState>>,
    auth: LightToolDeviceAuth,
) -> Result<StatusCode, ApiError> {
    let now = chrono::Utc::now().timestamp() as i32;
    LightToolDevicesRepository::new(state.pg_pool.clone())
        .disconnect_account_if_active(auth.device.id, now)
        .map_err(|error| {
            tracing::error!(
                device_id = auth.device.id,
                "Light Tool disconnect failed: {error}"
            );
            internal_error("disconnect failed")
        })?
        .ok_or_else(unauthorized)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn register_push_endpoint(
    State(state): State<Arc<AppState>>,
    auth: LightToolDeviceAuth,
    Json(request): Json<RegisterPushEndpointRequest>,
) -> Result<StatusCode, ApiError> {
    let endpoint = validate_push_endpoint(&request.endpoint)?;
    let now = chrono::Utc::now().timestamp() as i32;
    LightToolPushRepository::new(state.pg_pool.clone())
        .upsert(auth.device.id, endpoint, now)
        .map_err(|error| {
            tracing::error!(
                device_id = auth.device.id,
                "Light Tool push registration failed: {error}"
            );
            internal_error("push registration failed")
        })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn unregister_push_endpoint(
    State(state): State<Arc<AppState>>,
    auth: LightToolDeviceAuth,
) -> Result<StatusCode, ApiError> {
    LightToolPushRepository::new(state.pg_pool.clone())
        .delete_for_device(auth.device.id)
        .map_err(|error| {
            tracing::error!(
                device_id = auth.device.id,
                "Light Tool push unregistration failed: {error}"
            );
            internal_error("push unregistration failed")
        })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn send_message(
    State(state): State<Arc<AppState>>,
    auth: LightToolDeviceAuth,
    Json(request): Json<SendMessageRequest>,
) -> Result<(StatusCode, Json<SendMessageResponse>), ApiError> {
    let client_message_id = Uuid::parse_str(&request.client_message_id)
        .map_err(|_| bad_request("client_message_id must be a UUID"))?
        .to_string();
    let text = request.text.trim();
    if text.is_empty() {
        return Err(bad_request("text cannot be blank"));
    }
    if text.len() > MAX_MESSAGE_TEXT_BYTES {
        return Err(bad_request("text must be at most 8000 bytes"));
    }

    let repository = LightToolRunsRepository::new(state.pg_pool.clone());
    let now = chrono::Utc::now().timestamp() as i32;
    let (run, should_dispatch): (LightToolRunRecord, bool) = match auth.device.user_id {
        Some(user_id) => match repository
            .create_account_run(auth.device.id, user_id, &client_message_id, text, now)
            .map_err(map_run_repository_error)?
        {
            AccountRunCreation::Created(run) => (run, true),
            AccountRunCreation::Existing(run) => (run, false),
            AccountRunCreation::AccountUnavailable => {
                return Err(forbidden(
                    "connected account unavailable; reconnect to continue",
                ));
            }
            AccountRunCreation::IdempotencyConflict => return Err(idempotency_conflict()),
        },
        None => match repository
            .create_anonymous_trial_run(
                auth.device.id,
                &client_message_id,
                text,
                now,
                TRIAL_MESSAGE_LIMIT,
            )
            .map_err(map_run_repository_error)?
        {
            AnonymousTrialRunCreation::Created { run, .. } => (run, true),
            AnonymousTrialRunCreation::Existing { run, .. } => (run, false),
            AnonymousTrialRunCreation::TrialUnavailable => {
                return Err(forbidden(
                    "trial unavailable; connect a Lightfriend account to continue",
                ));
            }
            AnonymousTrialRunCreation::IdempotencyConflict => return Err(idempotency_conflict()),
        },
    };

    if should_dispatch {
        if let Some(responder) = state.light_tool_responder.clone() {
            let pool = state.pg_pool.clone();
            let run_id = run.id.clone();
            let device_id = auth.device.id;
            tokio::spawn(async move {
                let outcome = match dispatch_light_tool_run(pool.clone(), run_id.clone(), responder)
                    .await
                {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        tracing::error!(run_id = %run_id, "Light Tool dispatch failed: {}", error);
                        return;
                    }
                };

                if matches!(
                    outcome,
                    LightToolDispatchOutcome::Completed | LightToolDispatchOutcome::Failed
                ) {
                    let delivery = match LightToolPushDeliveryService::from_env(pool) {
                        Ok(delivery) => delivery,
                        Err(error) => {
                            tracing::error!(run_id = %run_id, "Light Tool push setup failed: {error}");
                            return;
                        }
                    };
                    if let Err(error) = delivery.send_conversation_changed(device_id).await {
                        tracing::warn!(
                            run_id = %run_id,
                            device_id,
                            "Light Tool conversation push failed: {error}"
                        );
                    }
                }
            });
        }
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(SendMessageResponse { run_id: run.id }),
    ))
}

pub async fn get_message_history(
    State(state): State<Arc<AppState>>,
    auth: LightToolDeviceAuth,
) -> Result<Json<MessageHistoryResponse>, ApiError> {
    let runs = LightToolRunsRepository::new(state.pg_pool.clone())
        .find_recent_for_principal(
            auth.device.id,
            auth.device.user_id,
            MESSAGE_HISTORY_RUN_LIMIT,
        )
        .map_err(map_run_repository_error)?;
    let runs = runs
        .into_iter()
        .map(message_history_run_response)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(MessageHistoryResponse { runs }))
}

pub async fn get_run_status(
    State(state): State<Arc<AppState>>,
    auth: LightToolDeviceAuth,
    Path(run_id): Path<String>,
) -> Result<Json<RunStatusResponse>, ApiError> {
    let run_id = Uuid::parse_str(&run_id)
        .map_err(|_| bad_request("run_id must be a UUID"))?
        .to_string();
    let run = LightToolRunsRepository::new(state.pg_pool.clone())
        .find_by_id_for_device(&run_id, auth.device.id)
        .map_err(map_run_repository_error)?
        .ok_or_else(|| not_found("run not found"))?;
    if run.account_user_id.is_some() && run.account_user_id != auth.device.user_id {
        return Err(not_found("run not found"));
    }

    let response_state = match run.status.as_str() {
        "queued" => RunStateResponse::Queued,
        "running" => RunStateResponse::Running,
        "completed" => RunStateResponse::Completed,
        "failed" => RunStateResponse::Failed,
        status => {
            tracing::error!(run_id = %run.id, status, "Light Tool run has an invalid status");
            return Err(internal_error("invalid run state"));
        }
    };
    let activity_text = match response_state {
        RunStateResponse::Queued | RunStateResponse::Running => run.activity_text,
        RunStateResponse::Completed | RunStateResponse::Failed => None,
    };
    let assistant_message = match (response_state, run.assistant_message) {
        (RunStateResponse::Completed, Some(text)) => Some(AssistantMessageResponse {
            id: run.id.clone(),
            text,
            created_at: format_timestamp(run.completed_at.unwrap_or(run.updated_at))?,
        }),
        _ => None,
    };
    let error_message = match response_state {
        RunStateResponse::Failed => run.error_message,
        _ => None,
    };

    Ok(Json(RunStatusResponse {
        run_id: run.id,
        state: response_state,
        activity_text,
        assistant_message,
        error_message,
    }))
}

fn message_history_run_response(
    run: LightToolRunRecord,
) -> Result<MessageHistoryRunResponse, ApiError> {
    let state = run_state_response(&run)?;
    let activity_text = match state {
        RunStateResponse::Queued | RunStateResponse::Running => run.activity_text,
        RunStateResponse::Completed | RunStateResponse::Failed => None,
    };
    let assistant_message = match (state, run.assistant_message) {
        (RunStateResponse::Completed, Some(text)) => Some(AssistantMessageResponse {
            id: run.id.clone(),
            text,
            created_at: format_timestamp(run.completed_at.unwrap_or(run.updated_at))?,
        }),
        _ => None,
    };
    let error_message = match state {
        RunStateResponse::Failed => run.error_message,
        _ => None,
    };

    Ok(MessageHistoryRunResponse {
        run_id: run.id,
        state,
        user_message: UserMessageResponse {
            id: run.client_message_id,
            text: run.user_message,
            created_at: format_timestamp(run.created_at)?,
        },
        activity_text,
        assistant_message,
        error_message,
    })
}

fn run_state_response(run: &LightToolRunRecord) -> Result<RunStateResponse, ApiError> {
    match run.status.as_str() {
        "queued" => Ok(RunStateResponse::Queued),
        "running" => Ok(RunStateResponse::Running),
        "completed" => Ok(RunStateResponse::Completed),
        "failed" => Ok(RunStateResponse::Failed),
        status => {
            tracing::error!(run_id = %run.id, status, "Light Tool run has an invalid status");
            Err(internal_error("invalid run state"))
        }
    }
}

fn validate_push_endpoint(value: &str) -> Result<&str, ApiError> {
    let endpoint = value.trim();
    if endpoint.is_empty() || endpoint.len() > MAX_PUSH_ENDPOINT_BYTES {
        return Err(bad_request("push endpoint is invalid"));
    }
    let parsed = Url::parse(endpoint).map_err(|_| bad_request("push endpoint is invalid"))?;
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.fragment().is_some() {
        return Err(bad_request("push endpoint is invalid"));
    }
    let host = parsed
        .host()
        .ok_or_else(|| bad_request("push endpoint is invalid"))?;
    let loopback_host = match host {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    };
    let valid_scheme = parsed.scheme() == "https"
        || (cfg!(debug_assertions) && parsed.scheme() == "http" && loopback_host);
    if !valid_scheme {
        return Err(bad_request("push endpoint is invalid"));
    }
    if parsed.scheme() == "https" {
        let non_public_host = match host {
            Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
            Host::Ipv4(address) => is_non_public_ip(IpAddr::V4(address)),
            Host::Ipv6(address) => is_non_public_ip(IpAddr::V6(address)),
        };
        if non_public_host {
            return Err(bad_request("push endpoint is invalid"));
        }
    }
    Ok(endpoint)
}

fn is_non_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            address.is_private()
                || address.is_loopback()
                || address.is_link_local()
                || address.is_broadcast()
                || address.is_unspecified()
        }
        IpAddr::V6(address) => {
            address.is_loopback()
                || address.is_unique_local()
                || address.is_unicast_link_local()
                || address.is_unspecified()
        }
    }
}

fn map_bootstrap_error(error: LightToolBootstrapError) -> ApiError {
    match error {
        LightToolBootstrapError::Identity(LightToolIdentityError::InvalidInstallationId) => {
            bad_request("installation_id must be a UUID")
        }
        LightToolBootstrapError::Identity(LightToolIdentityError::InvalidDeviceToken)
        | LightToolBootstrapError::DeviceTokenRequired
        | LightToolBootstrapError::InvalidDeviceCredentials => unauthorized(),
        LightToolBootstrapError::Repository(error) => {
            tracing::error!("Light Tool bootstrap repository error: {}", error);
            internal_error("bootstrap failed")
        }
    }
}

fn map_run_repository_error(error: LightToolRunsRepositoryError) -> ApiError {
    tracing::error!("Light Tool run repository operation failed: {}", error);
    internal_error("run operation failed")
}

fn map_pairing_error(error: LightToolPairingError) -> ApiError {
    match error {
        LightToolPairingError::InvalidOrExpired => {
            bad_request("pairing code is invalid or expired")
        }
        LightToolPairingError::DeviceUnavailable => unauthorized(),
        LightToolPairingError::DeviceAlreadyLinked => {
            conflict("disconnect the current account before pairing another")
        }
        LightToolPairingError::Repository(error) => {
            tracing::error!("Light Tool pairing repository error: {error}");
            internal_error("pairing failed")
        }
    }
}

fn bad_request(message: &str) -> ApiError {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": message })))
}

fn unauthorized() -> ApiError {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": "invalid device credentials" })),
    )
}

fn forbidden(message: &str) -> ApiError {
    (StatusCode::FORBIDDEN, Json(json!({ "error": message })))
}

fn conflict(message: &str) -> ApiError {
    (StatusCode::CONFLICT, Json(json!({ "error": message })))
}

fn idempotency_conflict() -> ApiError {
    (
        StatusCode::CONFLICT,
        Json(
            json!({ "error": "client_message_id was already used under a different account state" }),
        ),
    )
}

fn not_found(message: &str) -> ApiError {
    (StatusCode::NOT_FOUND, Json(json!({ "error": message })))
}

fn format_timestamp(timestamp: i32) -> Result<String, ApiError> {
    chrono::DateTime::from_timestamp(timestamp as i64, 0)
        .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Secs, true))
        .ok_or_else(|| internal_error("invalid run timestamp"))
}

fn internal_error(message: &str) -> ApiError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": message })),
    )
}
