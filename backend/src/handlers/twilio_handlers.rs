use crate::admin_alert;
use crate::repositories::twilio_status_repository::TwilioStatusRepository;
use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use std::sync::Arc;

/// Twilio Status Callback payload
/// https://www.twilio.com/docs/messaging/guides/track-outbound-message-status
/// Note: Twilio sends both MessageSid/SmsSid and MessageStatus/SmsStatus with same values.
/// We only capture the Message-prefixed ones to avoid serde duplicate field errors.
#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct TwilioStatusCallback {
    pub MessageSid: String,
    pub MessageStatus: String,
    #[serde(default)]
    pub ErrorCode: Option<String>,
    #[serde(default)]
    pub ErrorMessage: Option<String>,
    #[serde(default)]
    pub To: Option<String>,
    #[serde(default)]
    pub From: Option<String>,
    #[serde(default)]
    pub AccountSid: Option<String>,
    #[serde(default)]
    pub ApiVersion: Option<String>,
    #[serde(default)]
    pub Price: Option<String>,
    #[serde(default)]
    pub PriceUnit: Option<String>,
    // Twilio also sends these SMS-prefixed duplicates - we ignore them
    #[serde(default)]
    pub SmsSid: Option<String>,
    #[serde(default)]
    pub SmsStatus: Option<String>,
    // Additional fields Twilio may send
    #[serde(default)]
    pub RawDlrDoneDate: Option<String>,
}

/// Handle Twilio SMS status callback webhooks
///
/// This endpoint receives delivery status updates from Twilio for outbound messages.
/// It updates the message_status_log table and sends admin email on failures.
///
/// Status flow: queued -> sending -> sent -> delivered (success)
///              queued -> sending -> sent -> undelivered (failure)
///              queued -> failed (immediate failure)
pub async fn twilio_status_callback(
    State(state): State<Arc<AppState>>,
    body: String,
) -> StatusCode {
    use crate::api::twilio_client::{RealTwilioClient, TwilioCredentials};
    use crate::repositories::twilio_status_repository_impl::DieselTwilioStatusRepository;
    use crate::services::twilio_status_service::{
        StatusCallbackInput, TwilioStatusService, TwilioStatusServiceConfig,
    };

    tracing::info!("Twilio status callback raw body: {}", body);

    // Parse form data manually
    let payload: TwilioStatusCallback = match serde_urlencoded::from_str(&body) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Failed to parse Twilio status callback: {}", e);
            return StatusCode::BAD_REQUEST;
        }
    };

    tracing::info!(
        "Twilio status callback parsed: sid={}, status={}, error_code={:?}",
        payload.MessageSid,
        payload.MessageStatus,
        payload.ErrorCode
    );

    // Parse price from string to f32 (Twilio sends as string like "-0.0075")
    let price_value: Option<f32> = payload.Price.as_ref().and_then(|p| p.parse().ok());

    // Create the input for the service
    let input = StatusCallbackInput {
        message_sid: payload.MessageSid.clone(),
        message_status: payload.MessageStatus.clone(),
        error_code: payload.ErrorCode.clone(),
        error_message: payload.ErrorMessage.clone(),
        price: price_value,
        price_unit: payload.PriceUnit.clone(),
    };

    // Create real implementations
    let repository = Arc::new(DieselTwilioStatusRepository::new(state.pg_pool.clone()));

    // Check if Twilio credentials are available
    match TwilioCredentials::from_env() {
        Ok(credentials) => {
            let client = Arc::new(RealTwilioClient::new());
            let service = TwilioStatusService::new(repository.clone(), client, credentials);

            if let Err(e) = service.process_status_callback(input).await {
                tracing::error!("Failed to process status callback: {}", e);
                // Still return OK to Twilio to prevent retries
            }
        }
        Err(_) => {
            // Fallback: process without Twilio client (no price fetch or deletion)
            tracing::error!(
                "CRITICAL: Missing Twilio credentials during status callback - using NoOp fallback"
            );

            admin_alert!(
                state,
                Critical,
                "Twilio credentials missing during status callback",
                message_sid = payload.MessageSid,
                status = payload.MessageStatus,
                impact = "Price tracking and message cleanup disabled"
            );

            // Create a no-op client for when credentials aren't available
            // We still need credentials for the service, so create dummy ones
            let dummy_credentials = TwilioCredentials::new(String::new(), String::new());
            let client = Arc::new(NoOpTwilioClient);
            let config = TwilioStatusServiceConfig {
                send_failure_notifications: true,
                fetch_price_on_final: false,
                delete_on_final: false,
                price_fetch_delays: vec![],
            };
            let service = TwilioStatusService::with_config(
                repository.clone(),
                client,
                dummy_credentials,
                config,
            );

            if let Err(e) = service.process_status_callback(input).await {
                tracing::error!("Failed to process status callback: {}", e);
            }
        }
    }

    // Deduct credits on final status with actual Twilio price
    let is_final = matches!(
        payload.MessageStatus.as_str(),
        "delivered" | "sent" | "failed" | "undelivered"
    );
    if is_final {
        if let Some(price) = price_value {
            if price.abs() > 0.0 {
                // Look up user_id from message_status_log by message_sid
                if let Ok(Some(user_info)) = repository.get_message_user_info(&payload.MessageSid) {
                    match crate::utils::usage::deduct_from_twilio_price(
                        &state,
                        user_info.user_id,
                        price,
                    ) {
                        Ok(cost) => {
                            tracing::info!(
                                "Deducted {:.4} credits for user {} (sid: {})",
                                cost,
                                user_info.user_id,
                                payload.MessageSid
                            );
                            // Update the usage log entry with the actual cost
                            if let Err(e) = state
                                .user_repository
                                .update_usage_log_credits(&payload.MessageSid, cost as f32)
                            {
                                tracing::error!(
                                    "Failed to update usage log credits for sid {}: {}",
                                    payload.MessageSid,
                                    e
                                );
                            }
                        }
                        Err(e) => tracing::error!(
                            "Failed to deduct credits for user {}: {}",
                            user_info.user_id,
                            e
                        ),
                    }
                }
            }
        }
    }

    // Always return 200 OK to Twilio
    StatusCode::OK
}

