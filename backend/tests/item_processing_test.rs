//! LLM Integration Tests for Item Processing
//!
//! Organized around the simplified 2-prompt matrix:
//!   - NOTIFICATION_PROMPT: oneshot, recurring, legacy (N1-N6)
//!   - TRACKING_PROMPT: all tracking items (T1-T7)
//!   - Rust-level deterministic tests (R1-R7)
//!   - Monitor matching tests (check_item_monitor_match)
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
        due_at: Some(now),
        priority,
        source_id: None,
        created_at: now,
    }
}

/// Build a tracking Item with due_at set in the future (5 days).
/// Tracking items auto-delete at deadline, so tests must use a future due_at.
fn make_tracking_item(user_id: i32, summary: &str, priority: i32) -> Item {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    Item {
        id: Some(9999),
        user_id,
        summary: summary.to_string(),
        due_at: Some(now + 5 * 86400), // 5 days in the future
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
// N1: Simple oneshot reminder - non-empty SMS, deleted (due_at=None)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_n1_simple_oneshot_dentist() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms]\nRemind the user to call the dentist.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(result.due_at.is_none(), "Oneshot should be deleted");
    let sms = result.sms_message.as_ref().expect("N1: should produce SMS");
    assert!(!sms.is_empty(), "SMS should not be empty");
    assert!(
        sms.len() <= 480,
        "SMS exceeds 480 chars (got {}): {}",
        sms.len(),
        sms
    );
    assert_sms_contains_any(sms, &["dentist", "call"]);
    assert_eq!(result.priority.unwrap_or(1), 1, "SMS priority should be 1");

    eprintln!("  SMS: {}", sms);
}

#[tokio::test]
#[ignore]
async fn test_n1_simple_oneshot_meeting() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms]\nTeam standup meeting at 2pm on Zoom.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(result.due_at.is_none(), "Oneshot should be deleted");
    let sms = result.sms_message.as_ref().expect("N1: should produce SMS");
    assert_sms_contains_any(sms, &["standup", "meeting", "zoom"]);

    eprintln!("  SMS: {}", sms);
}

#[tokio::test]
#[ignore]
async fn test_n1_simple_oneshot_call_priority() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:call]\nScheduled wake-up check-in for the user.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(result.due_at.is_none(), "Oneshot should be deleted");
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
async fn test_n1_simple_oneshot_context_rich() {
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
async fn test_n1_simple_oneshot_sms_length() {
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

#[tokio::test]
#[ignore]
async fn test_n1_simple_oneshot_multilingual() {
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

// =============================================================================
// N2: Conditional oneshot - condition met (weather fetch, SMS non-empty)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_n2_conditional_weather_fetch() {
    // NOTE: Depends on weather API availability.
    // Verifies [fetch:weather] causes pre-fetch and LLM processes it.
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:oneshot] [notify:sms] [fetch:weather]\nCheck weather in Helsinki. If below freezing, remind the user to warm up the car. If not, no notification needed.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        result.due_at.is_none(),
        "Oneshot should be deleted regardless of condition"
    );
    // sms_message may or may not be present depending on actual weather
    eprintln!("  SMS: {:?}", result.sms_message);
}

// =============================================================================
// N3: Conditional oneshot - condition NOT met (empty SMS, still deleted)
// Cannot reliably test this without controlled weather data.
// The N2 test covers the structural behavior.
// =============================================================================

// =============================================================================
// N4: Recurring digest - pre-fetched data, non-empty SMS, rescheduled
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_n4_recurring_digest_with_fetch() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:recurring] [notify:sms] [repeat:daily 09:00] [fetch:email]\nMorning briefing: summarize today's important emails.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        result.due_at.is_some(),
        "Recurring should reschedule (due_at present)"
    );
    // Summary should be frozen by Rust override
    assert!(
        result.summary.contains("[repeat:daily 09:00]"),
        "Summary must preserve tags, got: {}",
        result.summary
    );

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  due_at: {:?}", result.due_at);
}

// =============================================================================
// N5: Recurring digest - nothing noteworthy (empty SMS, rescheduled)
// Cannot reliably force empty SMS without controlled data.
// Structural behavior covered by N4.
// =============================================================================

// =============================================================================
// N6: Recurring simple (no fetch) - non-empty SMS, rescheduled
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_n6_recurring_simple() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(
        user.id,
        "[type:recurring] [notify:sms] [repeat:daily 09:00]\nRemind the user to take their vitamins.",
        1,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(result.due_at.is_some(), "Recurring should reschedule");
    let sms = result.sms_message.as_ref().expect("N6: should produce SMS");
    assert_sms_contains_any(sms, &["vitamin"]);
    // Summary frozen by Rust override
    assert!(
        result.summary.contains("[repeat:daily 09:00]"),
        "Summary must preserve [repeat] tag, got: {}",
        result.summary
    );
    assert!(
        result.summary.contains("[notify:sms]"),
        "Summary must preserve [notify:sms] tag, got: {}",
        result.summary
    );

    eprintln!("  SMS: {}", sms);
    eprintln!("  due_at: {:?}", result.due_at);
}

