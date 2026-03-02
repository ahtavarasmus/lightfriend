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

/// Assert that an item was created with no [type:tracking] tag and due_at set
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
        !item.summary.contains("[type:tracking]"),
        "Expected no [type:tracking] tag for a reminder"
    );
    assert!(
        item.due_at.is_some(),
        "Expected due_at to be set for a reminder, got None"
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

/// Assert that an item was created with [type:tracking] tag present.
/// `expect_due_at`: whether due_at should be set (time-bounded tracking items)
/// or absent (open-ended tracking items).
fn assert_monitor_item(
    state: &Arc<AppState>,
    user: &User,
    response: &TwilioResponse,
    expected_summary_words: &[&str],
    expect_due_at: bool,
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
        item.summary.contains("[type:tracking]"),
        "Expected [type:tracking] tag for a tracking item, got: {}",
        item.summary
    );

    if expect_due_at {
        assert!(
            item.due_at.is_some(),
            "Expected due_at to be set for a time-bounded tracking item, got None. Summary: {}",
            item.summary
        );
    } else {
        assert!(
            item.due_at.is_none(),
            "Expected due_at to be None for an open-ended tracking item, got {:?}. Summary: {}",
            item.due_at,
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

/// Assert due_at is approximately `expected_offset_secs` from now.
/// Uses asymmetric tolerance: early_tolerance allows the LLM to round down slightly,
/// late_tolerance is tighter because being late is usually worse than being early.
fn assert_due_at_approx(
    state: &Arc<AppState>,
    user: &User,
    expected_offset_secs: i64,
    early_tolerance_secs: i64,
    late_tolerance_secs: i64,
) {
    let items = get_user_items(state, user.id);
    let item = &items[0];
    let check_at = item.due_at.expect("due_at should be set") as i64;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let expected = now + expected_offset_secs;
    let diff = check_at - expected;
    // diff < 0 means early, diff > 0 means late
    assert!(
        diff > -early_tolerance_secs && diff < late_tolerance_secs,
        "due_at is off by {}s (negative=early, positive=late). \
         check_at={}, expected={}, tolerance: -{}/+{}s",
        diff,
        check_at,
        expected,
        early_tolerance_secs,
        late_tolerance_secs,
    );
}

// =============================================================================
// 1. Scheduled reminders (oneshot type)
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
    assert_due_at_approx(&state, &user, 3 * 3600, 15 * 60, 10 * 60);
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
// 2. Tracking items (tracking type)
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
    // Has a deadline: "by Friday" - should have due_at set
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
        !item.summary.contains("[type:tracking]"),
        "Expected no [type:tracking] tag for a scheduled future action"
    );
    assert!(
        item.due_at.is_some(),
        "Expected due_at set for a future action"
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
        !item.summary.contains("[type:tracking]"),
        "Expected no [type:tracking] tag for a scheduled future action"
    );
    assert!(
        item.due_at.is_some(),
        "Expected due_at set for a future action"
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

// =============================================================================
// Tracking creation tests (C1-C10): verify tags are set correctly
// =============================================================================

/// Assert that a tracking item was created with specific tags on the first line of its summary.
/// Each tag is a (key, value) pair, e.g. ("platform", "email") checks for "[platform:email]".
fn assert_tracking_tags(
    state: &Arc<AppState>,
    user: &User,
    response: &TwilioResponse,
    expected_tags: &[(&str, &str)],
) {
    assert!(
        response.created_item_id.is_some(),
        "Expected a tracking item to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let items = get_user_items(state, user.id);
    assert!(!items.is_empty(), "Expected at least one item in DB");

    let item = &items[0];
    assert!(
        item.summary.contains("[type:tracking]"),
        "Expected [type:tracking] tag. Got summary: {}",
        item.summary
    );

    let first_line = item.summary.lines().next().unwrap_or("");
    for (key, value) in expected_tags {
        let tag = format!("[{}:{}]", key, value);
        assert!(
            first_line.to_lowercase().contains(&tag.to_lowercase()),
            "Expected tag {} on first line. First line: {}. Full summary: {}",
            tag,
            first_line,
            item.summary
        );
    }
}

// C1: Email from person with topic
#[tokio::test]
#[ignore]
async fn test_c1_email_from_person_with_topic() {
    let (state, user, response) =
        send_message_with_retry("let me know if I get an email from my boss about the budget")
            .await;

    assert_tracking_tags(
        &state,
        &user,
        &response,
        &[
            ("platform", "email"),
            ("sender", "boss"),
            ("topic", "budget"),
        ],
    );

    let items = get_user_items(&state, user.id);
    eprintln!("  Summary: {}", items[0].summary);
}

// C2: WhatsApp scope:any
#[tokio::test]
#[ignore]
async fn test_c2_whatsapp_scope_any() {
    let (state, user, response) =
        send_message_with_retry("tell me whenever mom texts me on WhatsApp").await;

    assert_tracking_tags(
        &state,
        &user,
        &response,
        &[
            ("platform", "whatsapp"),
            ("sender", "mom"),
            ("scope", "any"),
        ],
    );

    let items = get_user_items(&state, user.id);
    eprintln!("  Summary: {}", items[0].summary);
}

// C3: Topic-only tracking
#[tokio::test]
#[ignore]
async fn test_c3_topic_only_tracking() {
    let (state, user, response) =
        send_message_with_retry("notify me if anyone mentions the hackathon").await;

    assert_tracking_tags(&state, &user, &response, &[("topic", "hackathon")]);

    let items = get_user_items(&state, user.id);
    eprintln!("  Summary: {}", items[0].summary);
}

// C4: Implicit platform from context
#[tokio::test]
#[ignore]
async fn test_c4_implicit_platform_email() {
    let (state, user, response) =
        send_message_with_retry("watch for a shipping confirmation email from Amazon").await;

    assert_tracking_tags(
        &state,
        &user,
        &response,
        &[("platform", "email"), ("sender", "Amazon")],
    );

    // Should NOT have platform:any since user said "email"
    let items = get_user_items(&state, user.id);
    let first_line = items[0].summary.lines().next().unwrap_or("");
    assert!(
        !first_line.to_lowercase().contains("[platform:any]"),
        "C4: Should infer platform:email, not platform:any. First line: {}",
        first_line
    );

    eprintln!("  Summary: {}", items[0].summary);
}

// C5: Deadline extraction - due_at should be set and in the future
#[tokio::test]
#[ignore]
async fn test_c5_deadline_extraction() {
    let (state, user, response) =
        send_message_with_retry("watch my email for reply from John, need it by Friday").await;

    assert_tracking_tags(
        &state,
        &user,
        &response,
        &[("platform", "email"), ("sender", "John")],
    );

    let items = get_user_items(&state, user.id);
    let item = &items[0];
    assert!(
        item.due_at.is_some(),
        "C5: 'by Friday' should set due_at. Got None. Summary: {}",
        item.summary
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    assert!(
        item.due_at.unwrap() > now,
        "C5: due_at should be in the future. Got: {}, now: {}",
        item.due_at.unwrap(),
        now
    );

    eprintln!("  Summary: {}", item.summary);
    eprintln!("  due_at: {:?}", item.due_at);
}

// C6: Fetch source for internet data
#[tokio::test]
#[ignore]
async fn test_c6_fetch_internet() {
    let (state, user, response) = send_message_with_retry("track if Bitcoin hits $100k").await;

    assert!(
        response.created_item_id.is_some(),
        "C6: Expected item created. Response: {}",
        response.message
    );

    let items = get_user_items(&state, user.id);
    let first_line = items[0].summary.lines().next().unwrap_or("");
    assert!(
        first_line.to_lowercase().contains("[fetch:internet]"),
        "C6: Bitcoin tracking should have [fetch:internet]. First line: {}",
        first_line
    );

    eprintln!("  Summary: {}", items[0].summary);
}

// C7: Call notification type
#[tokio::test]
#[ignore]
async fn test_c7_call_notification_type() {
    let (state, user, response) =
        send_message_with_retry("call me when my package arrives from Amazon").await;

    assert!(
        response.created_item_id.is_some(),
        "C7: Expected item created. Response: {}",
        response.message
    );

    let items = get_user_items(&state, user.id);
    let first_line = items[0].summary.lines().next().unwrap_or("");
    assert!(
        first_line.contains("[notify:call]"),
        "C7: 'call me when' should produce [notify:call]. First line: {}",
        first_line
    );
    assert_eq!(
        items[0].priority, 2,
        "C7: Call items should have priority 2, got: {}",
        items[0].priority
    );

    eprintln!("  Summary: {}", items[0].summary);
}

// C8: Recurring not tracking - daily digest should be recurring
#[tokio::test]
#[ignore]
async fn test_c8_recurring_not_tracking() {
    let (state, user, response) =
        send_message_with_retry("give me a daily morning briefing with my emails and calendar")
            .await;

    assert!(
        response.created_item_id.is_some(),
        "C8: Expected item created. Response: {}",
        response.message
    );

    let items = get_user_items(&state, user.id);
    let summary = &items[0].summary;
    assert!(
        summary.contains("[type:recurring]"),
        "C8: Daily briefing should be [type:recurring], got: {}",
        summary
    );
    assert!(
        !summary.contains("[type:tracking]"),
        "C8: Should NOT be [type:tracking]. Got: {}",
        summary
    );

    // Should have fetch sources
    let first_line = summary.lines().next().unwrap_or("");
    assert!(
        first_line.to_lowercase().contains("[fetch:"),
        "C8: Should have fetch sources for email/calendar. First line: {}",
        first_line
    );

    eprintln!("  Summary: {}", summary);
}

// C9: Tracking not recurring - one-time watch
#[tokio::test]
#[ignore]
async fn test_c9_tracking_not_recurring() {
    let (state, user, response) =
        send_message_with_retry("let me know when I get an email from John").await;

    assert!(
        response.created_item_id.is_some(),
        "C9: Expected item created. Response: {}",
        response.message
    );

    let items = get_user_items(&state, user.id);
    let summary = &items[0].summary;
    assert!(
        summary.contains("[type:tracking]"),
        "C9: 'let me know when' should be [type:tracking], got: {}",
        summary
    );
    assert!(
        !summary.contains("[type:recurring]"),
        "C9: Should NOT be [type:recurring]. Got: {}",
        summary
    );

    eprintln!("  Summary: {}", summary);
}

// C10: Multilingual - Finnish tracking request
#[tokio::test]
#[ignore]
async fn test_c10_multilingual_finnish() {
    let (state, user, response) =
        send_message_with_retry("kerro mulle kun aiti laittaa viestin").await;

    assert!(
        response.created_item_id.is_some(),
        "C10: Expected item created. Response: {}",
        response.message
    );

    let items = get_user_items(&state, user.id);
    let summary = &items[0].summary;
    assert!(
        summary.contains("[type:tracking]"),
        "C10: Finnish tracking request should create [type:tracking]. Got: {}",
        summary
    );

    // Sender tag should reference mom/aiti
    let first_line = summary.lines().next().unwrap_or("").to_lowercase();
    assert!(
        first_line.contains("[sender:")
            && (first_line.contains("mom")
                || first_line.contains("aiti")
                || first_line.contains("äiti")
                || first_line.contains("mother")),
        "C10: Should have sender tag with mom/aiti/äiti/mother. First line: {}",
        first_line
    );

    eprintln!("  Summary: {}", summary);
}
