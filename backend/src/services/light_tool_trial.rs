use crate::{
    repositories::light_tool_devices_repository::LightToolDevicesRepository,
    services::light_tool_bootstrap::TRIAL_MESSAGE_LIMIT,
};
use diesel::result::Error as DieselError;

#[derive(Debug, PartialEq, Eq)]
pub enum TrialMessageClaim {
    Claimed { messages_remaining: i32 },
    Unavailable,
}

pub struct LightToolTrialService {
    repository: LightToolDevicesRepository,
}

impl LightToolTrialService {
    pub fn new(repository: LightToolDevicesRepository) -> Self {
        Self { repository }
    }

    pub fn claim_message(
        &self,
        device_id: i32,
        now: i32,
    ) -> Result<TrialMessageClaim, DieselError> {
        let claimed =
            self.repository
                .claim_anonymous_trial_message(device_id, now, TRIAL_MESSAGE_LIMIT)?;

        Ok(match claimed {
            Some(device) => TrialMessageClaim::Claimed {
                messages_remaining: (TRIAL_MESSAGE_LIMIT - device.trial_messages_used).max(0),
            },
            None => TrialMessageClaim::Unavailable,
        })
    }
}