// =============================================================================
// N-legacy: Legacy items (no [type:X] tag) routed to NOTIFICATION_PROMPT
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_n_legacy_no_tags() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_item(user.id, "Remind the user to stretch", 1);
    let result = process_item_with_retry(&state, user.id, &item).await;

    let sms = result
        .sms_message
        .as_ref()
        .expect("Legacy should produce SMS");
    assert_sms_contains_any(sms, &["stretch"]);
    // Legacy without tags: next_check_locked=false, item_type != "oneshot", so due_at not overridden
    // But LLM only has sms_message field, so due_at stays None from default

    eprintln!("  SMS: {}", sms);
    eprintln!("  due_at: {:?}", result.due_at);
}

#[tokio::test]
#[ignore]
async fn test_n_legacy_via_call() {
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

#[tokio::test]
#[ignore]
async fn test_n_silent_notification() {
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
    assert!(result.due_at.is_none(), "Oneshot should be deleted");

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  Priority: {}", priority);
}

// =============================================================================
// T1: Tracking match - clear resolution (delivered) -> deleted
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t1_match_resolves_delivery() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:Amazon] [topic:shipping delivery]\nTrack Amazon package delivery. Notify user when shipped or delivered.",
        1,
    );
    let matched = "Platform: email\nFrom: amazon-notifications\nChat: inbox\nContent: \
        Your package with Sony WH-1000XM5 headphones has been delivered to your front door.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        result.tracking_complete.unwrap_or(false),
        "Delivery should resolve tracking. Summary: {}",
        result.summary
    );
    assert!(result.due_at.is_none(), "Resolved tracking -> deleted");
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(!sms.is_empty(), "Should notify about delivery");
    assert_sms_contains_any(sms, &["delivered", "headphones", "package"]);

    eprintln!("  SMS: {}", sms);
    eprintln!("  tracking_complete: {:?}", result.tracking_complete);
}

#[tokio::test]
#[ignore]
async fn test_t1_match_resolves_invoice_paid() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:client] [topic:invoice payment]\nInvoice #4523 for $2,400 sent to Acme Corp. Watch for payment confirmation.",
        0,
    );
    let matched = "Platform: email\nFrom: payments@acmecorp.com\nChat: inbox\nContent: \
        Payment received: Invoice #4523, amount $2,400.00 has been processed. Thank you for your services.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        result.tracking_complete.unwrap_or(false),
        "Payment confirmation should resolve. Summary: {}",
        result.summary
    );
    assert!(result.due_at.is_none(), "Resolved -> deleted");
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(!sms.is_empty(), "Should notify about payment");
    assert_sms_contains_any(sms, &["paid", "payment", "received", "$2,400"]);

    eprintln!("  SMS: {}", sms);
}

#[tokio::test]
#[ignore]
async fn test_t1_match_resolves_reply_received() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:chat] [platform:whatsapp] [sender:landlord] [topic:lease renewal]\nWaiting for landlord's response about lease renewal terms. Notify when they reply.",
        0,
    );
    let matched = "Platform: whatsapp\nFrom: landlord\nChat: Landlord\nContent: \
        Hi, I've decided to keep the rent the same for another year. I'll send the new lease agreement tomorrow.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        result.tracking_complete.unwrap_or(false),
        "Landlord's decision should resolve. Summary: {}",
        result.summary
    );
    assert!(result.due_at.is_none(), "Resolved -> deleted");
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(!sms.is_empty(), "Should notify about lease decision");
    assert_sms_contains_any(sms, &["landlord", "lease", "rent"]);

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// T2: Tracking match - noteworthy intermediate (shipped, not delivered)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t2_match_shipped_not_delivered() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:Amazon] [topic:laptop delivery]\nWatch for Amazon delivery of MacBook Pro. Notify when delivered.",
        0,
    );
    let matched = "Platform: email\nFrom: amazon-shipping@amazon.com\nChat: inbox\nContent: \
        Your order has shipped! MacBook Pro 16-inch. Estimated delivery: March 5. Tracking: 1Z999AA10123456784.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        !result.tracking_complete.unwrap_or(true),
        "Shipped != delivered, should keep tracking. Summary: {}",
        result.summary
    );
    assert!(
        result.due_at.is_some(),
        "Should reschedule (still tracking)"
    );
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(!sms.is_empty(), "Shipping update is noteworthy");
    assert_sms_contains_any(sms, &["shipped", "MacBook", "March"]);

    eprintln!("  SMS: {}", sms);
}

#[tokio::test]
#[ignore]
async fn test_t2_match_partial_payment() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:client] [topic:project payment]\nWatch for full payment of $5,000 from ClientCo for the design project.",
        0,
    );
    let matched = "Platform: email\nFrom: accounting@clientco.com\nChat: inbox\nContent: \
        Partial payment of $2,500 has been processed for the design project. Remaining balance: $2,500.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        !result.tracking_complete.unwrap_or(true),
        "Partial payment should NOT resolve. Summary: {}",
        result.summary
    );
    assert!(
        result.due_at.is_some(),
        "Should keep tracking for full payment"
    );
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(!sms.is_empty(), "Partial payment is noteworthy");

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// T3: Tracking match - routine noise (off-topic from tracked sender)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t3_match_off_topic_from_sender() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:chat] [platform:whatsapp] [sender:boss] [topic:promotion decision]\nWaiting for boss to share the promotion decision.",
        0,
    );
    let matched = "Platform: whatsapp\nFrom: boss\nChat: Boss\nContent: \
        Don't forget to submit your timesheet by end of day.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        !result.tracking_complete.unwrap_or(true),
        "Timesheet reminder is unrelated to promotion. Summary: {}",
        result.summary
    );
    assert!(result.due_at.is_some(), "Should keep tracking");
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(
        sms.is_empty(),
        "Off-topic message should not trigger notification. Got SMS: {}",
        sms
    );

    eprintln!("  SMS (should be empty): {:?}", result.sms_message);
}

