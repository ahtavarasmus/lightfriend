//! LLM Integration Tests for Item Creation via Chat
//!
//! These tests call the real LLM (via Tinfoil) to verify end-to-end behavior:
//! user sends natural language -> AI calls create_item tool -> items table has correct fields.
//!
//! All tests are gated with `#[ignore]` because they cost real API tokens.
//! Run explicitly with: `cargo test --test llm_integration_test -- --ignored --test-threads=1`
//!
//! Requirements:
//! - TINFOIL_API_KEY and OPENROUTER_API_KEY set in backend/.env
//! - Network access to Tinfoil API
//!
//! LLM calls are non-deterministic. Tests that expect item creation retry up to
//! MAX_RETRIES times with a fresh state each attempt to handle transient failures
//! (e.g. LLM produces malformed tool arguments on first try).

use backend::api::twilio_sms::{
    process_sms, ProcessSmsOptions, TwilioResponse, TwilioWebhookPayload,
};
use backend::models::user_models::User;
use backend::test_utils::{
    create_test_state, create_test_user, get_user_items, set_plan_type, TestUserParams,
};
use backend::{AiConfig, AppState, UserCoreOps};
use std::sync::Arc;

const MAX_RETRIES: usize = 3;

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
/// Autopilot is needed so monitor items pass the plan gate.
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

    // Set autopilot plan so monitor items are allowed
    set_plan_type(state, user.id, "autopilot");

    user
}

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
    };

    let (_status, _headers, axum::Json(response)) = process_sms(state, payload, options).await;
    response
}

/// Send a message with retries. Each retry uses a fresh state + user to avoid
/// stale conversation history from a failed attempt influencing the next one.
/// Returns (state, user, response) from the successful attempt.
async fn send_message_with_retry(body: &str) -> (Arc<AppState>, User, TwilioResponse) {
    let mut last_response = None;
    for attempt in 1..=MAX_RETRIES {
        let state = create_llm_test_state();
        let user = setup_user_with_location(&state);
        let response = send_message(&state, &user, body).await;
        if response.created_item_id.is_some() {
            return (state, user, response);
        }
        eprintln!(
            "  Attempt {}/{} did not create item. LLM response: {}",
            attempt, MAX_RETRIES, response.message
        );
        last_response = Some((state, user, response));
    }
    // Return the last attempt so the caller can assert and get a clear failure message
    last_response.unwrap()
}

