//! End-to-end tests for SMS processing and credit deduction
//!
//! ## Test Coverage
//! - MockLlmResponse: Creates valid ChatCompletionResponse with tool calls
//! - TestUserParams: Creates correct phone formats for US, Finland, UK, Germany
//! - ProcessSmsOptions: Verifies production, web_chat, and test_with_mock modes
//! - Database integration: Tests credit deduction with real in-memory database
//!
//! Credit deduction model (unified credits):
//! - All users get 25.0 credits monthly (MONTHLY_CREDIT_BUDGET)
//! - SMS: credits deducted at Twilio status callback (actual price * 1.3 margin)
//! - Voice/web: credits deducted at ElevenLabs callback
//! - Pre-send check: just verifies credits > 0 for SMS events

use axum::http::StatusCode;
use backend::api::twilio_sms::{process_sms, ProcessSmsOptions, TwilioWebhookPayload};
use backend::test_utils::{
    assert_charged, assert_no_content_leak, assert_not_charged, assert_sms_deliverable,
    create_test_state, create_test_user, deactivate_phone_service, get_total_credits,
    set_byot_credentials, MockLlmResponse, TestUserParams,
};
use backend::utils::usage::deduct_from_twilio_price;
use backend::UserCoreOps;
use serial_test::serial;

#[test]
#[serial]
fn test_mock_llm_response_creates_valid_response() {
    let mock = MockLlmResponse::with_direct_response("Test response");
    let response = mock.to_response();

    assert_eq!(response.choices.len(), 1);
    assert_eq!(
        response.choices[0].message.content,
        Some("Test response".to_string())
    );
    assert_eq!(
        response.choices[0].finish_reason,
        Some(openai_api_rs::v1::chat_completion::FinishReason::stop)
    );
}

#[test]
#[serial]
fn test_us_user_params_has_correct_phone_format() {
    let params = TestUserParams::us_user(10.0, 5.0);
    assert!(
        params.phone_number.starts_with("+1"),
        "US phone should start with +1"
    );
    assert_eq!(params.credits_left, 10.0);
    assert_eq!(params.credits, 5.0);
    assert_eq!(params.sub_tier, Some("tier 2".to_string()));
}

#[test]
#[serial]
fn test_finland_user_params_has_correct_phone_format() {
    let params = TestUserParams::finland_user(5.0, 2.5);
    assert!(
        params.phone_number.starts_with("+358"),
        "Finland phone should start with +358"
    );
}

#[test]
#[serial]
fn test_uk_user_params_has_correct_phone_format() {
    let params = TestUserParams::uk_user(5.0, 2.5);
    assert!(
        params.phone_number.starts_with("+44"),
        "UK phone should start with +44"
    );
}

#[test]
#[serial]
fn test_germany_user_params_has_correct_phone_format() {
    let params = TestUserParams::germany_user(5.0, 2.5);
    assert!(
        params.phone_number.starts_with("+49"),
        "Germany phone should start with +49"
    );
}

#[test]
#[serial]
fn test_process_sms_options_default_is_production() {
    let options = ProcessSmsOptions::default();
    assert!(!options.skip_twilio_send);
    assert!(options.mock_llm_response.is_none());
}

#[test]
#[serial]
fn test_process_sms_options_web_chat_skips_twilio() {
    let options = ProcessSmsOptions::web_chat();
    assert!(options.skip_twilio_send);
    assert!(options.mock_llm_response.is_none());
}

#[test]
#[serial]
fn test_process_sms_options_test_with_mock() {
    let mock = MockLlmResponse::with_direct_response("Test");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());
    assert!(options.skip_twilio_send);
    assert!(options.mock_llm_response.is_some());
}

// ============================================================
// Database Integration Tests - Callback Credit Deduction
// ============================================================
// SMS credits are deducted at Twilio status callback via deduct_from_twilio_price(),
// not at send time. These tests verify the callback deduction logic.

