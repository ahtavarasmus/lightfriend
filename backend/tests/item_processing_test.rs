//! LLM Integration Tests for Item Processing
//!
//! These tests call the real LLM (via Tinfoil) to verify that when a
//! triggered item fires, `process_triggered_item` returns correct results:
//! - sms_message is concise and actionable
//! - priority maps to the right delivery method (1=SMS, 2=call)
//! - next_check_at is always required at creation; set to null on trigger to delete the item
//! - summary may be updated on trigger (e.g. monitored items append tracking context)
//!
//! All tests are gated with `#[ignore]` because they cost real API tokens.
//! Run explicitly with: `cargo test --test item_processing_test -- --ignored --test-threads=1`
//!
//! Requirements:
//! - TINFOIL_API_KEY and OPENROUTER_API_KEY set in backend/.env
//! - Network access to Tinfoil API

use backend::models::user_models::{Item, User};
use backend::proactive::utils::{
    check_item_monitor_match, process_triggered_item, TriggeredItemResult,
};
use backend::test_utils::{
    create_test_item, create_test_state, create_test_user, set_plan_type, TestItemParams,
    TestUserParams,
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

/// Create a test user with location and timezone set
fn setup_user(state: &Arc<AppState>) -> User {
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

/// Build a minimal Item struct for process_triggered_item
fn make_item(user_id: i32, summary: &str, priority: i32) -> Item {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    Item {
        id: Some(9999),
        user_id,
        summary: summary.to_string(),
        monitor: false,
        next_check_at: Some(now),
        priority,
        source_id: None,
        created_at: now,
    }
}

/// Call process_triggered_item with retries (LLM can fail on malformed output)
async fn process_item_with_retry(
    state: &Arc<AppState>,
    user_id: i32,
    item: &Item,
) -> TriggeredItemResult {
    let mut last_err = None;
    for attempt in 1..=MAX_RETRIES {
        match process_triggered_item(state, user_id, item, None).await {
            Ok(result) => return result,
            Err(e) => {
                eprintln!(
                    "  Attempt {}/{} failed for summary '{}': {}",
                    attempt, MAX_RETRIES, item.summary, e
                );
                last_err = Some(e);
            }
        }
    }
    panic!(
        "process_triggered_item failed after {} retries. Last error: {}",
        MAX_RETRIES,
        last_err.unwrap()
    );
}

/// Call process_triggered_item with retries and an optional matched message
async fn process_item_with_retry_matched(
    state: &Arc<AppState>,
    user_id: i32,
    item: &Item,
    matched_message: Option<&str>,
) -> TriggeredItemResult {
    let mut last_err = None;
    for attempt in 1..=MAX_RETRIES {
        match process_triggered_item(state, user_id, item, matched_message).await {
            Ok(result) => return result,
            Err(e) => {
                eprintln!(
                    "  Attempt {}/{} failed for summary '{}': {}",
                    attempt, MAX_RETRIES, item.summary, e
                );
                last_err = Some(e);
            }
        }
    }
    panic!(
        "process_triggered_item failed after {} retries. Last error: {}",
        MAX_RETRIES,
        last_err.unwrap()
    );
}

/// Assert that the SMS message contains at least one of the expected keywords
fn assert_sms_contains_any(sms_message: &str, keywords: &[&str]) {
    let sms_lower = sms_message.to_lowercase();
    let found = keywords
        .iter()
        .any(|kw| sms_lower.contains(&kw.to_lowercase()));
    assert!(
        found,
        "SMS should contain one of {:?}, got: {}",
        keywords, sms_message
    );
}

// =============================================================================
// 1. Simple reminders - one-shot, LLM returns null next_check_at to signal deletion
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_process_simple_reminder() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms]\nRemind the user to call the dentist.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        result.next_check_at.is_none(),
        "One-shot reminder should not reschedule"
    );
    assert!(
        result.sms_message.is_some(),
        "Should produce an SMS notification"
    );
    let sms = result.sms_message.as_ref().unwrap();
    assert!(!sms.is_empty(), "SMS message should not be empty");
    assert!(
        sms.len() <= 480,
        "SMS exceeds 480 chars (got {}): {}",
        sms.len(),
        sms
    );
    assert_sms_contains_any(sms, &["dentist", "call"]);

    let priority = result.priority.unwrap_or(1);
    assert_eq!(
        priority, 1,
        "Simple reminder should default to SMS priority"
    );

    eprintln!("  SMS: {}", sms);
    eprintln!("  Priority: {}", priority);
    eprintln!("  Summary: {}", result.summary);
}

#[tokio::test]
#[ignore]
async fn test_process_grocery_reminder() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms]\nRemind the user to buy groceries.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(result.next_check_at.is_none());
    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert_sms_contains_any(sms, &["grocer"]);

    eprintln!("  SMS: {}", sms);
}