/// Assert that an item was created with monitor=false and next_check_at set
fn assert_reminder_item(
    state: &Arc<AppState>,
    user: &User,
    response: &TwilioResponse,
    expected_summary_words: &[&str],
) {
    assert!(
        response.created_item_id.is_some(),
        "Expected an item to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let items = get_user_items(state, user.id);
    assert!(!items.is_empty(), "Expected at least one item in DB");

    let item = &items[0];
    assert!(
        !item.monitor,
        "Expected monitor=false for a reminder, got monitor=true"
    );
    assert!(
        item.next_check_at.is_some(),
        "Expected next_check_at to be set for a reminder, got None"
    );

    let summary_lower = item.summary.to_lowercase();
    for word in expected_summary_words {
        assert!(
            summary_lower.contains(&word.to_lowercase()),
            "Expected summary to contain '{}', got: {}",
            word,
            item.summary
        );
    }
}

/// Assert that an item was created with monitor=true.
/// `expect_next_check_at`: whether next_check_at should be set (time-bounded monitors)
/// or absent (open-ended monitors).
fn assert_monitor_item(
    state: &Arc<AppState>,
    user: &User,
    response: &TwilioResponse,
    expected_summary_words: &[&str],
    expect_next_check_at: bool,
) {
    assert!(
        response.created_item_id.is_some(),
        "Expected an item to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let items = get_user_items(state, user.id);
    assert!(!items.is_empty(), "Expected at least one item in DB");

    let item = &items[0];
    assert!(
        item.monitor,
        "Expected monitor=true for a monitor item, got monitor=false"
    );

    if expect_next_check_at {
        assert!(
            item.next_check_at.is_some(),
            "Expected next_check_at to be set for a time-bounded monitor, got None. Summary: {}",
            item.summary
        );
    } else {
        assert!(
            item.next_check_at.is_none(),
            "Expected next_check_at to be None for an open-ended monitor, got {:?}. Summary: {}",
            item.next_check_at,
            item.summary
        );
    }

    let summary_lower = item.summary.to_lowercase();
    for word in expected_summary_words {
        assert!(
            summary_lower.contains(&word.to_lowercase()),
            "Expected summary to contain '{}', got: {}",
            word,
            item.summary
        );
    }
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

/// Assert next_check_at is approximately `expected_offset_secs` from now.
/// Uses asymmetric tolerance: early_tolerance allows the LLM to round down slightly,
/// late_tolerance is tighter because being late is usually worse than being early.
fn assert_next_check_at_approx(
    state: &Arc<AppState>,
    user: &User,
    expected_offset_secs: i64,
    early_tolerance_secs: i64,
    late_tolerance_secs: i64,
) {
    let items = get_user_items(state, user.id);
    let item = &items[0];
    let check_at = item.next_check_at.expect("next_check_at should be set") as i64;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let expected = now + expected_offset_secs;
    let diff = check_at - expected;
    // diff < 0 means early, diff > 0 means late
    assert!(
        diff > -early_tolerance_secs && diff < late_tolerance_secs,
        "next_check_at is off by {}s (negative=early, positive=late). \
         check_at={}, expected={}, tolerance: -{}/+{}s",
        diff,
        check_at,
        expected,
        early_tolerance_secs,
        late_tolerance_secs,
    );
}

// =============================================================================
// 1. Scheduled reminders (monitor=false)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_llm_simple_reminder() {
    let (state, user, response) =
        send_message_with_retry("remind me at 9am tomorrow to call the dentist").await;

    assert_reminder_item(&state, &user, &response, &["dentist"]);
}

#[tokio::test]
#[ignore]
async fn test_llm_relative_time_reminder() {
    let (state, user, response) =
        send_message_with_retry("in 3 hours remind me to check the oven").await;

    assert_reminder_item(&state, &user, &response, &["oven"]);

    // "in 3 hours" for an oven check: LLM might round to the nearest 15 min,
    // but being late on an oven is bad. Allow 15 min early, 10 min late.
    assert_next_check_at_approx(&state, &user, 3 * 3600, 15 * 60, 10 * 60);
}

#[tokio::test]
#[ignore]
async fn test_llm_implicit_date_reminder() {
    let (state, user, response) = send_message_with_retry("remind me at 9pm to take my meds").await;

    assert_reminder_item(&state, &user, &response, &["med"]);
}

#[tokio::test]
#[ignore]
async fn test_llm_vague_time_reminder() {
    let (state, user, response) =
        send_message_with_retry("remind me tomorrow afternoon to stretch").await;

    assert_reminder_item(&state, &user, &response, &["stretch"]);
}

#[tokio::test]
#[ignore]
async fn test_llm_call_me_reminder() {
    let (state, user, response) = send_message_with_retry("call me at 6am tomorrow").await;

    assert!(
        response.created_item_id.is_some(),
        "Expected an item to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let items = get_user_items(&state, user.id);
    assert!(!items.is_empty(), "Expected at least one item in DB");

    let item = &items[0];
    // New structured format uses [notify:call], legacy used [VIA CALL]
    assert!(
        item.summary.contains("[notify:call]")
            || item.summary.to_uppercase().contains("[VIA CALL]"),
        "Expected summary to contain '[notify:call]' or '[VIA CALL]', got: {}",
        item.summary
    );
    assert!(
        item.priority == 2,
        "Call item should have priority 2, got: {}",
        item.priority
    );
}

// =============================================================================
// 2. Monitors (monitor=true)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_llm_email_monitor() {
    // Open-ended: LLM infers a reasonable safety-net check-in date
    let (state, user, response) =
        send_message_with_retry("let me know if I get an email from HR about the job offer").await;

    assert_monitor_item(&state, &user, &response, &["HR"], true);
}

#[tokio::test]
#[ignore]
async fn test_llm_messaging_monitor() {
    // Open-ended: LLM infers a reasonable safety-net check-in date
    let (state, user, response) = send_message_with_retry("tell me when mom texts me").await;

    assert_monitor_item(&state, &user, &response, &["mom"], true);
}

#[tokio::test]
#[ignore]
async fn test_llm_time_bounded_monitor() {
    // Has a deadline: "by Friday" - should have next_check_at set
    let (state, user, response) = send_message_with_retry(
        "watch my email for a shipping confirmation from Amazon, I need it by Friday",
    )
    .await;

    assert_monitor_item(&state, &user, &response, &["Amazon"], true);
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

#[tokio::test]
#[ignore]
async fn test_llm_calendar_question_no_item() {
    let state = create_llm_test_state();
    let user = setup_user_with_location(&state);

    let response = send_message(
        &state,
        &user,
        "do I have anything scheduled for tomorrow morning?",
    )
    .await;

    assert_no_item(&response);
}

// =============================================================================
// 4. Future actions - must use create_item, not execute immediately
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_llm_future_message_creates_item() {
    let (state, user, response) =
        send_message_with_retry("at 5pm text my wife on WhatsApp that I'm running late").await;

    // Should create an item (scheduled for the future), NOT send immediately
    assert!(
        response.created_item_id.is_some(),
        "Expected an item to be created (future action). Response: {}",
        response.message
    );

    let items = get_user_items(&state, user.id);
    assert!(!items.is_empty(), "Expected at least one item in DB");

    let item = &items[0];
    assert!(
        !item.monitor,
        "Expected monitor=false for a scheduled future action"
    );
    assert!(
        item.next_check_at.is_some(),
        "Expected next_check_at set for a future action"
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_future_email_creates_item() {
    let (state, user, response) =
        send_message_with_retry("at 3pm send an email to john@example.com about the meeting").await;

    assert!(
        response.created_item_id.is_some(),
        "Expected an item to be created (future email). Response: {}",
        response.message
    );

    let items = get_user_items(&state, user.id);
    assert!(!items.is_empty(), "Expected at least one item in DB");

    let item = &items[0];
    assert!(
        !item.monitor,
        "Expected monitor=false for a scheduled future action"
    );
    assert!(
        item.next_check_at.is_some(),
        "Expected next_check_at set for a future action"
    );
}

// =============================================================================
// 5. Mixed conversation
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_llm_mixed_conversation() {
    // First message: should create an item (with retry)
    let (state, user, response1) =
        send_message_with_retry("remind me at 3pm to buy groceries").await;

    assert!(
        response1.created_item_id.is_some(),
        "First message should create an item after {} retries. Response: {}",
        MAX_RETRIES,
        response1.message
    );

    let items_after_first = get_user_items(&state, user.id);
    assert!(
        !items_after_first.is_empty(),
        "Should have at least 1 item after first message"
    );

    // Second message: should NOT create an item (realtime question)
    // Use the same state/user so conversation context carries over
    let response2 = send_message(&state, &user, "what's the weather like?").await;

    assert!(
        response2.created_item_id.is_none(),
        "Second message should NOT create an item. Got item_id: {:?}",
        response2.created_item_id
    );
    assert!(
        !response2.message.is_empty(),
        "Expected non-empty response for weather question"
    );
}

// =============================================================================
// 6. Multilingual - item creation should work regardless of language
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_llm_finnish_reminder() {
    let (state, user, response) =
        send_message_with_retry("muistuta mua huomenna aamulla soittaa hammaslaakarille").await;

    assert_reminder_item(&state, &user, &response, &["hammaslaa"]);
}

#[tokio::test]
#[ignore]
async fn test_llm_spanish_reminder() {
    let (state, user, response) =
        send_message_with_retry("recuerdame manana a las 9 llamar al dentista").await;

    assert_reminder_item(&state, &user, &response, &["dentista"]);
}

#[tokio::test]
#[ignore]
async fn test_llm_german_reminder() {
    let (state, user, response) =
        send_message_with_retry("erinnere mich morgen um 10 Uhr den Zahnarzt anzurufen").await;

    assert_reminder_item(&state, &user, &response, &["Zahnarzt"]);
}

#[tokio::test]
#[ignore]
async fn test_llm_japanese_reminder() {
    let (state, user, response) =
        send_message_with_retry("明日の朝9時に歯医者に電話するのをリマインドして").await;

    assert_reminder_item(&state, &user, &response, &["歯医者"]);
}

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