#[test]
#[serial]
fn test_callback_deduction_from_credits_left() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    assert_eq!(user.credits_left, 10.0);
    assert_eq!(user.credits, 5.0);

    // Simulate Twilio callback with $0.0075 price (typical US SMS)
    let cost = deduct_from_twilio_price(&state, user.id, 0.0075).unwrap();

    // Expected: $0.0075 * 1.3 margin = $0.00975
    assert!(cost > 0.0, "Should have deducted some credits");

    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert!(
        updated.credits_left < 10.0,
        "credits_left should be deducted"
    );
    assert_eq!(updated.credits, 5.0); // Unchanged (credits_left used first)
}

#[test]
#[serial]
fn test_callback_deduction_fallback_when_credits_left_exhausted() {
    let state = create_test_state();
    let params = TestUserParams::us_user(0.0, 5.0);
    let user = create_test_user(&state, &params);

    assert_eq!(user.credits_left, 0.0);
    assert_eq!(user.credits, 5.0);

    // Simulate Twilio callback - should fall back to credits pool
    let cost = deduct_from_twilio_price(&state, user.id, 0.0075).unwrap();
    assert!(cost > 0.0);

    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 0.0); // Still 0
    assert!(updated.credits < 5.0, "credits should be deducted");
}

#[test]
#[serial]
fn test_callback_deduction_finland_user() {
    let state = create_test_state();
    let params = TestUserParams::finland_user(5.0, 2.5);
    let user = create_test_user(&state, &params);

    assert_eq!(user.credits_left, 5.0);
    assert_eq!(user.credits, 2.5);

    // Simulate Twilio callback with typical Finland SMS price
    let cost = deduct_from_twilio_price(&state, user.id, 0.06).unwrap();
    assert!(cost > 0.0);

    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert!(
        updated.credits_left < 5.0,
        "credits_left should be deducted for Finland user"
    );
    assert_eq!(updated.credits, 2.5); // credits unchanged (credits_left used first)
}

#[test]
#[serial]
fn test_callback_deduction_uk_user() {
    let state = create_test_state();
    let params = TestUserParams::uk_user(5.0, 2.5);
    let user = create_test_user(&state, &params);

    // Simulate Twilio callback with typical UK SMS price
    let cost = deduct_from_twilio_price(&state, user.id, 0.04).unwrap();
    assert!(cost > 0.0);

    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert!(
        updated.credits_left < 5.0,
        "credits_left should be deducted for UK user"
    );
    assert_eq!(updated.credits, 2.5); // credits unchanged
}

#[test]
#[serial]
fn test_callback_deduction_germany_user() {
    let state = create_test_state();
    let params = TestUserParams::germany_user(5.0, 2.5);
    let user = create_test_user(&state, &params);

    // Simulate Twilio callback with typical Germany SMS price
    let cost = deduct_from_twilio_price(&state, user.id, 0.07).unwrap();
    assert!(cost > 0.0);

    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert!(
        updated.credits_left < 5.0,
        "credits_left should be deducted for Germany user"
    );
    assert_eq!(updated.credits, 2.5); // credits unchanged
}

// ============================================================
// Full process_sms Flow Tests
// ============================================================
// These tests run the complete SMS processing pipeline with mock LLM

#[tokio::test]
#[serial]
async fn test_process_sms_us_user_full_flow() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    // Verify initial state
    assert_eq!(user.credits_left, 10.0);
    assert_eq!(user.credits, 5.0);

    // Create webhook payload simulating incoming SMS
    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "What is 2+2?".to_string(),
        message_sid: "SM_test_us_123".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    // Use mock LLM response
    let mock = MockLlmResponse::with_direct_response("2+2 equals 4");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    // Process the SMS
    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // SMS credits are deducted at Twilio callback, not at send time
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 10.0); // Unchanged at send time
    assert_eq!(updated.credits, 5.0); // Unchanged
}

#[tokio::test]
#[serial]
async fn test_process_sms_finland_user_full_flow() {
    let state = create_test_state();
    let params = TestUserParams::finland_user(5.0, 2.5);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+358401000000".to_string(),
        body: "Mikä on sää tänään?".to_string(),
        message_sid: "SM_test_fi_123".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Sää on aurinkoinen");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // SMS credits deducted at Twilio callback, not at send time
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 5.0); // Unchanged at send time
    assert_eq!(updated.credits, 2.5); // Unchanged
}

