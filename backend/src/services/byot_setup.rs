use crate::api::twilio_client::{
    IncomingPhoneNumberConfig, TwilioClient, TwilioClientError, TwilioCredentials,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByotWebhookEndpoints {
    pub sms: String,
    pub voice: String,
}

impl ByotWebhookEndpoints {
    pub fn from_server_url(server_url: &str) -> Result<Self, ByotSetupError> {
        let base = server_url.trim().trim_end_matches('/');
        if base.is_empty()
            || !(base.starts_with("https://")
                || base.starts_with("http://localhost")
                || base.starts_with("http://127.0.0.1"))
        {
            return Err(ByotSetupError::new(
                "server_configuration",
                "Lightfriend's webhook address is not configured safely.",
            ));
        }
        Ok(Self {
            sms: format!("{base}/api/sms/server"),
            voice: format!("{base}/api/voice/incoming"),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByotSetupError {
    pub code: &'static str,
    pub user_message: &'static str,
}

impl ByotSetupError {
    fn new(code: &'static str, user_message: &'static str) -> Self {
        Self { code, user_message }
    }
}

pub fn safe_twilio_error(error: &TwilioClientError) -> ByotSetupError {
    match error {
        TwilioClientError::MissingCredentials(_) => ByotSetupError::new(
            "credentials_missing",
            "Add both your Twilio Account SID and Auth Token.",
        ),
        TwilioClientError::ApiError {
            status: 401 | 403, ..
        } => ByotSetupError::new("credentials_rejected", "Twilio rejected those credentials."),
        TwilioClientError::NotFound(_) => ByotSetupError::new(
            "number_not_owned",
            "That number was not found in the authenticated Twilio account.",
        ),
        TwilioClientError::RequestFailed(_) => ByotSetupError::new(
            "twilio_unavailable",
            "Twilio could not be reached. Please retry.",
        ),
        _ => ByotSetupError::new(
            "twilio_rejected",
            "Twilio could not verify this number. Please retry or contact support.",
        ),
    }
}

pub fn verify_live_configuration(
    config: &IncomingPhoneNumberConfig,
    phone_number: &str,
    endpoints: &ByotWebhookEndpoints,
) -> Result<(), ByotSetupError> {
    if config.phone_number != phone_number {
        return Err(ByotSetupError::new(
            "number_not_owned",
            "That number was not found in the authenticated Twilio account.",
        ));
    }
    if !config.sms_capable {
        return Err(ByotSetupError::new(
            "sms_not_supported",
            "This Twilio number does not support SMS.",
        ));
    }
    if !config.voice_capable {
        return Err(ByotSetupError::new(
            "voice_not_supported",
            "This Twilio number does not support voice calls.",
        ));
    }
    if config.sms_url != endpoints.sms
        || !config.sms_method.eq_ignore_ascii_case("POST")
        || config.voice_url != endpoints.voice
        || !config.voice_method.eq_ignore_ascii_case("POST")
    {
        return Err(ByotSetupError::new(
            "webhook_drift",
            "The Twilio webhooks do not match Lightfriend's required configuration.",
        ));
    }
    Ok(())
}

pub async fn configure_and_verify(
    client: &dyn TwilioClient,
    credentials: &TwilioCredentials,
    phone_number: &str,
    endpoints: &ByotWebhookEndpoints,
) -> Result<IncomingPhoneNumberConfig, ByotSetupError> {
    let before = client
        .fetch_incoming_phone_number(credentials, phone_number)
        .await
        .map_err(|error| safe_twilio_error(&error))?;

    if before.phone_number != phone_number {
        return Err(ByotSetupError::new(
            "number_not_owned",
            "That number was not found in the authenticated Twilio account.",
        ));
    }
    if !before.sms_capable {
        return Err(ByotSetupError::new(
            "sms_not_supported",
            "This Twilio number does not support SMS.",
        ));
    }
    if !before.voice_capable {
        return Err(ByotSetupError::new(
            "voice_not_supported",
            "This Twilio number does not support voice calls.",
        ));
    }

    client
        .configure_webhook(
            credentials,
            phone_number,
            &endpoints.sms,
            Some(&endpoints.voice),
        )
        .await
        .map_err(|error| safe_twilio_error(&error))?;

    let after = client
        .fetch_incoming_phone_number(credentials, phone_number)
        .await
        .map_err(|error| safe_twilio_error(&error))?;
    verify_live_configuration(&after, phone_number, endpoints)?;
    Ok(after)
}