// =============================================================================
// T4: Tracking check - fetch finds resolved (scheduled check)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t4_check_weather_condition() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:weather]\nCheck if it will rain in Tampere today. Notify user to bring umbrella if rain expected.",
        0,
    );
    // No matched message = scheduled check. LLM will call weather fetch tool.
    let result = process_item_with_retry(&state, user.id, &item).await;

    // Weather either shows rain or not - verify structural correctness
    assert!(!result.summary.is_empty(), "Should return updated summary");
    assert!(
        result.summary.contains("[type:tracking]"),
        "Should preserve tags. Summary: {}",
        result.summary
    );
    let lines: Vec<&str> = result.summary.lines().collect();
    assert!(
        lines.len() >= 2,
        "Summary should have findings appended below tags. Summary: {}",
        result.summary
    );

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  tracking_complete: {:?}", result.tracking_complete);
    eprintln!("  Summary: {}", result.summary);
}

// =============================================================================
// T5: Tracking check - fetch finds nothing changed
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t5_check_email_nothing_found() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:university] [topic:admission]\nWaiting for university admission decision email.\nNo updates as of Feb 27.",
        0,
    );
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        !result.tracking_complete.unwrap_or(true),
        "No admission email found, should continue. Summary: {}",
        result.summary
    );
    assert!(result.due_at.is_some(), "Should reschedule for next check");

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  Summary: {}", result.summary);
}

// =============================================================================
// T6: Tracking match - ambiguous (return processed, refund pending)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t6_match_ambiguous_return_processed() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:Amazon] [topic:refund]\nWatch for Amazon refund of $299.99 for returned headphones. Notify when refund is credited to account.",
        0,
    );
    let matched = "Platform: email\nFrom: returns@amazon.com\nChat: inbox\nContent: \
        Your return has been received and processed. Refund of $299.99 is being reviewed and will be credited within 5-7 business days.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    // Refund is being processed but not yet credited - should keep tracking
    assert!(
        !result.tracking_complete.unwrap_or(true),
        "Refund pending (not credited) should not resolve. Summary: {}",
        result.summary
    );
    assert!(
        result.due_at.is_some(),
        "Should keep tracking until credited"
    );
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(!sms.is_empty(), "Return processed is noteworthy");

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// T7: Tracking match - auto-reply noise
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t7_match_routine_shipping_scan() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:FedEx] [topic:guitar delivery]\nWatch for delivery of guitar from FedEx. Notify when delivered.\nShipped Feb 25, tracking 789456123.",
        0,
    );
    let matched = "Platform: email\nFrom: tracking@fedex.com\nChat: inbox\nContent: \
        Package scan update: Your package has departed the FedEx facility in Memphis, TN. In transit to destination.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        !result.tracking_complete.unwrap_or(true),
        "In-transit scan is not delivery. Summary: {}",
        result.summary
    );
    assert!(result.due_at.is_some(), "Should keep tracking");
    // Routine facility-scan is not "delivered" - notification not expected

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  Summary: {}", result.summary);
}

// =============================================================================
// T8: Tags preserved after match processing
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t8_tags_preserved_after_processing() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:Amazon] [topic:delivery]\nWatch for Amazon package delivery.",
        1,
    );
    let matched = "Platform: email\nFrom: amazon-notifications\nChat: inbox\nContent: \
        Your package is out for delivery and will arrive today between 2-6pm.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    let first_line = result.summary.lines().next().unwrap_or("");
    assert!(
        first_line.contains("[type:tracking]"),
        "T8: Must preserve [type:tracking]. First line: {}",
        first_line
    );
    assert!(
        first_line.contains("[notify:sms]"),
        "T8: Must preserve [notify:sms]. First line: {}",
        first_line
    );
    assert!(
        first_line.contains("[platform:email]"),
        "T8: Must preserve [platform:email]. First line: {}",
        first_line
    );
    assert!(
        first_line.contains("[sender:Amazon]"),
        "T8: Must preserve [sender:Amazon]. First line: {}",
        first_line
    );
    assert!(
        first_line.contains("[topic:delivery]"),
        "T8: Must preserve [topic:delivery]. First line: {}",
        first_line
    );

    eprintln!("  Summary: {}", result.summary);
}