#[tokio::test]
#[ignore]
async fn test_process_medication_reminder() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms]\nRemind the user to take their medication.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(result.next_check_at.is_none());
    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert_sms_contains_any(sms, &["med", "pill", "take"]);

    eprintln!("  SMS: {}", sms);
}

#[tokio::test]
#[ignore]
async fn test_process_meeting_reminder() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms]\nTeam standup meeting at 2pm on Zoom.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(result.next_check_at.is_none());
    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert_sms_contains_any(sms, &["standup", "meeting", "zoom"]);

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// 2. Call delivery - [notify:call] should produce priority >= 2
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_process_call_priority() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:call]\nScheduled wake-up check-in for the user.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        result.next_check_at.is_none(),
        "One-shot call should not reschedule"
    );
    let priority = result.priority.unwrap_or(1);
    assert!(
        priority >= 2,
        "[notify:call] should produce priority >= 2, got {}",
        priority
    );

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  Priority: {}", priority);
}

#[tokio::test]
#[ignore]
async fn test_process_via_call_alarm() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:call]\nMorning alarm - wake the user up.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    let priority = result.priority.unwrap_or(1);
    assert!(
        priority >= 2,
        "Call alarm should have priority >= 2, got {}",
        priority
    );

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  Priority: {}", priority);
}

// =============================================================================
// 3. Context-rich summaries
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_process_appointment_with_location() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms]\nDentist appointment at 9am, Tampere Dental Clinic, Hameenkatu 12.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert_sms_contains_any(sms, &["dentist", "dental"]);
    assert_sms_contains_any(sms, &["Tampere", "Hameenkatu", "9am", "9:00"]);

    eprintln!("  SMS: {}", sms);
}

#[tokio::test]
#[ignore]
async fn test_process_travel_reminder() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms]\nFlight to London at 2pm, Tampere-Pirkkala airport, check in online first.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert_sms_contains_any(sms, &["flight", "London"]);
    assert_sms_contains_any(sms, &["check in", "airport", "2pm", "2:00"]);

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// 4. Minimal summaries
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_process_minimal_summary() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(user.id, "[type:oneshot] [notify:sms]\nStretch.", 1);
    let result = process_item_with_retry(&state, user.id, &item).await;

    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert_sms_contains_any(sms, &["stretch"]);

    eprintln!("  SMS: {}", sms);
}

#[tokio::test]
#[ignore]
async fn test_process_oven_check_reminder() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms]\nCheck the oven - the user has food cooking.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert_sms_contains_any(sms, &["oven"]);

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// 5. Multilingual summaries
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_process_finnish_summary() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms]\nMuistuta kayttajaa soittamaan hammaslaakarille.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert_sms_contains_any(sms, &["hammasl", "dentist", "soita"]);

    eprintln!("  SMS: {}", sms);
}

#[tokio::test]
#[ignore]
async fn test_process_spanish_summary() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms]\nRecordar al usuario llamar al dentista.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert_sms_contains_any(sms, &["dentist", "llamar"]);

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// 6. Recurring items with [repeat] tag - should reschedule (next_check_at present)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_process_recurring_daily_reminder() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:recurring] [notify:sms] [repeat:daily 09:00]\nRemind the user to take their vitamins.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        result.next_check_at.is_some(),
        "[repeat:daily 09:00] should reschedule (next_check_at present)"
    );
    assert!(
        result.sms_message.is_some(),
        "Should produce an SMS notification"
    );
    let sms = result.sms_message.as_ref().unwrap();
    assert_sms_contains_any(sms, &["vitamin"]);

    // Summary should preserve tags for recurring items
    assert!(
        result.summary.contains("[repeat:daily"),
        "Summary should preserve [repeat] tag for recurring items, got: {}",
        result.summary
    );

    eprintln!("  SMS: {}", sms);
    eprintln!("  next_check_at: {:?}", result.next_check_at);
    eprintln!("  Summary: {}", result.summary);
}

// =============================================================================
// 7. process_triggered_item returns non-empty summary
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_process_returns_updated_summary() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms]\nRemind the user about the team lunch at noon.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        !result.summary.is_empty(),
        "Summary field should be present and non-empty"
    );

    eprintln!("  Summary: {}", result.summary);
    eprintln!("  SMS: {:?}", result.sms_message);
}

// =============================================================================
// 8. Monitor matching - incoming messages that SHOULD match
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_monitor_match_email_from_hr() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:HR] [topic:job offer]\nWatch for emails from HR about the job offer.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: email\nFrom: hr@company.com\nChat: inbox\nContent: \
        Hi, we're happy to inform you that we'd like to extend an offer for the Senior Engineer position.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Should match the HR email monitor");

    assert_eq!(
        response.task_id,
        Some(item_id),
        "Should match the correct item"
    );

    eprintln!("  task_id: {:?}", response.task_id);
}