#[tokio::test]
#[serial]
async fn test_process_sms_uk_user_full_flow() {
    let state = create_test_state();
    let params = TestUserParams::uk_user(5.0, 2.5);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+447000000000".to_string(),
        body: "What's the weather?".to_string(),
        message_sid: "SM_test_uk_123".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Cloudy with a chance of rain");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // SMS credits deducted at Twilio callback, not at send time
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 5.0); // Unchanged at send time
    assert_eq!(updated.credits, 2.5); // Unchanged
}

#[tokio::test]
#[serial]
async fn test_process_sms_germany_user_full_flow() {
    let state = create_test_state();
    let params = TestUserParams::germany_user(5.0, 2.5);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+4915000000000".to_string(),
        body: "Wie ist das Wetter?".to_string(),
        message_sid: "SM_test_de_123".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Es ist sonnig");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // SMS credits deducted at Twilio callback, not at send time
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 5.0); // Unchanged at send time
    assert_eq!(updated.credits, 2.5); // Unchanged
}

#[tokio::test]
#[serial]
async fn test_process_sms_credits_fallback_full_flow() {
    let state = create_test_state();
    // User with no credits_left, only credits
    let params = TestUserParams::us_user(0.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Hello".to_string(),
        message_sid: "SM_test_fallback_123".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Hello there!");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // SMS credits deducted at Twilio callback, not at send time
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 0.0); // Still 0
    assert_eq!(updated.credits, 5.0); // Unchanged at send time
}

#[tokio::test]
#[serial]
async fn test_process_sms_rejects_user_with_no_credits() {
    let state = create_test_state();
    // User with no credits at all
    let params = TestUserParams::us_user(0.0, 0.0);
    let user = create_test_user(&state, &params);

    // Verify initial state - no credits
    assert_eq!(user.credits_left, 0.0);
    assert_eq!(user.credits, 0.0);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Hello".to_string(),
        message_sid: "SM_test_no_credits_123".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("This should not be called");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    // Should reject with PAYMENT_REQUIRED (402)
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED);

    // Verify: Credits unchanged (message was not processed)
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 0.0);
    assert_eq!(updated.credits, 0.0);
}

// ============================================================
// Edge Case Tests
// ============================================================

#[tokio::test]
#[serial]
async fn test_process_sms_unknown_phone_returns_not_found() {
    let state = create_test_state();
    // Don't create any user - phone number won't exist

    let payload = TwilioWebhookPayload {
        from: "+19995551234".to_string(), // Unknown number
        to: "+18005551234".to_string(),
        body: "Hello".to_string(),
        message_sid: "SM_test_unknown".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Should not be called");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[serial]
async fn test_process_sms_phone_service_deactivated() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    // Deactivate phone service (e.g., stolen phone scenario)
    deactivate_phone_service(&state, user.id);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Hello".to_string(),
        message_sid: "SM_test_deactivated".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Should not be called");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, response) = process_sms(&state, payload, options).await;

    // Should reject with FORBIDDEN - phone service is deactivated
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(
        response.message.contains("deactivated"),
        "Response should mention deactivation: {}",
        response.message
    );

    // Credits should be unchanged
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 10.0);
    assert_eq!(updated.credits, 5.0);
}

#[tokio::test]
#[serial]
async fn test_process_sms_cancel_message() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "c".to_string(), // Cancel command
        message_sid: "SM_test_cancel".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Should not be called");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // Credits should NOT be deducted for cancel command
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 10.0); // Unchanged
    assert_eq!(updated.credits, 5.0); // Unchanged
}

#[tokio::test]
#[serial]
async fn test_process_sms_byot_user_skips_credit_check() {
    let state = create_test_state();
    // User with NO credits at all
    let params = TestUserParams::us_user(0.0, 0.0);
    let user = create_test_user(&state, &params);

    // Set BYOT credentials - user pays Twilio directly
    set_byot_credentials(&state, user.id);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Hello from BYOT user".to_string(),
        message_sid: "SM_test_byot".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Hello BYOT user!");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    // Should succeed despite zero credits (BYOT skips credit check)
    assert_eq!(status, StatusCode::OK);

    // Credits still zero (no deduction happened, but also no rejection)
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 0.0);
    assert_eq!(updated.credits, 0.0);
}

