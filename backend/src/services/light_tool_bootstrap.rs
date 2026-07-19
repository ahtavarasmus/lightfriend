use crate::{
    models::light_tool_models::NewLightToolDevice,
    repositories::light_tool_devices_repository::LightToolDevicesRepository,
    services::light_tool_identity::{
        generate_device_token, hash_device_token, hash_installation_id, LightToolIdentityError,
    },
};
use diesel::result::Error as DieselError;
use thiserror::Error;

pub const TRIAL_MESSAGE_LIMIT: i32 = 12;
pub const TRIAL_DURATION_SECONDS: i32 = 3 * 24 * 60 * 60;

pub struct LightToolBootstrapSession {
    pub device_token: String,
    pub can_send: bool,
    pub account_user_id: Option<i32>,
    pub trial_messages_remaining: i32,
    pub trial_expires_at: i32,
}

#[derive(Debug, Error)]
pub enum LightToolBootstrapError {
    #[error(transparent)]
    Identity(#[from] LightToolIdentityError),
    #[error("device token is required for this installation")]
    DeviceTokenRequired,
    #[error("device credentials are invalid")]
    InvalidDeviceCredentials,
    #[error(transparent)]
    Repository(#[from] DieselError),
}

pub struct LightToolBootstrapService {
    repository: LightToolDevicesRepository,
}

impl LightToolBootstrapService {
    pub fn new(repository: LightToolDevicesRepository) -> Self {
        Self { repository }
    }

    pub fn bootstrap(
        &self,
        installation_id: &str,
        existing_device_token: Option<&str>,
        now: i32,
    ) -> Result<LightToolBootstrapSession, LightToolBootstrapError> {
        let installation_id_hash = hash_installation_id(installation_id)?;

        let (device, device_token) = match existing_device_token {
            Some(raw_token) => {
                let token_hash = hash_device_token(raw_token)?;
                let device = self
                    .repository
                    .find_active_by_token_hash(&token_hash)?
                    .ok_or(LightToolBootstrapError::InvalidDeviceCredentials)?;
                if device.installation_id_hash != installation_id_hash {
                    return Err(LightToolBootstrapError::InvalidDeviceCredentials);
                }
                let device = self
                    .repository
                    .update_last_seen_if_active(device.id, now)?
                    .ok_or(LightToolBootstrapError::InvalidDeviceCredentials)?;
                (device, raw_token.to_string())
            }
            None => {
                if self
                    .repository
                    .find_by_installation_hash(&installation_id_hash)?
                    .is_some()
                {
                    return Err(LightToolBootstrapError::DeviceTokenRequired);
                }

                let token = generate_device_token();
                let device = self.repository.create(&NewLightToolDevice {
                    installation_id_hash,
                    device_token_hash: token.hash,
                    trial_started_at: now,
                    trial_expires_at: now + TRIAL_DURATION_SECONDS,
                    last_seen_at: now,
                    created_at: now,
                    updated_at: now,
                })?;
                (device, token.raw)
            }
        };

        let trial_messages_remaining = (TRIAL_MESSAGE_LIMIT - device.trial_messages_used).max(0);
        let anonymous_trial_active = device.user_id.is_none()
            && now < device.trial_expires_at
            && trial_messages_remaining > 0;

        Ok(LightToolBootstrapSession {
            device_token,
            can_send: device.user_id.is_some() || anonymous_trial_active,
            account_user_id: device.user_id,
            trial_messages_remaining,
            trial_expires_at: device.trial_expires_at,
        })
    }
}