#[tokio::test]
#[ignore]
async fn test_monitor_match_mom_texts() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:mom] [scope:any]\nWatch for messages from mom.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: whatsapp\nFrom: mom\nChat: Mom\nContent: \
        Hey, are you coming for dinner tonight?";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Should match the mom monitor");

    assert_eq!(response.task_id, Some(item_id));

    eprintln!("  task_id: {:?}", response.task_id);
}

#[tokio::test]
#[ignore]
async fn test_monitor_match_package_delivered() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:Amazon] [topic:headphones shipping]\nWatch for shipping update from Amazon about the new headphones.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: email\nFrom: amazon-notifications\nChat: inbox\nContent: \
        Your package with Sony WH-1000XM5 headphones has been delivered to your front door.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Should match the package monitor");

    assert_eq!(response.task_id, Some(item_id));

    eprintln!("  task_id: {:?}", response.task_id);
}

// =============================================================================
// 9. Monitor matching - incoming messages that should NOT match
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_monitor_no_match_unrelated_email() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(user.id, "[platform:email] [sender:HR] [topic:job offer]\nWatch for emails from HR about the job offer."),
    );

    let message = "Platform: email\nFrom: newsletter@techshop.com\nChat: inbox\nContent: \
        Big sale this weekend! 50% off all electronics.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result.expect("LLM call should succeed");

    assert!(
        response.is_none(),
        "Marketing email should NOT match the HR monitor. Got: {:?}",
        response
    );
}

#[tokio::test]
#[ignore]
async fn test_monitor_no_match_wrong_person() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:mom] [scope:any]\nWatch for messages from mom.",
        ),
    );

    let message = "Platform: whatsapp\nFrom: john\nChat: John\nContent: \
        Hey, want to grab lunch tomorrow?";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result.expect("LLM call should succeed");

    assert!(
        response.is_none(),
        "Message from John should NOT match the mom monitor. Got: {:?}",
        response
    );
}

#[tokio::test]
#[ignore]
async fn test_monitor_no_match_similar_but_different() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:Amazon] [topic:headphones shipping]\nWatch for shipping update from Amazon about the new headphones.",
        ),
    );

    let message = "Platform: email\nFrom: amazon-notifications\nChat: inbox\nContent: \
        Your order of 'Kitchen Paper Towels (12 pack)' has shipped and will arrive Thursday.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result.expect("LLM call should succeed");

    assert!(
        response.is_none(),
        "Paper towels shipping should NOT match headphones monitor. Got: {:?}",
        response
    );
}

// =============================================================================
// 10. Monitor matching - multiple items, correct one should match
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_monitor_match_correct_item_among_multiple() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item_hr = create_test_item(
        &state,
        &TestItemParams::monitor(user.id, "[platform:email] [sender:HR] [topic:job offer]\nWatch for emails from HR about the job offer."),
    );
    let item_mom = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:mom] [scope:any]\nWatch for messages from mom.",
        ),
    );
    let item_package = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:Amazon] [topic:headphones shipping]\nWatch for Amazon shipping update on the headphones.",
        ),
    );

    let mom_item_id = item_mom.id.unwrap();
    let all_items = vec![item_hr, item_mom, item_package];

    let message = "Platform: whatsapp\nFrom: mom\nChat: Mom\nContent: \
        Can you pick up some milk on the way home?";

    let result = check_item_monitor_match(&state, user.id, message, &all_items).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Should match the mom monitor");

    assert_eq!(
        response.task_id,
        Some(mom_item_id),
        "Should match the mom item (id={}), not HR or package. Got task_id: {:?}",
        mom_item_id,
        response.task_id
    );

    eprintln!("  Matched item_id: {:?}", response.task_id);
}

// =============================================================================
// 11. Scheduled check for monitors (via process_triggered_item with None)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_monitor_scheduled_check_with_deadline() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:any] [topic:invoice payment]\nWatch for invoice payment from client due tomorrow. Remind user if still unpaid.",
        )
        .with_next_check_at(now),
    );

    // Scheduled monitor checks now go through process_triggered_item with None
    let result = process_triggered_item(&state, user.id, &item, None).await;
    let response = result.expect("LLM call should succeed");

    // Should produce a notification about the upcoming deadline
    assert!(!response.summary.is_empty(), "Summary should be present");

    eprintln!("  summary: {}", response.summary);
    eprintln!("  sms_message: {:?}", response.sms_message);
    eprintln!("  next_check_at: {:?}", response.next_check_at);
    eprintln!("  priority: {:?}", response.priority);
}

