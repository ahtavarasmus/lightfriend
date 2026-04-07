//! Unit tests for Twilio handlers - form parsing and status callback tests.
//!
//! Tests the TwilioStatusCallback struct parsing from Twilio webhook payloads.

use backend::api::twilio_utils::{compute_twilio_signature, verify_twilio_signature};
use backend::handlers::twilio_handlers::TwilioStatusCallback;
use std::collections::BTreeMap;

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
        ("", None),     // Empty string should fail
        ("abc", None),  // Non-numeric should fail
        ("null", None), // "null" string should fail
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

// =========================================================================
// Twilio Signature Verification Tests
// =========================================================================

fn make_callback_params() -> BTreeMap<String, String> {
    let mut params = BTreeMap::new();
    params.insert("MessageSid".to_string(), "SMabc123".to_string());
    params.insert("MessageStatus".to_string(), "delivered".to_string());
    params.insert("To".to_string(), "+525540369693".to_string());
    params.insert("From".to_string(), "+527341840032".to_string());
    params.insert("AccountSid".to_string(), "AC123".to_string());
    params
}

#[test]
fn test_signature_verifies_when_signed_with_correct_token() {
    let url = "https://lightfriend.ai/api/twilio/status-callback";
    let params = make_callback_params();
    let auth_token = "correct_secret_token";

    let signature = compute_twilio_signature(url, &params, auth_token);
    let result = verify_twilio_signature(url, &params, &signature, auth_token);
    assert!(result.is_ok(), "Valid signature should verify successfully");
}

#[test]
fn test_signature_rejects_when_signed_with_wrong_token() {
    // This is exactly the BYOT bug: signature was computed with the BYOT
    // user's auth token, but we tried to verify with the master token.
    let url = "https://lightfriend.ai/api/twilio/status-callback";
    let params = make_callback_params();
    let byot_user_token = "byot_user_secret";
    let master_token = "master_account_secret";

    let signature = compute_twilio_signature(url, &params, byot_user_token);
    let result = verify_twilio_signature(url, &params, &signature, master_token);
    assert!(
        result.is_err(),
        "Signature signed with one token must not verify against another"
    );
}

#[test]
fn test_signature_rejects_when_url_differs() {
    let params = make_callback_params();
    let token = "secret";
    let signature = compute_twilio_signature(
        "https://lightfriend.ai/api/twilio/status-callback",
        &params,
        token,
    );
    let result = verify_twilio_signature(
        "https://attacker.example.com/api/twilio/status-callback",
        &params,
        &signature,
        token,
    );
    assert!(
        result.is_err(),
        "Signature must not verify if URL is different"
    );
}

#[test]
fn test_signature_rejects_when_params_tampered() {
    let url = "https://lightfriend.ai/api/twilio/status-callback";
    let mut params = make_callback_params();
    let token = "secret";
    let signature = compute_twilio_signature(url, &params, token);

    // Tamper: change the message status
    params.insert("MessageStatus".to_string(), "failed".to_string());

    let result = verify_twilio_signature(url, &params, &signature, token);
    assert!(
        result.is_err(),
        "Signature must not verify if params were modified"
    );
}

#[test]
fn test_signature_rejects_garbage_signature() {
    let url = "https://lightfriend.ai/api/twilio/status-callback";
    let params = make_callback_params();
    let token = "secret";

    let result = verify_twilio_signature(url, &params, "not-base64!!!", token);
    assert!(result.is_err(), "Garbage signature must be rejected");
}

#[test]
fn test_byot_signature_round_trip() {
    // Simulates the full BYOT flow:
    // 1. BYOT user's Twilio account computes signature with their token
    // 2. Our validator looks up the user, finds they're BYOT, and uses their token
    // 3. Verification succeeds
    let url = "https://lightfriend.ai/api/twilio/status-callback";
    let params = make_callback_params();
    let byot_user_token = "AC_BYOT_USER_AUTH_TOKEN_FROM_THEIR_TWILIO_DASHBOARD";

    // Step 1: simulate Twilio (the BYOT user's account) signing the callback
    let signature_from_twilio = compute_twilio_signature(url, &params, byot_user_token);

    // Step 2: our validator picks the BYOT user's token (because the MessageSid
    // belongs to a BYOT user) and verifies
    let result = verify_twilio_signature(url, &params, &signature_from_twilio, byot_user_token);

    assert!(
        result.is_ok(),
        "BYOT signature round trip should succeed: {:?}",
        result.err()
    );
}
