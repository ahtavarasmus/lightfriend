//! LLM Integration Tests
//!
//! These tests call the real LLM (via Tinfoil) to verify end-to-end behavior:
//! user sends natural language -> AI responds or calls tools.
//!
//! All tests are gated with `#[ignore]` because they cost real API tokens.
//! Run explicitly with: `cargo test --test llm_integration_test -- --ignored --test-threads=1`
//!
//! Requirements:
//! - TINFOIL_API_KEY and OPENROUTER_API_KEY set in backend/.env
//! - Network access to Tinfoil API

use backend::api::twilio_sms::{
    process_sms, ProcessSmsOptions, TwilioResponse, TwilioWebhookPayload,
};
use backend::models::user_models::User;
use backend::test_utils::{create_test_state, create_test_user, set_plan_type, TestUserParams};
use backend::{AiConfig, AppState, UserCoreOps};
use std::sync::Arc;

// =============================================================================
// Helpers
// =============================================================================

/// Create an AppState with real LLM credentials (loads from .env)
fn create_llm_test_state() -> Arc<AppState> {
    dotenvy::dotenv().ok();
    let state = create_test_state();
    let real_ai_config = AiConfig::from_env();
    let mut inner =
        Arc::try_unwrap(state).unwrap_or_else(|_| panic!("Only one reference should exist"));
    inner.ai_config = real_ai_config;
    Arc::new(inner)
}

/// Create a test user with location, timezone, and autopilot plan set.
/// Autopilot is needed so tracking items pass the plan gate.
fn setup_user_with_location(state: &Arc<AppState>) -> User {
    let params = TestUserParams::finland_user(100.0, 100.0);
    let user = create_test_user(state, &params);

    state
        .user_core
        .ensure_user_info_exists(user.id)
        .expect("Failed to ensure user_info exists");
    state
        .user_core
        .update_location(user.id, "Tampere, Finland")
        .expect("Failed to update location");
    state
        .user_core
        .update_timezone(user.id, "Europe/Helsinki")
        .expect("Failed to update timezone");

    // Set autopilot plan so tracking items are allowed
    set_plan_type(state, user.id, "autopilot");

    user
}

/// Max time for a single LLM round-trip (includes tool calls, retries, and follow-up)
const SEND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Send a message through the full SMS processing pipeline (with real LLM, no Twilio send)
async fn send_message(state: &Arc<AppState>, user: &User, body: &str) -> TwilioResponse {
    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+15551234567".to_string(),
        body: body.to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
        message_sid: format!("SM_test_{}", uuid::Uuid::new_v4()),
    };

    let options = ProcessSmsOptions {
        skip_twilio_send: true,
        mock_llm_response: None,
        status_tx: None,
        mock_tool_responses: None,
    };

    let (_status, _headers, axum::Json(response)) =
        tokio::time::timeout(SEND_TIMEOUT, process_sms(state, payload, options))
            .await
            .unwrap_or_else(|_| panic!("send_message timed out after {}s", SEND_TIMEOUT.as_secs()));
    response
}

/// Send a message with mock tool responses. Tool calls matching keys in the map
/// return the mock value instead of executing the real handler.
#[allow(dead_code)]
async fn send_message_with_mocks(
    state: &Arc<AppState>,
    user: &User,
    body: &str,
    mocks: std::collections::HashMap<String, String>,
) -> TwilioResponse {
    let payload = TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: "+15551234567".to_string(),
        body: body.to_string(),
        num_media: None,
        media_url0: None,
        media_content_type0: None,
        message_sid: format!("SM_test_{}", uuid::Uuid::new_v4()),
    };

    let options = ProcessSmsOptions {
        skip_twilio_send: true,
        mock_llm_response: None,
        status_tx: None,
        mock_tool_responses: Some(mocks),
    };

    let (_status, _headers, axum::Json(response)) =
        tokio::time::timeout(SEND_TIMEOUT, process_sms(state, payload, options))
            .await
            .unwrap_or_else(|_| panic!("send_message timed out after {}s", SEND_TIMEOUT.as_secs()));
    response
}

/// Assert that NO item was created (question/realtime request)
fn assert_no_item(response: &TwilioResponse) {
    assert!(
        response.created_item_id.is_none(),
        "Expected NO item to be created. Got item_id: {:?}. Response: {}",
        response.created_item_id,
        response.message
    );
    assert!(
        !response.message.is_empty(),
        "Expected non-empty response message"
    );
}

// =============================================================================
// 3. Questions - should NOT create items
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_llm_question_no_item() {
    let state = create_llm_test_state();
    let user = setup_user_with_location(&state);

    let response = send_message(&state, &user, "what time is it?").await;
    assert_no_item(&response);
}

#[tokio::test]
#[ignore]
async fn test_llm_email_check_no_item() {
    let state = create_llm_test_state();
    let user = setup_user_with_location(&state);

    let response = send_message(&state, &user, "check my email").await;
    assert_no_item(&response);
}

#[tokio::test]
#[ignore]
async fn test_llm_weather_question_no_item() {
    let state = create_llm_test_state();
    let user = setup_user_with_location(&state);

    let response = send_message(&state, &user, "what's the weather like right now?").await;
    assert_no_item(&response);
}

#[tokio::test]
#[ignore]
async fn test_llm_question_looks_like_task() {
    let state = create_llm_test_state();
    let user = setup_user_with_location(&state);

    let response = send_message(
        &state,
        &user,
        "tomorrow at 8am I have a dentist appointment, what should I bring?",
    )
    .await;

    assert!(
        response.created_item_id.is_none(),
        "Expected NO item - user is asking a question, not scheduling. Got item_id: {:?}",
        response.created_item_id
    );
    assert!(
        !response.message.is_empty(),
        "Expected non-empty response (AI answers the question)"
    );
}

// =============================================================================
// 4. Multilingual questions - should NOT create items
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_llm_finnish_question_no_item() {
    let state = create_llm_test_state();
    let user = setup_user_with_location(&state);

    let response = send_message(&state, &user, "mika kello on?").await;
    assert_no_item(&response);
}

#[tokio::test]
#[ignore]
async fn test_llm_spanish_question_no_item() {
    let state = create_llm_test_state();
    let user = setup_user_with_location(&state);

    let response = send_message(&state, &user, "que hora es?").await;
    assert_no_item(&response);
}
