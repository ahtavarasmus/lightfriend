//! LLM Integration Tests for Conditional Tasks
//!
//! These tests call the real LLM (via OpenRouter) to verify end-to-end behavior:
//! user sends natural language -> AI creates task with correct sources/condition/action -> DB state is correct.
//!
//! All tests are gated with `#[ignore]` because they cost real API tokens.
//! Run explicitly with: `cargo test --test llm_integration_test -- --ignored`
//!
//! Requirements:
//! - OPENROUTER_API_KEY set in backend/.env
//! - Network access to OpenRouter API
//!
//! LLM calls are non-deterministic. Tests that expect task creation retry up to
//! MAX_RETRIES times with a fresh state each attempt to handle transient failures
//! (e.g. LLM produces malformed tool arguments on first try).

use backend::api::twilio_sms::{
    process_sms, ProcessSmsOptions, TwilioResponse, TwilioWebhookPayload,
};
use backend::models::user_models::User;
use backend::test_utils::{create_test_state, create_test_user, get_user_tasks, TestUserParams};
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

/// Create a test user with location and timezone set (needed for weather/calendar tasks)
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
        skip_credit_deduction: true,
        mock_llm_response: None,
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
            "  Attempt {}/{} did not create task. LLM response: {}",
            attempt, MAX_RETRIES, response.message
        );
        last_response = Some((state, user, response));
    }
    // Return the last attempt so the caller can assert and get a clear failure message
    last_response.unwrap()
}