// =============================================================================
// T9: Findings appended on new lines below tags
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t9_findings_appended_below_tags() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:FedEx] [topic:guitar delivery]\nWatch for delivery of guitar from FedEx.",
        0,
    );
    let matched = "Platform: email\nFrom: tracking@fedex.com\nChat: inbox\nContent: \
        Your package has shipped from the warehouse in Nashville, TN. Estimated delivery: March 3.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    let lines: Vec<&str> = result.summary.lines().collect();
    assert!(
        lines.len() >= 2,
        "T9: Summary should have tags on first line + findings below. Got {} lines: {}",
        lines.len(),
        result.summary
    );
    // First line is tags, subsequent lines are findings/description
    assert!(
        lines[0].contains("[type:tracking]"),
        "T9: First line should contain tags. Got: {}",
        lines[0]
    );

    eprintln!("  Summary ({} lines): {}", lines.len(), result.summary);
}

// =============================================================================
// T10: "Out for delivery" is NOT delivered
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t10_out_for_delivery_not_complete() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [platform:email] [sender:Amazon] [topic:package delivery]\nWatch for Amazon package delivery. Notify user when the package is delivered.",
        1,
    );
    let matched = "Platform: email\nFrom: amazon-shipping@amazon.com\nChat: inbox\nContent: \
        Great news! Your package is out for delivery. It will arrive today.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        !result.tracking_complete.unwrap_or(true),
        "T10: 'Out for delivery' is NOT delivered. tracking_complete should be false. Summary: {}",
        result.summary
    );
    // Out for delivery is noteworthy - should produce an update
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(
        !sms.is_empty(),
        "T10: 'Out for delivery' is noteworthy update, should produce SMS"
    );

    eprintln!("  SMS: {}", sms);
    eprintln!("  tracking_complete: {:?}", result.tracking_complete);
}

// =============================================================================
// T11: Refund credited (vs T6 pending) - should resolve
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t11_refund_credited_resolves() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:Amazon] [topic:refund]\nWatch for Amazon refund of $299.99. Notify when refund is credited to account.",
        0,
    );
    let matched = "Platform: email\nFrom: returns@amazon.com\nChat: inbox\nContent: \
        Your refund of $299.99 has been credited to your Visa ending in 4532. \
        The credit should appear on your statement within 2-3 business days.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        result.tracking_complete.unwrap_or(false),
        "T11: Refund credited should resolve tracking. Summary: {}",
        result.summary
    );
    assert!(result.due_at.is_none(), "Resolved tracking -> deleted");

    eprintln!("  SMS: {:?}", result.sms_message);
}

// =============================================================================
// T12: Interview invite is NOT the job offer
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t12_interview_invite_not_job_offer() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:HR] [topic:job offer]\nWaiting for job offer from HR department. Notify when they extend an official offer.",
        0,
    );
    let matched = "Platform: email\nFrom: hr@techcorp.com\nChat: inbox\nContent: \
        Hi, thank you for your application. We'd like to schedule a final round interview \
        with our engineering director. Are you available next Tuesday at 2pm?";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        !result.tracking_complete.unwrap_or(true),
        "T12: Interview invite is NOT a job offer. Should keep tracking. Summary: {}",
        result.summary
    );
    // Interview invite is noteworthy though
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(
        !sms.is_empty(),
        "T12: Interview invite is noteworthy update for someone waiting for an offer"
    );

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// T13: Order cancellation resolves tracking (negative resolution)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t13_order_cancelled_resolves() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [platform:email] [sender:Amazon] [topic:package delivery]\nWatch for delivery of laptop from Amazon.",
        1,
    );
    let matched = "Platform: email\nFrom: order-update@amazon.com\nChat: inbox\nContent: \
        We're sorry, but your order for the MacBook Pro has been cancelled and a full refund \
        of $2,499.00 has been issued to your payment method. The refund should appear within 5-7 business days.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        result.tracking_complete.unwrap_or(false),
        "T13: Order cancellation should resolve tracking (negative resolution). Summary: {}",
        result.summary
    );

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  tracking_complete: {:?}", result.tracking_complete);
}

// =============================================================================
// T14: Server high CPU but still up - not the tracked condition
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t14_server_high_cpu_still_up() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:monitoring] [topic:server down]\nNotify user if the production server goes down or becomes unreachable.",
        0,
    );
    let matched = "Platform: email\nFrom: alerts@monitoring-service.com\nChat: inbox\nContent: \
        WARNING: Server prod-web-01 CPU usage at 95%. All health checks passing. \
        Server is responding normally to requests. No action required at this time.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        !result.tracking_complete.unwrap_or(true),
        "T14: High CPU but server still up - not the tracked condition. Summary: {}",
        result.summary
    );
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(
        sms.is_empty(),
        "T14: Server is up and responding, not what user is tracking. Should not notify. Got: {}",
        sms
    );

    eprintln!("  SMS (should be empty): {:?}", result.sms_message);
}

// =============================================================================
// T15: "Deliver" in customer service context - misleading keyword
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t15_deliver_in_wrong_context() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [platform:email] [sender:Amazon] [topic:package delivery]\nWatch for delivery of headphones from Amazon. Notify when delivered.",
        0,
    );
    let matched = "Platform: email\nFrom: customer-service@amazon.com\nChat: inbox\nContent: \
        At Amazon, we deliver excellent customer service to all our valued customers. \
        Please take a moment to complete our customer satisfaction survey about your \
        recent support interaction. Your feedback helps us serve you better.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        !result.tracking_complete.unwrap_or(true),
        "T15: 'deliver customer service' is not package delivery. Summary: {}",
        result.summary
    );
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(
        sms.is_empty(),
        "T15: Survey email with 'deliver' keyword should not trigger notification. Got: {}",
        sms
    );

    eprintln!("  SMS (should be empty): {:?}", result.sms_message);
}