#[tokio::test]
#[ignore]
async fn test_monitor_scheduled_check_no_deadline() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:chat] [sender:mom] [scope:any]\nWatch for messages from mom.",
        )
        .with_next_check_at(now),
    );

    // Scheduled monitor checks now go through process_triggered_item with None
    let result = process_triggered_item(&state, user.id, &item, None).await;
    let response = result.expect("LLM call should succeed");

    assert!(!response.summary.is_empty(), "Summary should be present");

    eprintln!("  summary: {}", response.summary);
    eprintln!("  sms_message: {:?}", response.sms_message);
    eprintln!("  next_check_at: {:?}", response.next_check_at);
    eprintln!("  priority: {:?}", response.priority);
}

// =============================================================================
// A. Same-platform matches (SHOULD match)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_email_sender_and_topic() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:boss] [topic:quarterly report]\nWatch for emails from boss about the quarterly report.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: email\nFrom: boss@company.com\nChat: inbox\nContent: \
        Hi, the Q1 quarterly report is ready for your review. Please check the numbers by Friday.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Should match boss email about quarterly report");
    assert_eq!(response.task_id, Some(item_id));
    eprintln!("  task_id: {:?}", response.task_id);
}

#[tokio::test]
#[ignore]
async fn test_match_whatsapp_scope_any() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:mom] [scope:any]\nNotify whenever mom sends any WhatsApp message.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: whatsapp\nFrom: mom\nChat: Mom\nContent: \
        Did you see the sunset today? It was beautiful!";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Should match any WhatsApp from mom with scope:any");
    assert_eq!(response.task_id, Some(item_id));
    eprintln!("  task_id: {:?}", response.task_id);
}

#[tokio::test]
#[ignore]
async fn test_match_telegram_with_topic() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:telegram] [sender:Alice] [topic:birthday party]\nWatch for messages from Alice about the birthday party.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: telegram\nFrom: alice\nChat: Alice\nContent: \
        Hey! I booked the venue for the birthday party. Saturday at 6pm works for everyone.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Should match Alice telegram about birthday party");
    assert_eq!(response.task_id, Some(item_id));
    eprintln!("  task_id: {:?}", response.task_id);
}

// =============================================================================
// B. Cross-platform matches (SHOULD match)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_cross_platform_boss_report() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:boss] [topic:quarterly report]\nWatch for emails from boss about the quarterly report.",
        ),
    );
    let item_id = item.id.unwrap();

    // Boss mentions the report on WhatsApp instead of email
    let message = "Platform: whatsapp\nFrom: boss\nChat: Boss\nContent: \
        Hey, I just sent you the quarterly report via the shared drive. Take a look when you can.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Cross-platform: boss + quarterly report topic should match");
    assert_eq!(response.task_id, Some(item_id));
    eprintln!("  task_id: {:?}", response.task_id);
}

#[tokio::test]
#[ignore]
async fn test_match_cross_platform_chat_to_email() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:chat] [sender:John] [topic:project deadline]\nWatch for messages from John about the project deadline.",
        ),
    );
    let item_id = item.id.unwrap();

    // John sends email about the project deadline instead of chat
    let message = "Platform: email\nFrom: John\nChat: inbox\nContent: \
        Hi, just wanted to let you know the project deadline has been moved to next Friday.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Cross-platform: John + project deadline should match from email");
    assert_eq!(response.task_id, Some(item_id));
    eprintln!("  task_id: {:?}", response.task_id);
}

// =============================================================================
// C. Same sender, different topic (SHOULD NOT match)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_no_match_amazon_wrong_product() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:Amazon] [topic:headphones]\nWatch for Amazon shipping update on the headphones.",
        ),
    );

    let message = "Platform: email\nFrom: amazon-notifications\nChat: inbox\nContent: \
        Your order of 'Bounty Paper Towels (8 rolls)' has shipped and will arrive Wednesday.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result.expect("LLM call should succeed");
    assert!(
        response.is_none(),
        "Paper towels from Amazon should NOT match headphones monitor. Got: {:?}",
        response
    );
}

#[tokio::test]
#[ignore]
async fn test_no_match_mom_wrong_topic() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:mom] [topic:recipe]\nWatch for mom's message about the recipe she promised.",
        ),
    );

    let message = "Platform: whatsapp\nFrom: mom\nChat: Mom\nContent: \
        The electricity bill this month is really high, did you leave the heater on?";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result.expect("LLM call should succeed");
    assert!(
        response.is_none(),
        "Mom's message about electricity should NOT match recipe monitor. Got: {:?}",
        response
    );
}

