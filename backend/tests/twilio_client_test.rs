//! Unit tests for TwilioClient trait and mock implementation
//!
//! Tests the mock client's behavior for recording calls,
//! error injection, and configured responses.

use backend::api::twilio_client::mock::MockTwilioClient;
use backend::api::twilio_client::{
    MessagePrice, MessagingPricingResult, SendMessageOptions, TwilioClient, TwilioCredentials,
    VoicePricingResult,
};

// =========================================================================
// Send Message Tests
// =========================================================================

#[tokio::test]
async fn test_mock_send_message_records_call() {
    let client = MockTwilioClient::new();
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let options = SendMessageOptions {
        to: "+1234567890".into(),
        body: "Hello".into(),
        from: Some("+0987654321".into()),
        ..Default::default()
    };

    let result = client.send_message(&creds, options.clone()).await;
    assert!(result.is_ok());

    let calls = client.get_calls();
    assert_eq!(calls.send_message_calls.len(), 1);
    assert_eq!(calls.send_message_calls[0].to, "+1234567890");
    assert_eq!(calls.send_message_calls[0].body, "Hello");
    assert_eq!(calls.send_message_calls[0].from, Some("+0987654321".into()));
}

#[tokio::test]
async fn test_mock_send_message_with_messaging_service() {
    let client = MockTwilioClient::new();
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let options = SendMessageOptions {
        to: "+1234567890".into(),
        body: "Hello via messaging service".into(),
        messaging_service_sid: Some("MG_test_sid".into()),
        ..Default::default()
    };

    let result = client.send_message(&creds, options).await;
    assert!(result.is_ok());

    let calls = client.get_calls();
    assert_eq!(
        calls.send_message_calls[0].messaging_service_sid,
        Some("MG_test_sid".into())
    );
    assert!(calls.send_message_calls[0].from.is_none());
}

#[tokio::test]
async fn test_mock_send_message_error_injection() {
    let client = MockTwilioClient::new().with_send_result(Err("Test error".into()));
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let options = SendMessageOptions {
        to: "+1234567890".into(),
        body: "Hello".into(),
        ..Default::default()
    };

    let result = client.send_message(&creds, options).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Test error"));
}

// =========================================================================
// Delete Message Tests
// =========================================================================

#[tokio::test]
async fn test_mock_delete_message_records_call() {
    let client = MockTwilioClient::new();
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client.delete_message(&creds, "SM_test_123").await;
    assert!(result.is_ok());

    assert!(client.was_deleted("SM_test_123"));
    assert_eq!(client.delete_message_call_count(), 1);
}

#[tokio::test]
async fn test_mock_delete_message_error_injection() {
    let client = MockTwilioClient::new().with_delete_message_error("Delete failed".to_string());
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client.delete_message(&creds, "SM_test_123").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Delete failed"));
}

// =========================================================================
// Delete Message Media Tests
// =========================================================================

#[tokio::test]
async fn test_mock_delete_message_media_records_call() {
    let client = MockTwilioClient::new();
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client
        .delete_message_media(&creds, "SM_msg_123", "ME_media_456")
        .await;
    assert!(result.is_ok());

    assert!(client.was_media_deleted("SM_msg_123", "ME_media_456"));
    let calls = client.get_calls();
    assert_eq!(calls.delete_media_calls.len(), 1);
}

#[tokio::test]
async fn test_mock_delete_message_media_error_injection() {
    let client = MockTwilioClient::new().with_delete_media_error("Media delete failed".to_string());
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client
        .delete_message_media(&creds, "SM_msg_123", "ME_media_456")
        .await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Media delete failed"));
}

// =========================================================================
// Fetch Message Price Tests
// =========================================================================

#[tokio::test]
async fn test_mock_fetch_price_records_call() {
    let client = MockTwilioClient::new();
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client.fetch_message_price(&creds, "SM_price_123").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none()); // Default is None

    assert_eq!(client.fetch_price_call_count(), 1);
}

#[tokio::test]
async fn test_mock_fetch_price_with_specific_response() {
    let client = MockTwilioClient::new();
    client.set_price_response(
        "SM_price_123",
        Some(MessagePrice {
            price: Some("-0.0075".into()),
            price_unit: Some("USD".into()),
        }),
    );
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client.fetch_message_price(&creds, "SM_price_123").await;
    assert!(result.is_ok());
    let price = result.unwrap().unwrap();
    assert_eq!(price.price, Some("-0.0075".into()));
    assert_eq!(price.price_unit, Some("USD".into()));
}

#[tokio::test]
async fn test_mock_fetch_price_error_injection() {
    let client = MockTwilioClient::new().with_fetch_price_error("Price fetch failed".to_string());
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client.fetch_message_price(&creds, "SM_price_123").await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Price fetch failed"));
}

// =========================================================================
// Configure Webhook Tests
// =========================================================================

#[tokio::test]
async fn test_mock_configure_webhook_records_call() {
    let client = MockTwilioClient::new();
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client
        .configure_webhook(
            &creds,
            "+14155551234",
            "https://example.com/sms",
            Some("https://example.com/voice"),
        )
        .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "PN_mock_sid");

    assert_eq!(client.configure_webhook_call_count(), 1);
    let calls = client.get_calls();
    assert_eq!(calls.configure_webhook_calls[0].0, "+14155551234");
    assert_eq!(
        calls.configure_webhook_calls[0].1,
        "https://example.com/sms"
    );
}

#[tokio::test]
async fn test_mock_configure_webhook_accepts_no_voice_url() {
    let client = MockTwilioClient::new();
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    // Calling without a voice URL is valid (e.g. SMS-only setup)
    let result = client
        .configure_webhook(&creds, "+14155551234", "https://example.com/sms", None)
        .await;
    assert!(result.is_ok());
    assert_eq!(client.configure_webhook_call_count(), 1);
}