// =============================================================================
// T16: Partial keyword match - wrong project from right sender
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t16_partial_keyword_wrong_project() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [platform:whatsapp] [sender:John] [topic:Alpha project deadline]\nWaiting for John's reply about the Alpha project deadline.",
        0,
    );
    let matched = "Platform: whatsapp\nFrom: john\nChat: John\nContent: \
        Hey, I'm replying to the company newsletter. By the way, project Sunshine \
        is going well and should be done by end of month. Talk soon!";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        !result.tracking_complete.unwrap_or(true),
        "T16: John's reply about project Sunshine is not about Alpha project. Summary: {}",
        result.summary
    );
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(
        sms.is_empty(),
        "T16: Wrong project should not trigger notification. Got: {}",
        sms
    );

    eprintln!("  SMS (should be empty): {:?}", result.sms_message);
}

// =============================================================================
// T17: Within cooldown - different info, SMS blocked
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t17_within_cooldown_blocked() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // last_notified 2 hours ago - within 6-hour cooldown
    let recent_ts = now - 2 * 3600;
    let summary = format!(
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:Amazon] [topic:delivery] [last_notified:{}]\nWatch for Amazon package delivery.\nPrevious: package shipped from warehouse.",
        recent_ts
    );
    let mut item = make_tracking_item(user.id, &summary, 0);
    item.due_at = Some(now + 5 * 86400);

    let matched = "Platform: email\nFrom: amazon-shipping@amazon.com\nChat: inbox\nContent: \
        Your package has arrived at the local distribution center and is scheduled for delivery tomorrow.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(
        sms.is_empty(),
        "T17: Within 6h cooldown (2h ago), SMS should be blocked. Got: {}",
        sms
    );

    eprintln!("  SMS (should be empty): {:?}", result.sms_message);
}

// =============================================================================
// T18: Past cooldown - new info, SMS allowed
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t18_past_cooldown_allowed() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // last_notified 8 hours ago - past 6-hour cooldown
    let old_ts = now - 8 * 3600;
    let summary = format!(
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:Amazon] [topic:delivery] [last_notified:{}]\nWatch for Amazon package delivery.\nPrevious: package shipped.",
        old_ts
    );
    let mut item = make_tracking_item(user.id, &summary, 0);
    item.due_at = Some(now + 5 * 86400);

    let matched = "Platform: email\nFrom: amazon-shipping@amazon.com\nChat: inbox\nContent: \
        Your package is out for delivery and will arrive today between 1pm and 5pm.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(
        !sms.is_empty(),
        "T18: Past cooldown (8h > 6h), notification should pass. Summary: {}",
        result.summary
    );

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// T19: Very long matched message (2000+ chars) - valid result, SMS under 480
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t19_very_long_matched_message() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [platform:email] [sender:HR] [topic:job offer]\nWaiting for job offer details from HR.",
        1,
    );

    // Very long email (2000+ chars) with realistic varied content
    let long_content = "Platform: email\nFrom: hr@company.com\nChat: inbox\nContent: \
        Dear Candidate, We are pleased to extend you an official offer for the position of Senior Software Engineer at TechCorp Inc. \
        This letter outlines the terms and conditions of your employment. Your starting salary will be $150,000 per year, \
        paid bi-weekly. You will also be eligible for an annual performance bonus of up to 15% of your base salary, \
        based on individual and company performance metrics. Your benefits package includes comprehensive health insurance \
        (medical, dental, and vision) for you and your dependents, with the company covering 90% of premiums. \
        You will receive 25 days of paid time off per year, plus 10 company holidays. The company offers a 401(k) \
        retirement plan with a 4% employer match, vesting immediately. You will be granted 10,000 stock options with a \
        four-year vesting schedule and a one-year cliff. Your start date will be March 15, 2026. You will report to the \
        Director of Engineering and be based in our San Francisco office with hybrid work flexibility (3 days in office, \
        2 days remote). Additional perks include a $2,000 annual learning and development budget, monthly team events, \
        free lunch on in-office days, and a one-time $1,500 home office setup allowance. We also offer 16 weeks of paid \
        parental leave and an employee assistance program. \
        Your team will consist of 8 engineers working on our core platform services. The tech stack includes Rust, \
        TypeScript, PostgreSQL, and Kubernetes. You will be expected to participate in on-call rotations (approximately \
        one week per month) and contribute to architectural decisions for the platform. \
        Relocation assistance of up to $10,000 is available if you are moving from outside the Bay Area. \
        This offer is contingent upon successful completion of a background check and verification of your right to \
        work in the United States. Please sign and return this offer letter within 7 business days. If you have any \
        questions, please contact Sarah Johnson in HR at sarah.johnson@techcorp.com or call 415-555-0142. \
        We look forward to welcoming you to the team. Best regards, Michael Chen, VP of Engineering, TechCorp Inc.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(long_content)).await;

    // Should process without error
    let sms = result.sms_message.as_deref().unwrap_or("");
    if !sms.is_empty() {
        assert!(
            sms.len() <= 480,
            "T19: SMS exceeds 480 chars (got {}): {}",
            sms.len(),
            sms
        );
    }

    eprintln!("  SMS ({} chars): {:?}", sms.len(), result.sms_message);
    eprintln!("  tracking_complete: {:?}", result.tracking_complete);
}