// =============================================================================
// Task creation tests - LLM should create tasks with correct fields
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_llm_weather_conditional_task() {
    let (state, user, response) = send_message_with_retry(
        "if it's below freezing at 7am tomorrow, remind me to warm up the car",
    )
    .await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    assert!(!tasks.is_empty(), "Expected at least one task in DB");

    let task = &tasks[0];
    let sources = task.sources.as_deref().unwrap_or("");
    assert!(
        sources.contains("weather"),
        "Expected sources to contain 'weather', got: {:?}",
        task.sources
    );
    assert!(
        task.condition.is_some() && !task.condition.as_ref().unwrap().is_empty(),
        "Expected non-empty condition, got: {:?}",
        task.condition
    );
    assert!(
        task.trigger.starts_with("once_"),
        "Expected trigger starting with 'once_', got: {}",
        task.trigger
    );
    assert!(
        !response.message.is_empty(),
        "Expected non-empty response message"
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_calendar_conditional_task() {
    let (state, user, response) = send_message_with_retry(
        "if I have any meetings before noon tomorrow, remind me at 7am to prepare my notes",
    )
    .await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];
    let sources = task.sources.as_deref().unwrap_or("");
    assert!(
        sources.contains("calendar"),
        "Expected sources to contain 'calendar', got: {:?}",
        task.sources
    );
    assert!(
        task.condition.is_some() && !task.condition.as_ref().unwrap().is_empty(),
        "Expected non-empty condition, got: {:?}",
        task.condition
    );
    assert!(
        task.trigger.starts_with("once_"),
        "Expected once_ trigger, got: {}",
        task.trigger
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_unconditional_reminder() {
    let (state, user, response) =
        send_message_with_retry("remind me at 9am tomorrow to call the dentist").await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];

    // No source needed for a simple reminder
    let sources = task.sources.as_deref().unwrap_or("");
    assert!(
        sources.is_empty(),
        "Expected empty sources for unconditional reminder, got: {:?}",
        task.sources
    );
    // No condition needed
    let condition = task.condition.as_deref().unwrap_or("");
    assert!(
        condition.is_empty(),
        "Expected empty condition for unconditional reminder, got: {:?}",
        task.condition
    );
    assert!(
        task.trigger.starts_with("once_"),
        "Expected once_ trigger, got: {}",
        task.trigger
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_realtime_question() {
    let state = create_llm_test_state();
    let user = setup_user_with_location(&state);

    let response = send_message(&state, &user, "what time is it?").await;

    assert!(
        response.created_item_id.is_none(),
        "Expected NO task for a realtime question. Got task_id: {:?}",
        response.created_item_id
    );
    assert!(
        !response.message.is_empty(),
        "Expected non-empty response for realtime question"
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_email_conditional_task() {
    let (state, user, response) = send_message_with_retry(
        "at 8am tomorrow check my email and if there's anything from my boss, remind me to reply before lunch",
    )
    .await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];
    let sources = task.sources.as_deref().unwrap_or("");
    assert!(
        sources.contains("email"),
        "Expected sources to contain 'email', got: {:?}",
        task.sources
    );
    assert!(
        task.condition.is_some() && !task.condition.as_ref().unwrap().is_empty(),
        "Expected non-empty condition, got: {:?}",
        task.condition
    );
    assert!(
        task.trigger.starts_with("once_"),
        "Expected once_ trigger (not recurring), got: {}",
        task.trigger
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_multi_source_task() {
    let (state, user, response) = send_message_with_retry(
        "at 10am tomorrow look at my calendar and weather both, and if I have an outdoor meeting during rain, remind me to move it indoors",
    )
    .await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];
    let sources = task.sources.as_deref().unwrap_or("");
    assert!(
        sources.contains("calendar") && sources.contains("weather"),
        "Expected sources to contain both 'calendar' and 'weather', got: {:?}",
        task.sources
    );
    assert!(
        task.condition.is_some() && !task.condition.as_ref().unwrap().is_empty(),
        "Expected non-empty condition, got: {:?}",
        task.condition
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_mixed_conversation() {
    // First message: should create a task (with retry)
    let (state, user, response1) =
        send_message_with_retry("remind me at 3pm tomorrow to buy groceries").await;

    assert!(
        response1.created_item_id.is_some(),
        "First message should create a task after {} retries. Response: {}",
        MAX_RETRIES,
        response1.message
    );

    let tasks_after_first = get_user_tasks(&state, user.id);
    assert!(!tasks_after_first.is_empty(), "Should have at least 1 task");

    // Second message: should NOT create a task (realtime question)
    // Use the same state/user so conversation context carries over
    let response2 = send_message(&state, &user, "what's the weather like?").await;

    assert!(
        response2.created_item_id.is_none(),
        "Second message should NOT create a task. Got task_id: {:?}",
        response2.created_item_id
    );
    assert!(
        !response2.message.is_empty(),
        "Expected non-empty response for weather question"
    );
}

// =============================================================================
// Realtime vs task disambiguation tests
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_llm_realtime_email_check() {
    let state = create_llm_test_state();
    let user = setup_user_with_location(&state);

    let response = send_message(&state, &user, "check my email").await;

    assert!(
        response.created_item_id.is_none(),
        "Expected NO task for realtime email check. Got task_id: {:?}",
        response.created_item_id
    );
    assert!(
        !response.message.is_empty(),
        "Expected non-empty response (AI tries to check email now)"
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_realtime_weather_check() {
    let state = create_llm_test_state();
    let user = setup_user_with_location(&state);

    let response = send_message(&state, &user, "what's the weather like right now?").await;

    assert!(
        response.created_item_id.is_none(),
        "Expected NO task for realtime weather question. Got task_id: {:?}",
        response.created_item_id
    );
    assert!(
        !response.message.is_empty(),
        "Expected non-empty response (AI fetches weather in realtime)"
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_tricky_realtime_looks_like_task() {
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
        "Expected NO task - user is asking a question, not creating a reminder. Got task_id: {:?}",
        response.created_item_id
    );
    assert!(
        !response.message.is_empty(),
        "Expected non-empty response (AI answers the question)"
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_tricky_realtime_check_calendar() {
    let state = create_llm_test_state();
    let user = setup_user_with_location(&state);

    let response = send_message(
        &state,
        &user,
        "do I have anything scheduled for tomorrow morning?",
    )
    .await;

    assert!(
        response.created_item_id.is_none(),
        "Expected NO task - user wants to know NOW, not set up a future check. Got task_id: {:?}",
        response.created_item_id
    );
    assert!(!response.message.is_empty(), "Expected non-empty response");
}

// =============================================================================
// Hard scenarios - vague datetimes, remind-vs-do, notification types, recurring
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_llm_vague_time_afternoon() {
    let (state, user, response) =
        send_message_with_retry("remind me tomorrow afternoon to stretch").await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];
    assert!(
        task.trigger.starts_with("once_"),
        "Expected once_ trigger for 'tomorrow afternoon', got: {}",
        task.trigger
    );
    assert!(
        task.action.contains("send_reminder"),
        "Expected action to contain 'send_reminder', got: {}",
        task.action
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_relative_time() {
    let (state, user, response) =
        send_message_with_retry("in 3 hours remind me to check the oven").await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];
    assert!(
        task.trigger.starts_with("once_"),
        "Expected once_ trigger for relative time, got: {}",
        task.trigger
    );

    // Parse the trigger timestamp (Unix epoch) and verify it's roughly 3 hours from now (within 1h tolerance)
    let trigger_ts_str = task.trigger.strip_prefix("once_").unwrap();
    let trigger_ts: i64 = trigger_ts_str.parse().unwrap_or_else(|_| {
        panic!(
            "Failed to parse trigger timestamp as unix epoch: {}",
            trigger_ts_str
        )
    });
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let three_hours = 3 * 3600;
    let one_hour = 3600;
    let diff = (trigger_ts - now - three_hours).abs();
    assert!(
        diff < one_hour,
        "Expected trigger ~3h from now (tolerance 1h). Trigger: {}, now: {}, diff_from_3h: {}s",
        trigger_ts_str,
        now,
        diff
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_implicit_date() {
    let (state, user, response) = send_message_with_retry("remind me at 9pm to take my meds").await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];
    assert!(
        task.trigger.starts_with("once_"),
        "Expected once_ trigger for implicit date, got: {}",
        task.trigger
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_notification_call_me() {
    let (state, user, response) = send_message_with_retry("call me at 6am tomorrow").await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];
    let notif = task.notification_type.as_deref().unwrap_or("");
    assert!(
        notif.contains("call"),
        "Expected notification_type to contain 'call', got: {:?}",
        task.notification_type
    );
    // "call me" should map to send_reminder with call notification, not a nonexistent tool
    assert!(
        task.action.contains("send_reminder"),
        "Expected action to contain 'send_reminder' (not a fabricated tool), got: {}",
        task.action
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_remind_vs_do_remind() {
    let (state, user, response) =
        send_message_with_retry("remind me to turn on my Tesla climate at 8am tomorrow").await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];
    // User said "remind me" - should be a reminder, NOT the actual Tesla action
    assert!(
        task.action.contains("send_reminder"),
        "Expected action to contain 'send_reminder' (user wants a REMINDER), got: {}",
        task.action
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_remind_vs_do_action() {
    let (state, user, response) =
        send_message_with_retry("turn on my Tesla climate at 8am tomorrow").await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];
    // User said "turn on" - should be the actual Tesla action, NOT just a reminder
    assert!(
        task.action.contains("control_tesla"),
        "Expected action to contain 'control_tesla' (user wants the ACTION), got: {}",
        task.action
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_whatsapp_task() {
    let (state, user, response) =
        send_message_with_retry("at 5pm text my wife on WhatsApp that I'm running late").await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];
    assert!(
        task.action.contains("send_chat_message"),
        "Expected action to contain 'send_chat_message', got: {}",
        task.action
    );
    // Should target WhatsApp specifically
    let action_lower = task.action.to_lowercase();
    assert!(
        action_lower.contains("whatsapp"),
        "Expected action to mention 'whatsapp', got: {}",
        task.action
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_recurring_email_watch() {
    let (state, user, response) = send_message_with_retry(
        "watch my email for anything from HR about the job offer and let me know",
    )
    .await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];
    assert!(
        task.trigger == "recurring_email",
        "Expected trigger 'recurring_email', got: {}",
        task.trigger
    );
    assert!(
        task.condition.is_some() && !task.condition.as_ref().unwrap().is_empty(),
        "Expected non-empty condition mentioning HR/job, got: {:?}",
        task.condition
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_recurring_messaging_watch() {
    let (state, user, response) = send_message_with_retry("let me know if mom texts me").await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];
    assert!(
        task.trigger == "recurring_messaging",
        "Expected trigger 'recurring_messaging', got: {}",
        task.trigger
    );
    assert!(
        task.condition.is_some() && !task.condition.as_ref().unwrap().is_empty(),
        "Expected non-empty condition mentioning mom, got: {:?}",
        task.condition
    );
}

#[tokio::test]
#[ignore]
async fn test_llm_weather_source_with_action() {
    let (state, user, response) = send_message_with_retry(
        "at 8am tomorrow if it's above 25 degrees, remind me to water the plants",
    )
    .await;

    assert!(
        response.created_item_id.is_some(),
        "Expected a task to be created after {} retries. Response: {}",
        MAX_RETRIES,
        response.message
    );

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];
    let sources = task.sources.as_deref().unwrap_or("");
    assert!(
        sources.contains("weather"),
        "Expected sources to contain 'weather', got: {:?}",
        task.sources
    );
    assert!(
        task.condition.is_some() && !task.condition.as_ref().unwrap().is_empty(),
        "Expected non-empty condition about temperature, got: {:?}",
        task.condition
    );
    assert!(
        task.action.contains("send_reminder"),
        "Expected action to contain 'send_reminder', got: {}",
        task.action
    );
    assert!(
        task.trigger.starts_with("once_"),
        "Expected once_ trigger, got: {}",
        task.trigger
    );
}