// ============================================================
// Input Edge Case Tests
// ============================================================

#[tokio::test]
#[serial]
async fn test_process_sms_empty_message_body() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "".to_string(), // Empty message
        message_sid: "SM_test_empty".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("I received an empty message");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    // Empty message should still be processed
    assert_eq!(status, StatusCode::OK);

    // SMS credits deducted at Twilio callback, not at send time
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 10.0); // Unchanged at send time
}

#[tokio::test]
#[serial]
async fn test_process_sms_whitespace_only_message() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "   ".to_string(), // Whitespace only
        message_sid: "SM_test_whitespace".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("I received whitespace");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn test_process_sms_very_long_message() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "a".repeat(10000), // 10,000 character message
        message_sid: "SM_test_long_input".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Received your long message");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn test_process_sms_unicode_emoji_message() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Hello 👋 世界 🌍 Привет".to_string(), // Unicode and emoji
        message_sid: "SM_test_unicode".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Hello! 你好!");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn test_process_sms_uppercase_cancel() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "C".to_string(), // Uppercase cancel
        message_sid: "SM_test_uppercase_cancel".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Should not be called");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // Credits should NOT be deducted for cancel
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 10.0);
}

// ============================================================
// Credit Boundary Tests
// ============================================================

#[tokio::test]
#[serial]
async fn test_process_sms_low_credits_still_processes() {
    let state = create_test_state();
    // User with low credits_left - SMS pre-check only requires > 0.01
    let params = TestUserParams::us_user(1.0, 0.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Hello".to_string(),
        message_sid: "SM_test_exact_credits".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Hello!");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // SMS credits deducted at Twilio callback, not at send time
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 1.0); // Unchanged at send time
}

#[tokio::test]
#[serial]
async fn test_process_sms_fractional_credits_accepted() {
    let state = create_test_state();
    // User with 0.5 credits_left - SMS pre-check only requires > 0.01
    let params = TestUserParams::us_user(0.5, 0.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Hello".to_string(),
        message_sid: "SM_test_fractional".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Hello there!");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    // Should succeed - SMS only requires credits > 0.01
    assert_eq!(status, StatusCode::OK);

    // Credits unchanged at send time (deducted at callback)
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 0.5);
}

#[tokio::test]
#[serial]
async fn test_process_sms_negative_credits_rejected() {
    let state = create_test_state();
    // User with negative credits (data corruption scenario)
    let params = TestUserParams::us_user(-1.0, 0.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Hello".to_string(),
        message_sid: "SM_test_negative".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Should not be called");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    // Should be rejected
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED);
}

// ============================================================
// MMS/Media Tests
// ============================================================

#[tokio::test]
#[serial]
async fn test_process_sms_with_image_attachment() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "What is this?".to_string(),
        message_sid: "SM_test_image".to_string(),
        num_media: Some("1".to_string()),
        media_url0: Some("https://api.twilio.com/test/Media/ME123".to_string()),
        media_content_type0: Some("image/jpeg".to_string()),
    };

    let mock = MockLlmResponse::with_direct_response("I see an image");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // SMS credits deducted at Twilio callback, not at send time
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 10.0); // Unchanged at send time
}

#[tokio::test]
#[serial]
async fn test_process_sms_with_multiple_attachments() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    // num_media = "2" but we only have media_url0 (first attachment)
    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Multiple images".to_string(),
        message_sid: "SM_test_multi_media".to_string(),
        num_media: Some("2".to_string()),
        media_url0: Some("https://api.twilio.com/test/Media/ME123".to_string()),
        media_content_type0: Some("image/png".to_string()),
    };

    let mock = MockLlmResponse::with_direct_response("I see the first image");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    // Should process successfully (only first media is handled)
    assert_eq!(status, StatusCode::OK);
}