// =============================================================================
// T20: Minimal content "ok" with scope:any - processes without error
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_t20_minimal_content_ok_scope_any() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [platform:whatsapp] [sender:mom] [scope:any]\nWatch for any message from mom.",
        1,
    );
    let matched = "Platform: whatsapp\nFrom: mom\nChat: Mom\nContent: ok";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    // With scope:any from mom, "ok" is a valid message - should notify.
    // tracking_complete can be true (one-shot match) or false (keep watching) - both are valid.
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(
        !sms.is_empty(),
        "T20: scope:any from mom - 'ok' should produce notification. Summary: {}",
        result.summary
    );

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// R1: tracking_complete=true -> due_at=None (delete)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_r1_tracking_complete_deletes() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:client] [topic:invoice payment]\nInvoice #4523 for $2,400 sent to Acme Corp. Watch for payment confirmation.",
        0,
    );
    let matched = "Platform: email\nFrom: payments@acmecorp.com\nChat: inbox\nContent: \
        Payment received: Invoice #4523, amount $2,400.00 has been processed.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    assert!(
        result.tracking_complete.unwrap_or(false),
        "Should be complete"
    );
    assert!(
        result.due_at.is_none(),
        "R1: tracking_complete=true -> due_at=None"
    );

    eprintln!("  due_at: {:?}", result.due_at);
}

// =============================================================================
// R2: At/past deadline -> due_at=None (auto-delete)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_r2_past_deadline_auto_delete() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let mut item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:HR] [topic:contract]\nWaiting for signed contract from HR.",
        0,
    );
    item.due_at = Some(now - 3600); // 1 hour past deadline

    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        result.due_at.is_none(),
        "R2: Past-deadline item should be auto-deleted. due_at: {:?}",
        result.due_at
    );

    eprintln!("  due_at (should be None): {:?}", result.due_at);
}

#[tokio::test]
#[ignore]
async fn test_r2_exact_deadline_auto_delete() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let mut item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:chat]\nWaiting for confirmation message from event organizer.",
        0,
    );
    item.due_at = Some(now); // Exactly at deadline

    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        result.due_at.is_none(),
        "R2: Item at exact deadline should be auto-deleted. due_at: {:?}",
        result.due_at
    );

    eprintln!("  due_at (should be None): {:?}", result.due_at);
}

// =============================================================================
// R3: Before deadline, not complete -> due_at preserved
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_r3_before_deadline_preserves() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:silent]\nTrack Bitcoin price. Notify user when it hits $100,000.",
        0,
    );
    // make_tracking_item sets due_at 5 days in future
    let result = process_item_with_retry(&state, user.id, &item).await;

    assert!(
        result.due_at.is_some(),
        "R3: Before-deadline tracking should preserve due_at"
    );

    eprintln!("  due_at: {:?}", result.due_at);
    eprintln!("  tracking_complete: {:?}", result.tracking_complete);
}

// =============================================================================
// R4: Cooldown blocks SMS within 6h
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_r4_cooldown_blocks_recent() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // [last_notified:X] set 2 hours ago - within 6-hour cooldown
    let recent_ts = now - 2 * 3600;
    let summary = format!(
        "[type:tracking] [notify:sms] [fetch:email] [last_notified:{}]\nWatch for email from boss about quarterly report. Notify when it arrives.",
        recent_ts
    );
    let mut item = make_tracking_item(user.id, &summary, 0);
    item.due_at = Some(now + 5 * 86400);

    let matched = "Platform: email\nFrom: boss@company.com\nChat: inbox\nContent: \
        Here's the quarterly report update. Numbers look good, revenue up 15%.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(
        sms.is_empty(),
        "R4: 6-hour cooldown should block notification. last_notified was {}s ago. Got SMS: {}",
        now - recent_ts,
        sms
    );

    eprintln!("  SMS (should be empty): {:?}", result.sms_message);
}

// =============================================================================
// R5: Cooldown allows SMS after 6h+
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_r5_cooldown_allows_old() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // [last_notified:X] set 8 hours ago - past cooldown
    let old_ts = now - 8 * 3600;
    let summary = format!(
        "[type:tracking] [notify:sms] [fetch:email] [last_notified:{}]\nWatch for payment confirmation from Acme Corp for invoice #789. Notify when paid.",
        old_ts
    );
    let mut item = make_tracking_item(user.id, &summary, 0);
    item.due_at = Some(now + 5 * 86400);

    let matched = "Platform: email\nFrom: payments@acmecorp.com\nChat: inbox\nContent: \
        Payment of $3,200 for invoice #789 has been processed successfully.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(
        !sms.is_empty(),
        "R5: Past cooldown (8h > 6h), notification should pass. Summary: {}",
        result.summary
    );

    eprintln!("  SMS: {}", sms);
}

