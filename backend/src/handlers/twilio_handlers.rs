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
    let repository = Arc::new(DieselTwilioStatusRepository::new(state.db_pool.clone()));

    // Check if Twilio credentials are available
    match TwilioCredentials::from_env() {
        Ok(credentials) => {
            let client = Arc::new(RealTwilioClient::new());
            let service = TwilioStatusService::new(repository, client, credentials);

            if let Err(e) = service.process_status_callback(input).await {
                tracing::error!("Failed to process status callback: {}", e);
                // Still return OK to Twilio to prevent retries
            }
        }
        Err(_) => {
            // Fallback: process without Twilio client (no price fetch or deletion)
            tracing::warn!(
                "Missing Twilio credentials, processing status callback without API calls"
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
            let service =
                TwilioStatusService::with_config(repository, client, dummy_credentials, config);

            if let Err(e) = service.process_status_callback(input).await {
                tracing::error!("Failed to process status callback: {}", e);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Form Parsing Tests - TwilioStatusCallback
    // =========================================================================

    #[test]
    fn test_status_callback_parses_delivered_status() {
        let body = "MessageSid=SM123&MessageStatus=delivered&To=%2B14155551234&From=%2B18005551234";
        let payload: TwilioStatusCallback = serde_urlencoded::from_str(body).unwrap();

        assert_eq!(payload.MessageSid, "SM123");
        assert_eq!(payload.MessageStatus, "delivered");
        assert_eq!(payload.To, Some("+14155551234".to_string()));
        assert_eq!(payload.From, Some("+18005551234".to_string()));
        assert!(payload.ErrorCode.is_none());
        assert!(payload.ErrorMessage.is_none());
    }

    #[test]
    fn test_status_callback_parses_failed_status_with_error() {
        let body = "MessageSid=SM456&MessageStatus=failed&ErrorCode=30006&ErrorMessage=Landline+or+unreachable+carrier";
        let payload: TwilioStatusCallback = serde_urlencoded::from_str(body).unwrap();

        assert_eq!(payload.MessageSid, "SM456");
        assert_eq!(payload.MessageStatus, "failed");
        assert_eq!(payload.ErrorCode, Some("30006".to_string()));
        assert_eq!(
            payload.ErrorMessage,
            Some("Landline or unreachable carrier".to_string())
        );
    }

    #[test]
    fn test_status_callback_parses_undelivered_status() {
        let body = "MessageSid=SM789&MessageStatus=undelivered&ErrorCode=30003&ErrorMessage=Unreachable+destination+handset";
        let payload: TwilioStatusCallback = serde_urlencoded::from_str(body).unwrap();

        assert_eq!(payload.MessageSid, "SM789");
        assert_eq!(payload.MessageStatus, "undelivered");
        assert_eq!(payload.ErrorCode, Some("30003".to_string()));
    }

    #[test]
    fn test_status_callback_parses_price_string_negative() {
        // Twilio sends prices as negative strings like "-0.0075"
        let body = "MessageSid=SM123&MessageStatus=delivered&Price=-0.0075&PriceUnit=USD";
        let payload: TwilioStatusCallback = serde_urlencoded::from_str(body).unwrap();

        assert_eq!(payload.Price, Some("-0.0075".to_string()));
        assert_eq!(payload.PriceUnit, Some("USD".to_string()));

        // Test parsing to f32 (as done in handler)
        let price_value: Option<f32> = payload.Price.as_ref().and_then(|p| p.parse().ok());
        assert_eq!(price_value, Some(-0.0075));
    }

    #[test]
    fn test_status_callback_parses_price_string_unicode_minus() {
        // Some systems might send unicode minus sign (U+2212) instead of hyphen-minus
        let body = "MessageSid=SM123&MessageStatus=delivered&Price=%E2%88%920.0075&PriceUnit=USD";
        let payload: TwilioStatusCallback = serde_urlencoded::from_str(body).unwrap();

        // Unicode minus sign decodes correctly
        assert!(payload.Price.is_some());
        // But parsing to f32 may fail - this is expected behavior
        let price_value: Option<f32> = payload.Price.as_ref().and_then(|p| p.parse().ok());
        // Unicode minus doesn't parse as float (this is fine, we handle it gracefully)
        assert!(price_value.is_none());
    }

    #[test]
    fn test_status_callback_handles_missing_optional_fields() {
        // Minimum required fields - just MessageSid and MessageStatus
        let body = "MessageSid=SM123&MessageStatus=queued";
        let payload: TwilioStatusCallback = serde_urlencoded::from_str(body).unwrap();

        assert_eq!(payload.MessageSid, "SM123");
        assert_eq!(payload.MessageStatus, "queued");
        assert!(payload.ErrorCode.is_none());
        assert!(payload.ErrorMessage.is_none());
        assert!(payload.To.is_none());
        assert!(payload.From.is_none());
        assert!(payload.AccountSid.is_none());
        assert!(payload.Price.is_none());
        assert!(payload.PriceUnit.is_none());
    }

    #[test]
    fn test_status_callback_handles_sms_prefix_duplicates() {
        // Twilio sends both MessageSid/SmsSid with same values - we capture SmsSid but use MessageSid
        let body = "MessageSid=SM123&SmsSid=SM123&MessageStatus=sent&SmsStatus=sent";
        let payload: TwilioStatusCallback = serde_urlencoded::from_str(body).unwrap();

        assert_eq!(payload.MessageSid, "SM123");
        assert_eq!(payload.MessageStatus, "sent");
        // SMS-prefixed fields are captured but ignored
        assert_eq!(payload.SmsSid, Some("SM123".to_string()));
        assert_eq!(payload.SmsStatus, Some("sent".to_string()));
    }

    #[test]
    fn test_status_callback_parses_all_status_types() {
        let statuses = vec![
            "queued",
            "sending",
            "sent",
            "delivered",
            "undelivered",
            "failed",
            "canceled",
        ];

        for status in statuses {
            let body = format!("MessageSid=SM123&MessageStatus={}", status);
            let payload: TwilioStatusCallback = serde_urlencoded::from_str(&body).unwrap();
            assert_eq!(payload.MessageStatus, status);
        }
    }

    #[test]
    fn test_status_callback_parses_full_payload() {
        // Full payload as Twilio might send it
        let body = "AccountSid=AC123&ApiVersion=2010-04-01&From=%2B18005551234&MessageSid=SM456&MessageStatus=delivered&SmsSid=SM456&SmsStatus=delivered&To=%2B14155559999";
        let payload: TwilioStatusCallback = serde_urlencoded::from_str(body).unwrap();

        assert_eq!(payload.AccountSid, Some("AC123".to_string()));
        assert_eq!(payload.ApiVersion, Some("2010-04-01".to_string()));
        assert_eq!(payload.From, Some("+18005551234".to_string()));
        assert_eq!(payload.MessageSid, "SM456");
        assert_eq!(payload.MessageStatus, "delivered");
        assert_eq!(payload.To, Some("+14155559999".to_string()));
    }

    #[test]
    fn test_status_callback_rejects_missing_required_fields() {
        // Missing MessageSid
        let body = "MessageStatus=delivered";
        let result: Result<TwilioStatusCallback, _> = serde_urlencoded::from_str(body);
        assert!(result.is_err());

        // Missing MessageStatus
        let body = "MessageSid=SM123";
        let result: Result<TwilioStatusCallback, _> = serde_urlencoded::from_str(body);
        assert!(result.is_err());
    }

    #[test]
    fn test_status_callback_parses_special_characters_in_error_message() {
        // Error messages might contain special characters
        let body = "MessageSid=SM123&MessageStatus=failed&ErrorCode=30001&ErrorMessage=Queue+overflow%3A+too+many+messages";
        let payload: TwilioStatusCallback = serde_urlencoded::from_str(body).unwrap();

        assert_eq!(
            payload.ErrorMessage,
            Some("Queue overflow: too many messages".to_string())
        );
    }

    // =========================================================================
    // Price Parsing Tests
    // =========================================================================

    #[test]
    fn test_price_parsing_various_formats() {
        // Test various price string formats Twilio might send
        let test_cases = vec![
            ("-0.0075", Some(-0.0075f32)),
            ("0.0075", Some(0.0075f32)),
            ("-0.00750", Some(-0.0075f32)),
            ("0", Some(0.0f32)),
            ("", None),      // Empty string should fail
            ("abc", None),   // Non-numeric should fail
            ("null", None),  // "null" string should fail
        ];

        for (input, expected) in test_cases {
            let price_value: Option<f32> = if input.is_empty() {
                None
            } else {
                input.parse().ok()
            };
            assert_eq!(
                price_value, expected,
                "Failed for input: '{}', got {:?}, expected {:?}",
                input, price_value, expected
            );
        }
    }

    #[test]
    fn test_price_parsing_from_payload() {
        // Test price parsing as done in the actual handler
        let body = "MessageSid=SM123&MessageStatus=delivered&Price=-0.0079&PriceUnit=USD";
        let payload: TwilioStatusCallback = serde_urlencoded::from_str(body).unwrap();

        let price_value: Option<f32> = payload.Price.as_ref().and_then(|p| p.parse().ok());
        assert!(price_value.is_some());
        // Allow for floating point comparison
        let price = price_value.unwrap();
        assert!((price - (-0.0079)).abs() < 0.0001);
    }
}