// =============================================================================
// D. Similar topic, different sender (SHOULD NOT match)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_no_match_recruiter_not_hr() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:HR] [topic:job offer]\nWatch for emails from the company's HR department about the pending job offer.",
        ),
    );

    // External recruiter from a different company - not the user's HR department
    let message = "Platform: email\nFrom: jane@external-staffing-agency.com\nChat: inbox\nContent: \
        Hi there, I'm a recruiter at TechStaff Inc. We have several engineering roles open. Want to chat?";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result.expect("LLM call should succeed");
    assert!(
        response.is_none(),
        "Recruiter email should NOT match HR job offer monitor. Got: {:?}",
        response
    );
}

#[tokio::test]
#[ignore]
async fn test_no_match_bestbuy_headphones_not_amazon() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:Amazon] [topic:headphones]\nWatch for Amazon shipping update on the headphones.",
        ),
    );

    let message = "Platform: email\nFrom: bestbuy-orders@bestbuy.com\nChat: inbox\nContent: \
        Your Sony WH-1000XM5 headphones order has shipped! Track your package here.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result.expect("LLM call should succeed");
    assert!(
        response.is_none(),
        "Best Buy headphones email should NOT match Amazon headphones monitor. Got: {:?}",
        response
    );
}

// =============================================================================
// E. Group chats
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_sender_in_group_chat() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:John] [topic:project update]\nWatch for messages from John about project updates.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: whatsapp\nFrom: john\nChat: Team Project Group\nContent: \
        Just pushed the latest project update. The backend is ready for testing.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("John's message in group about project should match");
    assert_eq!(response.task_id, Some(item_id));
    eprintln!("  task_id: {:?}", response.task_id);
}

#[tokio::test]
#[ignore]
async fn test_no_match_wrong_sender_in_group() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:John] [topic:project update]\nWatch for messages from John about project updates.",
        ),
    );

    // Sarah messages in the same group, even mentions John
    let message = "Platform: whatsapp\nFrom: sarah\nChat: Team Project Group\nContent: \
        Hey John, when will you have the project update ready? We need it by Friday.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result.expect("LLM call should succeed");
    assert!(
        response.is_none(),
        "Sarah's message mentioning John should NOT match John's monitor. Got: {:?}",
        response
    );
}

// =============================================================================
// F. Non-English messages
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_finnish_message_scope_any() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:mom] [scope:any]\nWatch for any message from mom.",
        ),
    );
    let item_id = item.id.unwrap();

    // Finnish message from mom
    let message = "Platform: whatsapp\nFrom: mom\nChat: Mom\nContent: \
        Hei kulta, muista ottaa takki mukaan, huomenna tulee kylma!";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Finnish message from mom should match scope:any monitor");
    assert_eq!(response.task_id, Some(item_id));
    eprintln!("  task_id: {:?}", response.task_id);
}

#[tokio::test]
#[ignore]
async fn test_match_spanish_message_topic() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:chat] [sender:Carlos] [topic:meeting]\nWatch for messages from Carlos about the meeting.",
        ),
    );
    let item_id = item.id.unwrap();

    // Spanish message about the meeting
    let message = "Platform: whatsapp\nFrom: carlos\nChat: Carlos\nContent: \
        Oye, la reunion se cambio para las 3 de la tarde. No te olvides!";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Spanish meeting message from Carlos should match");
    assert_eq!(response.task_id, Some(item_id));
    eprintln!("  task_id: {:?}", response.task_id);
}

// =============================================================================
// G. Media messages
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_image_from_mom_scope_any() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:mom] [scope:any]\nNotify whenever mom sends any WhatsApp message.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: whatsapp\nFrom: mom\nChat: Mom\nContent: IMAGE";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Image from mom with scope:any should match");
    assert_eq!(response.task_id, Some(item_id));
    eprintln!("  task_id: {:?}", response.task_id);
}

#[tokio::test]
#[ignore]
async fn test_no_match_image_from_mom_with_topic() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:mom] [topic:recipe]\nWatch for mom's recipe message.",
        ),
    );

    let message = "Platform: whatsapp\nFrom: mom\nChat: Mom\nContent: IMAGE";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result.expect("LLM call should succeed");
    assert!(
        response.is_none(),
        "Image from mom should NOT match recipe topic monitor (no content to match). Got: {:?}",
        response
    );
}