// =============================================================================
// R6: [last_notified:X] stamped after SMS passes
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_r6_stamps_last_notified() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:email] [platform:email] [sender:Amazon] [topic:refund]\nWatch for Amazon refund confirmation for returned headphones. Notify when processed.",
        0,
    );
    let matched = "Platform: email\nFrom: returns@amazon.com\nChat: inbox\nContent: \
        Your refund of $299.99 for Sony WH-1000XM5 has been processed. Expect it in 3-5 business days.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    let sms = result.sms_message.as_deref().unwrap_or("");
    if !sms.is_empty() {
        let first_line = result.summary.lines().next().unwrap_or("");
        assert!(
            first_line.contains("[last_notified:"),
            "R6: [last_notified:X] should be stamped on first line after notification. First line: {}",
            first_line
        );
    }

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  Summary: {}", result.summary);
}

#[tokio::test]
#[ignore]
async fn test_r6_no_stamp_when_no_notification() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:sms] [fetch:chat] [platform:whatsapp] [sender:boss] [topic:budget approval]\nWaiting for boss to approve the Q2 budget.",
        0,
    );
    let matched = "Platform: whatsapp\nFrom: boss\nChat: Boss\nContent: \
        Reminder: team lunch is at 12:30 today in the cafeteria.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    let sms = result.sms_message.as_deref().unwrap_or("");
    if sms.is_empty() {
        assert!(
            !result.summary.contains("[last_notified:"),
            "R6: Should NOT stamp [last_notified:] when no notification sent. Summary: {}",
            result.summary
        );
    }

    eprintln!("  SMS: {:?}", result.sms_message);
    eprintln!("  Summary: {}", result.summary);
}

// =============================================================================
// R7: Priority 0 - SMS generated but handle_triggered_item_result skips send
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_r7_priority_escalation_for_urgent() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:silent] [fetch:email] [platform:email] [sender:bank] [topic:fraud alert]\nWatch for fraud alerts from the bank.",
        0,
    );
    let matched = "Platform: email\nFrom: alerts@mybank.com\nChat: inbox\nContent: \
        URGENT FRAUD ALERT: Suspicious transaction of $4,500 detected on your account ending in 7823. \
        If you did not authorize this, call 1-800-BANK immediately.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    let priority = result.priority.unwrap_or(0);
    assert!(
        priority >= 1,
        "R7: Fraud alert should escalate priority from 0 to at least 1, got {}",
        priority
    );
    let sms = result.sms_message.as_deref().unwrap_or("");
    assert!(!sms.is_empty(), "Fraud alert should produce notification");

    eprintln!("  Priority: {}", priority);
    eprintln!("  SMS: {}", sms);
}

#[tokio::test]
#[ignore]
async fn test_r7_preserves_silent_for_routine() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let item = make_tracking_item(
        user.id,
        "[type:tracking] [notify:silent] [fetch:email] [platform:email] [sender:Amazon] [topic:book order]\nWatch for Amazon order confirmation for the programming book.",
        0,
    );
    let matched = "Platform: email\nFrom: order-update@amazon.com\nChat: inbox\nContent: \
        Order update: Your order of 'Programming Rust, 2nd Edition' is being prepared for shipment.";

    let result = process_item_with_retry_matched(&state, user.id, &item, Some(matched)).await;

    let priority = result.priority.unwrap_or(0);
    assert!(
        priority == 0,
        "R7: Routine update on silent item should stay priority 0, got {}",
        priority
    );

    eprintln!("  Priority: {}", priority);
    eprintln!("  SMS: {:?}", result.sms_message);
}

// =============================================================================
// Monitor matching - incoming messages that SHOULD match
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_monitor_match_email_from_hr() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
        &TestItemParams::tracking(
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
        &TestItemParams::tracking(
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
// Monitor matching - incoming messages that should NOT match
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_monitor_no_match_unrelated_email() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(user.id, "[platform:email] [sender:HR] [topic:job offer]\nWatch for emails from HR about the job offer."),
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
        &TestItemParams::tracking(
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
        &TestItemParams::tracking(
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
// Monitor matching - multiple items, correct one should match
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_monitor_match_correct_item_among_multiple() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item_hr = create_test_item(
        &state,
        &TestItemParams::tracking(user.id, "[platform:email] [sender:HR] [topic:job offer]\nWatch for emails from HR about the job offer."),
    );
    let item_mom = create_test_item(
        &state,
        &TestItemParams::tracking(
            user.id,
            "[platform:whatsapp] [sender:mom] [scope:any]\nWatch for messages from mom.",
        ),
    );
    let item_package = create_test_item(
        &state,
        &TestItemParams::tracking(
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
// Same-platform matches
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_email_sender_and_topic() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
}

#[tokio::test]
#[ignore]
async fn test_match_whatsapp_scope_any() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
}

#[tokio::test]
#[ignore]
async fn test_match_telegram_with_topic() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
}

// =============================================================================
// Cross-platform matches
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_cross_platform_boss_report() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
            user.id,
            "[platform:email] [sender:boss] [topic:quarterly report]\nWatch for emails from boss about the quarterly report.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: whatsapp\nFrom: boss\nChat: Boss\nContent: \
        Hey, I just sent you the quarterly report via the shared drive. Take a look when you can.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Cross-platform: boss + quarterly report topic should match");
    assert_eq!(response.task_id, Some(item_id));
}

#[tokio::test]
#[ignore]
async fn test_match_cross_platform_chat_to_email() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
            user.id,
            "[platform:chat] [sender:John] [topic:project deadline]\nWatch for messages from John about the project deadline.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: email\nFrom: John\nChat: inbox\nContent: \
        Hi, just wanted to let you know the project deadline has been moved to next Friday.";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Cross-platform: John + project deadline should match from email");
    assert_eq!(response.task_id, Some(item_id));
}