// ============================================================
// LLM Response Edge Case Tests
// ============================================================

#[tokio::test]
#[serial]
async fn test_process_sms_long_llm_response() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Tell me a story".to_string(),
        message_sid: "SM_test_long_response".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    // LLM returns a 1000+ character response
    let mock = MockLlmResponse::with_long_response(1500);
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    // Should handle long response (may be segmented for SMS)
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn test_process_sms_empty_llm_response() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Hello".to_string(),
        message_sid: "SM_test_empty_response".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    // LLM returns empty response (no tool calls, empty content)
    let mock = MockLlmResponse::with_empty_response();
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    // With tool_choice: Auto, an empty text response with finish_reason: stop
    // is valid (LLM chose to respond with empty text). Returns OK.
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[serial]
async fn test_process_sms_invalid_tool_call() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Hello".to_string(),
        message_sid: "SM_test_invalid_tool".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    // LLM returns invalid/malformed tool call
    let mock = MockLlmResponse::with_invalid_tool_call();
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    // Invalid/malformed tool call is a system error - should return failure
    // This is abnormal LLM behavior, not a user input issue
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

// ============================================================
// Subscription Tier Tests
// ============================================================

#[tokio::test]
#[serial]
async fn test_process_sms_user_with_no_sub_tier() {
    let state = create_test_state();
    // User with sub_tier = None (no subscription)
    let params = TestUserParams::us_user_with_tier(10.0, 5.0, None);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Hello".to_string(),
        message_sid: "SM_test_no_tier".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Should not be called");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, response) = process_sms(&state, payload, options).await;

    // Should be rejected - no subscription
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(response.message.contains("subscription"));

    // Credits unchanged
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 10.0);
}

#[tokio::test]
#[serial]
async fn test_process_sms_user_with_invalid_sub_tier() {
    let state = create_test_state();
    // User with invalid sub_tier (not "tier 2")
    let params = TestUserParams::us_user_with_tier(10.0, 5.0, Some("tier 1".to_string()));
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Hello".to_string(),
        message_sid: "SM_test_invalid_tier".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Should not be called");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, response) = process_sms(&state, payload, options).await;

    // Should be rejected - wrong tier
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(response.message.contains("subscription"));
}

// ============================================================
// Tool Error Handling Tests
// ============================================================

#[tokio::test]
#[serial]
async fn test_process_sms_missing_function_name_returns_internal_error() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Search for something".to_string(),
        message_sid: "SM_test_missing_name".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    // LLM returns tool call with missing function name
    let mock = MockLlmResponse::with_missing_function_name();
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, response) = process_sms(&state, payload, options).await;

    // Should return INTERNAL_SERVER_ERROR for LLM malformed response
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        response.message.contains("encountered an issue"),
        "Response should contain error message: {}",
        response.message
    );

    // Credits should NOT be deducted for internal errors
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 10.0);
}

#[tokio::test]
#[serial]
async fn test_process_sms_missing_arguments_returns_internal_error() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "What is the weather?".to_string(),
        message_sid: "SM_test_missing_args".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    // LLM returns tool call with missing arguments
    let mock = MockLlmResponse::with_missing_arguments();
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, response) = process_sms(&state, payload, options).await;

    // Should return INTERNAL_SERVER_ERROR for LLM malformed response
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        response.message.contains("encountered an issue"),
        "Response should contain error message: {}",
        response.message
    );
}

#[tokio::test]
#[serial]
async fn test_process_sms_malformed_json_continues_gracefully() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Search for something".to_string(),
        message_sid: "SM_test_bad_json".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    // LLM returns tool call with malformed JSON arguments (use terminal tool
    // so the single-response mock doesn't cause a second LLM call)
    let mock = MockLlmResponse::with_malformed_json_arguments("set_reminder");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, response) = process_sms(&state, payload, options).await;

    // Should return OK with a fallback response (graceful degradation)
    assert_eq!(status, StatusCode::OK);
    // With malformed JSON, set_reminder fails and the error becomes the response
    assert!(
        !response.message.is_empty(),
        "Response should not be empty: {}",
        response.message
    );
}