// =============================================================================
// H. Multiple items - correct one matches
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_correct_among_three_items() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item_hr = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:HR] [topic:job offer]\nWatch for HR job offer email.",
        ),
    );
    let item_mom = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:mom] [scope:any]\nWatch for any WhatsApp from mom.",
        ),
    );
    let item_hackathon = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:any] [sender:any] [topic:hackathon]\nWatch for any messages about the hackathon.",
        ),
    );

    let mom_id = item_mom.id.unwrap();
    let all_items = vec![item_hr, item_mom, item_hackathon];

    let message = "Platform: whatsapp\nFrom: mom\nChat: Mom\nContent: \
        Are you free this weekend? Let's have lunch together.";

    let result = check_item_monitor_match(&state, user.id, message, &all_items).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Mom's WhatsApp should match mom monitor");
    assert_eq!(
        response.task_id,
        Some(mom_id),
        "Should match mom item, not HR or hackathon"
    );
    eprintln!("  task_id: {:?}", response.task_id);
}

#[tokio::test]
#[ignore]
async fn test_match_correct_amazon_item_among_two() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item_headphones = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:Amazon] [topic:headphones]\nWatch for Amazon headphones shipping update.",
        ),
    );
    let item_refund = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:Amazon] [topic:refund]\nWatch for Amazon refund confirmation.",
        ),
    );

    let refund_id = item_refund.id.unwrap();
    let all_items = vec![item_headphones, item_refund];

    let message = "Platform: email\nFrom: amazon-notifications\nChat: inbox\nContent: \
        Your refund of $49.99 for 'Wireless Mouse' has been processed and will appear in 3-5 business days.";

    let result = check_item_monitor_match(&state, user.id, message, &all_items).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Amazon refund email should match refund monitor");
    assert_eq!(
        response.task_id,
        Some(refund_id),
        "Should match refund item, not headphones"
    );
    eprintln!("  task_id: {:?}", response.task_id);
}

// =============================================================================
// I. Short messages
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_short_message_scope_any() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:boss] [scope:any]\nWatch for any message from boss.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: whatsapp\nFrom: boss\nChat: Boss\nContent: ok";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Short 'ok' from boss with scope:any should match");
    assert_eq!(response.task_id, Some(item_id));
    eprintln!("  task_id: {:?}", response.task_id);
}

#[tokio::test]
#[ignore]
async fn test_no_match_short_message_with_topic() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:John] [topic:project deadline]\nWatch for John's messages about the project deadline.",
        ),
    );

    let message = "Platform: whatsapp\nFrom: john\nChat: John\nContent: thanks";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result.expect("LLM call should succeed");
    assert!(
        response.is_none(),
        "'thanks' from John should NOT match project deadline monitor. Got: {:?}",
        response
    );
}

// =============================================================================
// J. Forwarded messages
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_no_match_forwarded_email() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:boss] [topic:quarterly report]\nWatch for emails from boss about the quarterly report.",
        ),
    );

    // Email forwarded BY a colleague, originally FROM boss
    let message = "Platform: email\nFrom: colleague@company.com\nChat: inbox\nSubject: Fwd: Quarterly Report\nContent: \
        FYI, forwarding this from boss. The quarterly report numbers look good.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result.expect("LLM call should succeed");
    assert!(
        response.is_none(),
        "Forwarded email from colleague (not boss) should NOT match. Got: {:?}",
        response
    );
}

// =============================================================================
// K. Platform wildcards
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_platform_any_email() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:any] [sender:any] [topic:hackathon]\nWatch for any messages about the hackathon.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: email\nFrom: organizers@hackathon.dev\nChat: inbox\nSubject: Hackathon Registration Confirmed\nContent: \
        Your registration for the Spring Hackathon 2026 has been confirmed. See you there!";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("platform:any + hackathon topic should match email");
    assert_eq!(response.task_id, Some(item_id));
    eprintln!("  task_id: {:?}", response.task_id);
}

#[tokio::test]
#[ignore]
async fn test_match_platform_chat_matches_signal() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:chat] [sender:any] [topic:hackathon]\nWatch for chat messages about the hackathon.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: signal\nFrom: dave\nChat: Dave\nContent: \
        Are you joining the hackathon this weekend? We need a fourth team member!";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("platform:chat should match Signal message about hackathon");
    assert_eq!(response.task_id, Some(item_id));
    eprintln!("  task_id: {:?}", response.task_id);
}

// =============================================================================
// L. Legacy format (backward compatibility)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_legacy_format_mom_email() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    // Legacy format: no structured tags, just natural language
    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "Watch for emails from mom. Notify the user when one arrives.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: email\nFrom: mom@gmail.com\nChat: inbox\nContent: \
        Hi sweetie, just wanted to check in. How is work going?";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Legacy format should still match via semantic reasoning");
    assert_eq!(response.task_id, Some(item_id));
    eprintln!("  task_id: {:?}", response.task_id);
}

#[tokio::test]
#[ignore]
async fn test_no_match_legacy_format_wrong_sender() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    // Legacy format
    let item = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "Watch for messages from John about the project deadline.",
        ),
    );

    let message = "Platform: whatsapp\nFrom: sarah\nChat: Sarah\nContent: \
        The project deadline is next Friday, make sure to submit everything on time.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result.expect("LLM call should succeed");
    assert!(
        response.is_none(),
        "Legacy format: Sarah's message should NOT match John's monitor. Got: {:?}",
        response
    );
}