// =============================================================================
// Same sender, different topic (SHOULD NOT match)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_no_match_amazon_wrong_product() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
        &TestItemParams::tracking(
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
// Similar topic, different sender (SHOULD NOT match)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_no_match_recruiter_not_hr() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
            user.id,
            "[platform:email] [sender:HR] [topic:job offer]\nWatch for emails from the company's HR department about the pending job offer.",
        ),
    );

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
        &TestItemParams::tracking(
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
// Group chats
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_sender_in_group_chat() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
}

#[tokio::test]
#[ignore]
async fn test_no_match_wrong_sender_in_group() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
            user.id,
            "[platform:whatsapp] [sender:John] [topic:project update]\nWatch for messages from John about project updates.",
        ),
    );

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
// Non-English messages
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_finnish_message_scope_any() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
            user.id,
            "[platform:whatsapp] [sender:mom] [scope:any]\nWatch for any message from mom.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: whatsapp\nFrom: mom\nChat: Mom\nContent: \
        Hei kulta, muista ottaa takki mukaan, huomenna tulee kylma!";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Finnish message from mom should match scope:any monitor");
    assert_eq!(response.task_id, Some(item_id));
}

#[tokio::test]
#[ignore]
async fn test_match_spanish_message_topic() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
            user.id,
            "[platform:chat] [sender:Carlos] [topic:meeting]\nWatch for messages from Carlos about the meeting.",
        ),
    );
    let item_id = item.id.unwrap();

    let message = "Platform: whatsapp\nFrom: carlos\nChat: Carlos\nContent: \
        Oye, la reunion se cambio para las 3 de la tarde. No te olvides!";

    let result = check_item_monitor_match(&state, user.id, message, &[item]).await;
    let response = result
        .expect("LLM call should succeed")
        .expect("Spanish meeting message from Carlos should match");
    assert_eq!(response.task_id, Some(item_id));
}

// =============================================================================
// Media messages
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_image_from_mom_scope_any() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
}

#[tokio::test]
#[ignore]
async fn test_no_match_image_from_mom_with_topic() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
// Multiple items - correct one matches
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_correct_among_three_items() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item_hr = create_test_item(
        &state,
        &TestItemParams::tracking(
            user.id,
            "[platform:email] [sender:HR] [topic:job offer]\nWatch for HR job offer email.",
        ),
    );
    let item_mom = create_test_item(
        &state,
        &TestItemParams::tracking(
            user.id,
            "[platform:whatsapp] [sender:mom] [scope:any]\nWatch for any WhatsApp from mom.",
        ),
    );
    let item_hackathon = create_test_item(
        &state,
        &TestItemParams::tracking(
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
}

#[tokio::test]
#[ignore]
async fn test_match_correct_amazon_item_among_two() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item_headphones = create_test_item(
        &state,
        &TestItemParams::tracking(
            user.id,
            "[platform:email] [sender:Amazon] [topic:headphones]\nWatch for Amazon headphones shipping update.",
        ),
    );
    let item_refund = create_test_item(
        &state,
        &TestItemParams::tracking(
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
}

// =============================================================================
// Short messages
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_short_message_scope_any() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
}

#[tokio::test]
#[ignore]
async fn test_no_match_short_message_with_topic() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
// Forwarded messages
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_no_match_forwarded_email() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
            user.id,
            "[platform:email] [sender:boss] [topic:quarterly report]\nWatch for emails from boss about the quarterly report.",
        ),
    );

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
// Platform wildcards
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_platform_any_email() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
}

#[tokio::test]
#[ignore]
async fn test_match_platform_chat_matches_signal() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
}

// =============================================================================
// Legacy format (backward compatibility for monitor matching)
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_match_legacy_format_mom_email() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
}

#[tokio::test]
#[ignore]
async fn test_no_match_legacy_format_wrong_sender() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(
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
// No match at all
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_no_match_tech_news_bot() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    set_plan_type(&state, user.id, "autopilot");

    let item_mom = create_test_item(
        &state,
        &TestItemParams::tracking(
            user.id,
            "[platform:whatsapp] [sender:mom] [scope:any]\nWatch for any WhatsApp from mom.",
        ),
    );
    let item_hr = create_test_item(
        &state,
        &TestItemParams::tracking(
            user.id,
            "[platform:email] [sender:HR] [topic:job offer]\nWatch for HR job offer email.",
        ),
    );
    let item_hackathon = create_test_item(
        &state,
        &TestItemParams::tracking(
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