#[tokio::test]
#[serial]
async fn test_process_sms_error_message_does_not_leak_content() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "My secret password is hunter2".to_string(), // Sensitive content
        message_sid: "SM_test_privacy".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    // Trigger error with missing function name
    let mock = MockLlmResponse::with_missing_function_name();
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    // Error message should NOT contain user's message content
    assert!(
        !response.message.contains("hunter2"),
        "Error message should not contain user content"
    );
    assert!(
        !response.message.contains("password"),
        "Error message should not contain user content"
    );
    assert!(
        !response.message.contains("secret"),
        "Error message should not contain user content"
    );
}

// ============================================================
// Behavioral Contract Tests
// ============================================================
// These tests verify observable outcomes regardless of implementation details.
// They should pass even after refactoring as long as behavior is correct.

/// Contract A.2: Response fits SMS length limit (<= 480 chars)
#[tokio::test]
#[serial]
async fn test_contract_response_fits_sms() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Tell me a long story".to_string(),
        message_sid: "SM_contract_sms_length".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    // Even with a long LLM response, the final SMS should be truncated
    let mock = MockLlmResponse::with_long_response(1500);
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);
    assert_sms_deliverable(&response.message);
}

/// Contract B.1: Successful message + callback charges user
#[tokio::test]
#[serial]
async fn test_contract_successful_message_charges_user() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "What is 2+2?".to_string(),
        message_sid: "SM_contract_charges".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("4");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // No deduction at send time
    let mid_credits = get_total_credits(&state, user.id);
    assert_not_charged(15.0, mid_credits); // 10.0 + 5.0 unchanged

    // Simulate Twilio callback - this is where deduction happens
    let before_credits = get_total_credits(&state, user.id);
    deduct_from_twilio_price(&state, user.id, 0.0075).unwrap();
    let after_credits = get_total_credits(&state, user.id);
    assert_charged(before_credits, after_credits);
}

/// Contract B.2: Failed message preserves credits
#[tokio::test]
#[serial]
async fn test_contract_failed_message_preserves_credits() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let before_credits = get_total_credits(&state, user.id);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "Test message".to_string(),
        message_sid: "SM_contract_preserves".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    // Trigger a system error with missing function name
    let mock = MockLlmResponse::with_missing_function_name();
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    // Status should indicate failure
    assert_ne!(status, StatusCode::OK);

    // Credits should be preserved
    let after_credits = get_total_credits(&state, user.id);
    assert_not_charged(before_credits, after_credits);
}

/// Contract D.2: Error messages protect privacy (don't leak user content)
#[tokio::test]
#[serial]
async fn test_contract_error_messages_protect_privacy() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let sensitive_content = "My bank account is 123456789 and password is SuperSecret!";

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: sensitive_content.to_string(),
        message_sid: "SM_contract_privacy".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    // Trigger error
    let mock = MockLlmResponse::with_missing_function_name();
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (_status, _headers, response) = process_sms(&state, payload, options).await;

    // Response should not contain any of the sensitive content
    assert_no_content_leak(sensitive_content, &response.message);
    assert_no_content_leak("123456789", &response.message);
    assert_no_content_leak("SuperSecret", &response.message);
}

/// Contract E.1: "forget" command clears context
#[tokio::test]
#[serial]
async fn test_contract_forget_command_clears_context() {
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    // First message to establish context
    let payload1 = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "My name is Alice".to_string(),
        message_sid: "SM_contract_context1".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock1 = MockLlmResponse::with_direct_response("Nice to meet you Alice!");
    let options1 = ProcessSmsOptions::test_with_mock(mock1.to_response());
    let (status1, _headers1, _response1) = process_sms(&state, payload1, options1).await;
    assert_eq!(status1, StatusCode::OK);

    // Second message with "forget" prefix
    let payload2 = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+18005551234".to_string(),
        body: "forget what is my name?".to_string(),
        message_sid: "SM_contract_context2".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock2 = MockLlmResponse::with_direct_response("I don't know your name.");
    let options2 = ProcessSmsOptions::test_with_mock(mock2.to_response());
    let (status2, _headers2, response2) = process_sms(&state, payload2, options2).await;

    assert_eq!(status2, StatusCode::OK);
    // The response should indicate the context was cleared (LLM won't remember Alice)
    // Since we're using a mock, we verify the request processed successfully
    assert!(!response2.message.is_empty());
}