// =============================================================================
// M. No match at all
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_no_match_tech_news_bot() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item_mom = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:whatsapp] [sender:mom] [scope:any]\nWatch for any WhatsApp from mom.",
        ),
    );
    let item_hr = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:email] [sender:HR] [topic:job offer]\nWatch for HR job offer email.",
        ),
    );
    let item_hackathon = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "[platform:any] [sender:any] [topic:hackathon]\nWatch for messages about the hackathon.",
        ),
    );

    let all_items = vec![item_mom, item_hr, item_hackathon];

    let message = "Platform: telegram\nFrom: tech-news-bot\nChat: Tech News\nContent: \
        Linux kernel 6.12 released with improved Rust support and better power management for laptops.";

    let result = check_item_monitor_match(&state, user.id, message, &all_items).await;
    let response = result.expect("LLM call should succeed");
    assert!(
        response.is_none(),
        "Tech news bot message should match nothing. Got: {:?}",
        response
    );
}

// =============================================================================
// N. Monitor match via process_triggered_item - SMS notification
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_trigger_monitor_match_sms() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [platform:whatsapp] [sender:mom] [scope:any] [notify:sms]\nWatch for messages from mom.",
        1,
    );
    let matched = "Platform: whatsapp\nFrom: mom\nChat: Mom\nContent: Hey, are you coming for dinner tonight?";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        result.sms_message.is_some(),
        "Monitor match should produce SMS notification"
    );
    let sms = result.sms_message.as_ref().unwrap();
    assert_sms_contains_any(sms, &["mom", "dinner"]);
    assert!(
        result.next_check_at.is_none(),
        "One-shot monitor should not reschedule (no [repeat] tag)"
    );
    let priority = result.priority.unwrap_or(1);
    assert_eq!(
        priority, 1,
        "Monitor with [notify:sms] should have priority 1"
    );

    eprintln!("  SMS: {}", sms);
    eprintln!("  Priority: {}", priority);
}

// =============================================================================
// O. Monitor match via process_triggered_item - call notification
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_trigger_monitor_match_call() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [platform:email] [sender:HR] [topic:job offer] [notify:call]\nWatch for emails from HR about the job offer.",
        1,
    );
    let matched = "Platform: email\nFrom: hr@company.com\nChat: inbox\nContent: \
        Hi, we're happy to extend an offer for the Senior Engineer position. Please review the attached.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        result.sms_message.is_some(),
        "Monitor match should produce notification text"
    );
    let priority = result.priority.unwrap_or(1);
    assert!(
        priority >= 2,
        "Monitor with [notify:call] should have priority >= 2, got {}",
        priority
    );
    assert!(
        result.next_check_at.is_none(),
        "One-shot monitor should not reschedule"
    );

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  Priority: {}", priority);
}

// =============================================================================
// P. Monitor match - content quality (mentions relevant details)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_trigger_monitor_match_content_quality() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [platform:email] [sender:Amazon] [topic:headphones] [notify:sms]\nWatch for Amazon shipping update on the headphones.",
        1,
    );
    let matched = "Platform: email\nFrom: amazon-notifications\nChat: inbox\nContent: \
        Your package with Sony WH-1000XM5 headphones has been delivered to your front door.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert_sms_contains_any(sms, &["headphones", "delivered", "package"]);

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// Q. Weather-conditional fetch (infra-dependent - needs weather API)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_trigger_weather_fetch() {
    // NOTE: This test depends on the weather API being available.
    // It verifies that [fetch:weather] causes the LLM to call get_weather
    // and produce an SMS mentioning weather conditions.
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms] [fetch:weather]\nCheck weather in Helsinki. If below freezing, remind the user to warm up the car. If not, no notification needed.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    // The LLM should have called get_weather and made a decision.
    // We can't assert on the weather condition itself, but the LLM should have processed it.
    assert!(
        result.next_check_at.is_none(),
        "One-shot weather check should not reschedule"
    );
    // sms_message may or may not be present depending on actual weather
    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  Priority: {:?}", result.priority);
    eprintln!("  Summary: {}", result.summary);
}

// =============================================================================
// R. Silent notification - [notify:silent] should produce priority 0
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_trigger_silent_notification() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:silent]\nLog that the user's daily backup completed.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    let priority = result.priority.unwrap_or(1);
    assert_eq!(
        priority, 0,
        "[notify:silent] should produce priority 0, got {}",
        priority
    );
    // SMS may be absent or present but priority 0 means no notification sent
    assert!(
        result.next_check_at.is_none(),
        "One-shot silent item should not reschedule"
    );

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  Priority: {}", priority);
}

