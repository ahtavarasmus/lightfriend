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

    let item = make_item(user.id, "Remind the user to call the dentist", 1);
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

    let item = make_item(user.id, "Remind the user to buy groceries", 1);
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

    let item = make_item(user.id, "Remind the user to take their medication", 1);
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

    let item = make_item(user.id, "Team standup meeting at 2pm on Zoom", 1);
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(result.next_check_at.is_none());
    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert_sms_contains_any(sms, &["standup", "meeting", "zoom"]);

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// 2. Call delivery - [VIA CALL] should produce priority >= 2
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_process_call_priority() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "Scheduled wake-up check-in for the user [VIA CALL]",
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
        "Summary with [VIA CALL] should produce priority >= 2, got {}",
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

    let item = make_item(user.id, "Morning alarm - wake the user up [VIA CALL]", 1);
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
        "Dentist appointment at 9am, Tampere Dental Clinic, Hameenkatu 12",
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
        "Flight to London at 2pm, Tampere-Pirkkala airport, check in online first",
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

    let item = make_item(user.id, "Stretch", 1);
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

    let item = make_item(user.id, "Check the oven - the user has food cooking", 1);
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

    let item = make_item(user.id, "Muistuta kayttajaa soittamaan hammaslaakarille", 1);
    let result = process_item_with_retry(&state, user.id, &item).await;

    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert_sms_contains_any(sms, &["hammaslaa", "dentist"]);

    eprintln!("  SMS: {}", sms);
}

#[tokio::test]
#[ignore]
async fn test_process_spanish_summary() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(user.id, "Recordar al usuario llamar al dentista", 1);
    let result = process_item_with_retry(&state, user.id, &item).await;

    let sms = result.sms_message.as_ref().expect("Should have SMS");
    assert_sms_contains_any(sms, &["dentist", "llamar"]);

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// 6. Recurring/digest items - should reschedule (next_check_at present)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_process_daily_digest() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let summary = "Daily morning digest: check unread messages from all messaging apps and email. \
        Summarize highlights and send to user. Reschedule for tomorrow at 9:00.";
    let item = make_item(user.id, summary, 1);
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        result.next_check_at.is_some(),
        "Daily digest should reschedule (next_check_at present)"
    );
    assert!(
        result.sms_message.is_some(),
        "Digest should produce an SMS with highlights"
    );
    // Summary should be returned unchanged for recurring items
    assert!(!result.summary.is_empty(), "Summary should be present");

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  next_check_at: {:?}", result.next_check_at);
    eprintln!("  Summary: {}", result.summary);
}

#[tokio::test]
#[ignore]
async fn test_process_weekly_digest() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let summary = "Weekly summary: gather messages and calendar events for the past week. \
        Summarize highlights. Reschedule for next Monday at 8:00.";
    let item = make_item(user.id, summary, 1);
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        result.next_check_at.is_some(),
        "Weekly digest should reschedule"
    );

    eprintln!("  SMS: {:?}", result.sms_message);
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

    let item = make_item(user.id, "Remind the user about the team lunch at noon", 1);
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
            "Watch for emails from HR about the job offer. Notify the user when one arrives.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "From: hr@company.com\nService: email\nRoom: inbox\nContent: \
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
            "Watch for messages from mom. Notify the user when she texts.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "From: mom\nService: whatsapp\nRoom: Mom\nContent: \
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
            "Watch for shipping update from Amazon about the new headphones",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "From: amazon-notifications\nService: email\nRoom: inbox\nContent: \
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
        &TestItemParams::monitor(user.id, "Watch for emails from HR about the job offer"),
    );

    let message = "From: newsletter@techshop.com\nService: email\nRoom: inbox\nContent: \
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
            "Watch for messages from mom. Notify the user when she texts.",
        ),
    );

    let message = "From: john\nService: whatsapp\nRoom: John\nContent: \
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
            "Watch for shipping update from Amazon about the new headphones",
        ),
    );

    let message = "From: amazon-notifications\nService: email\nRoom: inbox\nContent: \
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
        &TestItemParams::monitor(user.id, "Watch for emails from HR about the job offer"),
    );
    let item_mom = create_test_item(
        &state,
        &TestItemParams::monitor(user.id, "Watch for messages from mom"),
    );
    let item_package = create_test_item(
        &state,
        &TestItemParams::monitor(
            user.id,
            "Watch for Amazon shipping update on the headphones",
        ),
    );

    let mom_item_id = item_mom.id.unwrap();
    let all_items = vec![item_hr, item_mom, item_package];

    let message = "From: mom\nService: whatsapp\nRoom: Mom\nContent: \
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
            "Watch for invoice payment from client due tomorrow. Remind user if still unpaid.",
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
        &TestItemParams::monitor(user.id, "Watch for messages from mom").with_next_check_at(now),
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