// ============================================================
// Additional Country Coverage Tests
// ============================================================
// These tests ensure all country branches in TwilioMessageService are covered

#[tokio::test]
#[serial]
async fn test_process_sms_canada_user_full_flow() {
    let state = create_test_state();
    let params = TestUserParams::canada_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+16475551000".to_string(),
        body: "What's the weather in Toronto?".to_string(),
        message_sid: "SM_test_ca_123".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("It's cold in Toronto");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // SMS credits deducted at Twilio callback, not at send time
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 10.0); // Unchanged at send time
}

#[tokio::test]
#[serial]
async fn test_process_sms_netherlands_user_full_flow() {
    let state = create_test_state();
    let params = TestUserParams::netherlands_user(5.0, 2.5);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+31201234567".to_string(),
        body: "Wat is het weer?".to_string(),
        message_sid: "SM_test_nl_123".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Het is bewolkt");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // SMS credits deducted at Twilio callback, not at send time
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 5.0); // Unchanged at send time
    assert_eq!(updated.credits, 2.5); // Unchanged
}

#[tokio::test]
#[serial]
async fn test_process_sms_australia_user_full_flow() {
    let state = create_test_state();
    let params = TestUserParams::australia_user(5.0, 2.5);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+61291234567".to_string(),
        body: "G'day! What's the weather?".to_string(),
        message_sid: "SM_test_au_123".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Sunny day mate!");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // SMS credits deducted at Twilio callback, not at send time
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 5.0); // Unchanged at send time
    assert_eq!(updated.credits, 2.5); // Unchanged
}

#[tokio::test]
#[serial]
async fn test_process_sms_france_user_full_flow() {
    let state = create_test_state();
    let params = TestUserParams::france_user(5.0, 2.5);
    let user = create_test_user(&state, &params);

    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+33612345678".to_string(),
        body: "Quel temps fait-il?".to_string(),
        message_sid: "SM_test_fr_123".to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
    };

    let mock = MockLlmResponse::with_direct_response("Il fait beau");
    let options = ProcessSmsOptions::test_with_mock(mock.to_response());

    let (status, _headers, _response) = process_sms(&state, payload, options).await;

    assert_eq!(status, StatusCode::OK);

    // SMS credits deducted at Twilio callback, not at send time
    let updated = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(updated.credits_left, 5.0); // Unchanged at send time
    assert_eq!(updated.credits, 2.5); // Unchanged
}

// ============================================================
// Assertion Utility Tests
// ============================================================

#[test]
#[serial]
fn test_canada_user_params_has_correct_phone_format() {
    let params = TestUserParams::canada_user(10.0, 5.0);
    // Canada uses +1 but with Canadian area codes (e.g., 647 for Toronto)
    assert!(
        params.phone_number.starts_with("+1"),
        "Canada phone should start with +1 (NANP)"
    );
    // Area code 647 is Toronto
    assert!(
        params.phone_number.contains("647"),
        "Canada test phone should use Toronto area code 647"
    );
}

#[test]
#[serial]
fn test_netherlands_user_params_has_correct_phone_format() {
    let params = TestUserParams::netherlands_user(5.0, 2.5);
    assert!(
        params.phone_number.starts_with("+31"),
        "Netherlands phone should start with +31"
    );
}

#[test]
#[serial]
fn test_australia_user_params_has_correct_phone_format() {
    let params = TestUserParams::australia_user(5.0, 2.5);
    assert!(
        params.phone_number.starts_with("+61"),
        "Australia phone should start with +61"
    );
}

#[test]
#[serial]
fn test_france_user_params_has_correct_phone_format() {
    let params = TestUserParams::france_user(5.0, 2.5);
    assert!(
        params.phone_number.starts_with("+33"),
        "France phone should start with +33"
    );
}