// =============================================================================
// S. Legacy backward compatibility - [VIA CALL] without tags
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_trigger_legacy_via_call() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(user.id, "Check-in call for the user [VIA CALL]", 1);
    let result = process_item_with_retry(&state, user.id, &item).await;

    let priority = result.priority.unwrap_or(1);
    assert!(
        priority >= 2,
        "Legacy [VIA CALL] should produce priority >= 2, got {}",
        priority
    );

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  Priority: {}", priority);
}

// =============================================================================
// T. Legacy no-tag reminder
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_trigger_legacy_no_tags() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(user.id, "Remind the user to stretch", 1);
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        result.sms_message.is_some(),
        "Legacy no-tag reminder should produce SMS"
    );
    let sms = result.sms_message.as_ref().unwrap();
    assert_sms_contains_any(sms, &["stretch"]);
    assert!(
        result.next_check_at.is_none(),
        "One-shot legacy reminder should not reschedule"
    );

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// U. SMS length check - context-rich summary stays under 480 chars
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_trigger_sms_length_limit() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms]\nThe user has a complex day ahead: 9am dentist at Tampere Dental Clinic on \
        Hameenkatu 12, then 11am meeting with the marketing team at the office 3rd floor conference \
        room B, lunch at 12:30 with Maria at Restaurant Plevna, 2pm call with the London office \
        about the Q1 report, and 4pm pick up kids from school at Tampere International School.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert!(
        sms.len() <= 480,
        "SMS exceeds 480 chars (got {}): {}",
        sms.len(),
        sms
    );

    eprintln!("  SMS ({} chars): {}", sms.len(), sms);
}

// =============================================================================
// V. Summary preservation for recurring items
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_trigger_summary_preservation_recurring() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let original_summary =
        "[type:recurring] [notify:sms] [repeat:daily 09:00]\nRemind the user to check their email.";
    let item = make_item(user.id, original_summary, 1);
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        result.next_check_at.is_some(),
        "Recurring item should reschedule"
    );
    assert!(
        result.summary.contains("[repeat:daily 09:00]"),
        "Summary must preserve [repeat:daily 09:00] tag, got: {}",
        result.summary
    );
    assert!(
        result.summary.contains("[notify:sms]"),
        "Summary must preserve [notify:sms] tag, got: {}",
        result.summary
    );

    eprintln!("  Summary: {}", result.summary);
    eprintln!("  next_check_at: {:?}", result.next_check_at);
}

// =============================================================================
// W. Tracking items - AI can update summary and decide next_check_at
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_tracking_item_updates_summary() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:tracking] [notify:sms] [platform:email] [sender:Amazon] [topic:shipping delivery]\nTrack Amazon package delivery. Notify user when shipped or delivered.",
        1,
    );
    let matched = "Platform: email\nFrom: amazon-notifications\nChat: inbox\nContent: \
        Your package with Sony WH-1000XM5 headphones has been delivered to your front door.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    // Tracking items should update summary with new info
    assert!(
        !result.summary.is_empty(),
        "Tracking item should return a summary"
    );
    assert!(
        result.summary.contains("[type:tracking]"),
        "Summary should preserve [type:tracking] tag, got: {}",
        result.summary
    );
    // Package delivered = immediate attention, should notify
    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  Summary: {}", result.summary);
    eprintln!("  next_check_at: {:?}", result.next_check_at);
    eprintln!("  priority: {:?}", result.priority);
    assert!(
        result.sms_message.is_some(),
        "Should notify user about delivery. Summary: {}",
        result.summary
    );
    let sms = result.sms_message.as_ref().unwrap();
    assert_sms_contains_any(sms, &["delivered", "headphones", "package"]);

    eprintln!("  SMS: {}", sms);
    eprintln!("  Summary: {}", result.summary);
    eprintln!("  next_check_at: {:?}", result.next_check_at);
}

#[tokio::test]
#[ignore]
async fn test_tracking_item_can_reschedule() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:tracking] [notify:silent]\nTrack Bitcoin price. Notify user when it hits $100,000.",
        0,
    );
    // Time-triggered check (no matched message) - AI should reschedule for next check
    let result = process_item_with_retry(&state, user.id, &item).await;

    // Should reschedule (Bitcoin probably hasn't hit 100k during this test)
    assert!(
        result.next_check_at.is_some(),
        "Tracking item should reschedule for next check"
    );
    assert!(!result.summary.is_empty(), "Should return a summary");

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  Summary: {}", result.summary);
    eprintln!("  next_check_at: {:?}", result.next_check_at);
    eprintln!("  priority: {:?}", result.priority);
}
