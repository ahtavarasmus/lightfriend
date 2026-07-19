use crate::{
    models::light_tool_models::LightToolDevice,
    repositories::light_tool_devices_repository::LightToolDevicesRepository,
    services::light_tool_identity::hash_device_token, AppState,
};
use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;

const LAST_SEEN_WRITE_INTERVAL_SECONDS: i32 = 60;

pub struct LightToolDeviceAuth {
    pub device: LightToolDevice,
}

#[derive(Debug)]
pub enum LightToolAuthError {
    Unauthorized,
    Internal,
}

impl IntoResponse for LightToolAuthError {
    fn into_response(self) -> Response {
        match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "invalid device credentials" })),
            )
                .into_response(),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "device authentication failed" })),
            )
                .into_response(),
        }
    }
}

impl FromRequestParts<Arc<AppState>> for LightToolDeviceAuth {
    type Rejection = LightToolAuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let raw_token = optional_bearer(&parts.headers)?.ok_or(LightToolAuthError::Unauthorized)?;
        let token_hash =
            hash_device_token(raw_token).map_err(|_| LightToolAuthError::Unauthorized)?;
        let repository = LightToolDevicesRepository::new(state.pg_pool.clone());
        let device = repository
            .find_active_by_token_hash(&token_hash)
            .map_err(|error| {
                tracing::error!("Light Tool authentication lookup failed: {}", error);
                LightToolAuthError::Internal
            })?
            .ok_or(LightToolAuthError::Unauthorized)?;
        let now = chrono::Utc::now().timestamp() as i32;
        let device = if device.last_seen_at <= now - LAST_SEEN_WRITE_INTERVAL_SECONDS {
            repository
                .update_last_seen_if_active(device.id, now)
                .map_err(|error| {
                    tracing::error!(
                        "Light Tool authentication activity update failed: {}",
                        error
                    );
                    LightToolAuthError::Internal
                })?
                .ok_or(LightToolAuthError::Unauthorized)?
        } else {
            device
        };

        Ok(Self { device })
    }
}

pub(crate) fn optional_bearer(headers: &HeaderMap) -> Result<Option<&str>, LightToolAuthError> {
    let Some(header) = headers.get(AUTHORIZATION) else {
        return Ok(None);
    };
    let raw = header
        .to_str()
        .map_err(|_| LightToolAuthError::Unauthorized)?;
    let token = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or(LightToolAuthError::Unauthorized)?;
    Ok(Some(token))
}