#[tokio::test]
async fn test_mock_configure_webhook_error_injection() {
    let client =
        MockTwilioClient::new().with_configure_webhook_error("Webhook config failed".to_string());
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client
        .configure_webhook(
            &creds,
            "+14155551234",
            "https://example.com/sms",
            Some("https://example.com/voice"),
        )
        .await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Webhook config failed"));
}

// =========================================================================
// Check Phone Numbers Available Tests
// =========================================================================

#[tokio::test]
async fn test_mock_check_phone_numbers_available_default_false() {
    let client = MockTwilioClient::new();
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client.check_phone_numbers_available(&creds, "US").await;
    assert!(result.is_ok());
    assert!(!result.unwrap()); // Default is false

    let calls = client.get_calls();
    assert_eq!(calls.check_phone_numbers_calls.len(), 1);
    assert_eq!(calls.check_phone_numbers_calls[0], "US");
}

#[tokio::test]
async fn test_mock_check_phone_numbers_available_configured_true() {
    let client = MockTwilioClient::new().with_phone_numbers_available(true);
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client.check_phone_numbers_available(&creds, "US").await;
    assert!(result.is_ok());
    assert!(result.unwrap());
}

// =========================================================================
// Get Messaging Pricing Tests
// =========================================================================

#[tokio::test]
async fn test_mock_get_messaging_pricing_records_call() {
    let client = MockTwilioClient::new();
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client.get_messaging_pricing(&creds, "US").await;
    assert!(result.is_ok());

    let calls = client.get_calls();
    assert_eq!(calls.get_messaging_pricing_calls.len(), 1);
    assert_eq!(calls.get_messaging_pricing_calls[0], "US");
}

#[tokio::test]
async fn test_mock_get_messaging_pricing_with_configured_response() {
    let client = MockTwilioClient::new().with_messaging_pricing(
        "US",
        MessagingPricingResult {
            outbound_sms_price: Some(0.0075),
            inbound_sms_price: Some(0.0085),
        },
    );
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client.get_messaging_pricing(&creds, "US").await;
    assert!(result.is_ok());
    let pricing = result.unwrap();
    assert_eq!(pricing.outbound_sms_price, Some(0.0075));
    assert_eq!(pricing.inbound_sms_price, Some(0.0085));
}

// =========================================================================
// Get Voice Pricing Tests
// =========================================================================

#[tokio::test]
async fn test_mock_get_voice_pricing_records_call() {
    let client = MockTwilioClient::new();
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client.get_voice_pricing(&creds, "US", true).await;
    assert!(result.is_ok());

    let calls = client.get_calls();
    assert_eq!(calls.get_voice_pricing_calls.len(), 1);
    assert_eq!(calls.get_voice_pricing_calls[0], ("US".to_string(), true));
}

#[tokio::test]
async fn test_mock_get_voice_pricing_with_configured_response() {
    let client = MockTwilioClient::new().with_voice_pricing(
        "US",
        VoicePricingResult {
            outbound_price_per_min: Some(0.013),
            inbound_price_per_min: Some(0.0085),
        },
    );
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    let result = client.get_voice_pricing(&creds, "US", false).await;
    assert!(result.is_ok());
    let pricing = result.unwrap();
    assert_eq!(pricing.outbound_price_per_min, Some(0.013));
    assert_eq!(pricing.inbound_price_per_min, Some(0.0085));
}

// =========================================================================
// Clear Calls Tests
// =========================================================================

#[tokio::test]
async fn test_mock_clear_calls() {
    let client = MockTwilioClient::new();
    let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

    // Make some calls
    let _ = client
        .send_message(
            &creds,
            SendMessageOptions {
                to: "+1".into(),
                body: "test".into(),
                ..Default::default()
            },
        )
        .await;
    let _ = client.delete_message(&creds, "SM1").await;

    assert_eq!(client.send_message_call_count(), 1);
    assert_eq!(client.delete_message_call_count(), 1);

    // Clear calls
    client.clear_calls();

    assert_eq!(client.send_message_call_count(), 0);
    assert_eq!(client.delete_message_call_count(), 0);
}

// =========================================================================
// TwilioCredentials Tests
// =========================================================================

#[test]
fn test_credentials_new() {
    let creds = TwilioCredentials::new("AC_sid".into(), "auth_token".into());
    assert_eq!(creds.account_sid, "AC_sid");
    assert_eq!(creds.auth_token, "auth_token");
}

#[test]
fn test_credentials_from_env_missing() {
    // Ensure env vars are not set
    std::env::remove_var("TWILIO_ACCOUNT_SID");
    std::env::remove_var("TWILIO_AUTH_TOKEN");

    let result = TwilioCredentials::from_env();
    assert!(result.is_err());
}

// =========================================================================
// TwilioClientError Tests
// =========================================================================

#[test]
fn test_error_display_missing_credentials() {
    use backend::api::twilio_client::TwilioClientError;
    let err = TwilioClientError::MissingCredentials("Test cred error".into());
    assert!(err.to_string().contains("Missing credentials"));
    assert!(err.to_string().contains("Test cred error"));
}

#[test]
fn test_error_display_api_error() {
    use backend::api::twilio_client::TwilioClientError;
    let err = TwilioClientError::ApiError {
        status: 400,
        message: "Bad request".into(),
    };
    assert!(err.to_string().contains("400"));
    assert!(err.to_string().contains("Bad request"));
}

#[test]
fn test_error_display_not_found() {
    use backend::api::twilio_client::TwilioClientError;
    let err = TwilioClientError::NotFound("Phone number not found".into());
    assert!(err.to_string().contains("Not found"));
}