/// No-op Twilio client for when credentials aren't available
struct NoOpTwilioClient;

#[async_trait::async_trait]
impl crate::api::twilio_client::TwilioClient for NoOpTwilioClient {
    async fn send_message(
        &self,
        _credentials: &crate::api::twilio_client::TwilioCredentials,
        _options: crate::api::twilio_client::SendMessageOptions,
    ) -> Result<
        crate::api::twilio_client::SendMessageResult,
        crate::api::twilio_client::TwilioClientError,
    > {
        Ok(crate::api::twilio_client::SendMessageResult {
            message_sid: "noop".to_string(),
        })
    }

    async fn delete_message(
        &self,
        _credentials: &crate::api::twilio_client::TwilioCredentials,
        _message_sid: &str,
    ) -> Result<(), crate::api::twilio_client::TwilioClientError> {
        Ok(())
    }

    async fn delete_message_media(
        &self,
        _credentials: &crate::api::twilio_client::TwilioCredentials,
        _message_sid: &str,
        _media_sid: &str,
    ) -> Result<(), crate::api::twilio_client::TwilioClientError> {
        Ok(())
    }

    async fn fetch_message_price(
        &self,
        _credentials: &crate::api::twilio_client::TwilioCredentials,
        _message_sid: &str,
    ) -> Result<
        Option<crate::api::twilio_client::MessagePrice>,
        crate::api::twilio_client::TwilioClientError,
    > {
        Ok(None)
    }

    async fn configure_webhook(
        &self,
        _credentials: &crate::api::twilio_client::TwilioCredentials,
        _phone_number: &str,
        _webhook_url: &str,
    ) -> Result<String, crate::api::twilio_client::TwilioClientError> {
        Ok("noop".to_string())
    }

    async fn check_phone_numbers_available(
        &self,
        _credentials: &crate::api::twilio_client::TwilioCredentials,
        _country_code: &str,
    ) -> Result<bool, crate::api::twilio_client::TwilioClientError> {
        Ok(false)
    }

    async fn get_messaging_pricing(
        &self,
        _credentials: &crate::api::twilio_client::TwilioCredentials,
        _country_code: &str,
    ) -> Result<
        crate::api::twilio_client::MessagingPricingResult,
        crate::api::twilio_client::TwilioClientError,
    > {
        Ok(crate::api::twilio_client::MessagingPricingResult::default())
    }

    async fn get_voice_pricing(
        &self,
        _credentials: &crate::api::twilio_client::TwilioCredentials,
        _country_code: &str,
        _use_v2: bool,
    ) -> Result<
        crate::api::twilio_client::VoicePricingResult,
        crate::api::twilio_client::TwilioClientError,
    > {
        Ok(crate::api::twilio_client::VoicePricingResult::default())
    }

    async fn make_call(
        &self,
        _credentials: &crate::api::twilio_client::TwilioCredentials,
        _to: &str,
        _from: &str,
        _twiml: &str,
    ) -> Result<String, crate::api::twilio_client::TwilioClientError> {
        Ok("noop_call_sid".to_string())
    }
}
