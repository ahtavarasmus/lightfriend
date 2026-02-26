//! LLM Integration Tests for Auto-Creating Tracking Items from Bridge Messages
//!
//! Tests verify that `check_message_trackable_items()` correctly:
//! - Creates silent tracking items for actionable messages (invoices, deliveries, questions, etc.)
//! - Skips casual chat, greetings, acknowledgements
//! - Deduplicates by topic within the same room
//! - Sets correct item fields (monitor=true, priority=0, next_check_at, source_id, tags)
//!
//! All LLM tests are gated with `#[ignore]` because they cost real API tokens.
//! Run explicitly with: `cargo test --test message_trackable_test -- --ignored --test-threads=1`
//!
//! Requirements:
//! - TINFOIL_API_KEY and OPENROUTER_API_KEY set in backend/.env
//! - Network access to Tinfoil API

use backend::models::user_models::Item;
use backend::proactive::utils::check_message_trackable_items;
use backend::test_utils::{
    create_test_state, create_test_user, get_user_items, set_plan_type, TestUserParams,
};
use backend::{AiConfig, AppState, UserCoreOps};
use std::sync::Arc;

const MAX_RETRIES: usize = 5;

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

/// Create a test user with location, timezone, and autopilot plan.
fn setup_user(state: &Arc<AppState>) -> backend::models::user_models::User {
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

    set_plan_type(state, user.id, "autopilot");
    user
}

/// Call check_message_trackable_items. Returns Ok(items) on success, Err on API failure.
async fn try_check_message(
    state: &Arc<AppState>,
    user_id: i32,
    service: &str,
    room_id: &str,
    sender: &str,
    content: &str,
) -> Result<Vec<Item>, String> {
    check_message_trackable_items(state, user_id, service, room_id, sender, content)
        .await
        .map_err(|e| format!("{}", e))?;
    Ok(get_user_items(state, user_id))
}

/// Call check_message with retries (handles both API errors and LLM non-determinism).
/// For "should create" tests: retries until an item is created or MAX_RETRIES exhausted.
/// Each retry uses a fresh state + user.
async fn check_message_expect_item(
    service: &str,
    sender: &str,
    content: &str,
) -> (Arc<AppState>, i32, Vec<Item>) {
    let mut last_err = None;
    for attempt in 1..=MAX_RETRIES {
        let state = create_llm_test_state();
        let user = setup_user(&state);
        match try_check_message(
            &state,
            user.id,
            service,
            "!room_test:matrix.org",
            sender,
            content,
        )
        .await
        {
            Ok(items) if !items.is_empty() => return (state, user.id, items),
            Ok(_) => {
                eprintln!(
                    "  Attempt {}/{}: LLM did not create item for: \"{}\"",
                    attempt, MAX_RETRIES, content
                );
            }
            Err(e) => {
                eprintln!(
                    "  Attempt {}/{}: API error for \"{}\": {}",
                    attempt, MAX_RETRIES, content, e
                );
                last_err = Some(e);
            }
        }
    }
    panic!(
        "Failed to create item after {} retries for: \"{}\". Last error: {:?}",
        MAX_RETRIES, content, last_err
    );
}

/// Call check_message with retries for "should NOT create" tests.
/// Retries on API errors but expects no items on successful calls.
async fn check_message_expect_no_item(
    state: &Arc<AppState>,
    user_id: i32,
    service: &str,
    room_id: &str,
    sender: &str,
    content: &str,
) -> Vec<Item> {
    for attempt in 1..=MAX_RETRIES {
        match try_check_message(state, user_id, service, room_id, sender, content).await {
            Ok(items) => return items,
            Err(e) => {
                eprintln!(
                    "  Attempt {}/{}: API error (retrying): {}",
                    attempt, MAX_RETRIES, e
                );
                if attempt == MAX_RETRIES {
                    panic!(
                        "All {} attempts failed with API errors for: \"{}\". Last: {}",
                        MAX_RETRIES, content, e
                    );
                }
            }
        }
    }
    unreachable!()
}

/// Assert that exactly one item was created with the expected properties.
fn assert_tracking_item(items: &[Item], expected_summary_words: &[&str]) {
    assert!(
        !items.is_empty(),
        "Expected a tracking item to be created after {} retries, got none",
        MAX_RETRIES
    );
    assert_eq!(
        items.len(),
        1,
        "Expected exactly 1 item, got {}",
        items.len()
    );

    let item = &items[0];

    // Must be a monitor with priority 0 (silent)
    assert!(
        item.monitor,
        "Expected monitor=true, got false. Summary: {}",
        item.summary
    );
    assert_eq!(
        item.priority, 0,
        "Expected priority=0 (silent), got {}. Summary: {}",
        item.priority, item.summary
    );

    // Must have next_check_at set
    assert!(
        item.next_check_at.is_some(),
        "Expected next_check_at to be set, got None. Summary: {}",
        item.summary
    );

    // Must have source_id starting with "msg_"
    assert!(
        item.source_id
            .as_ref()
            .is_some_and(|s| s.starts_with("msg_")),
        "Expected source_id starting with 'msg_', got: {:?}. Summary: {}",
        item.source_id,
        item.summary
    );

    // Must have correct tags in summary
    assert!(
        item.summary.contains("[type:tracking]"),
        "Expected [type:tracking] tag in summary: {}",
        item.summary
    );
    assert!(
        item.summary.contains("[notify:silent]"),
        "Expected [notify:silent] tag in summary: {}",
        item.summary
    );
    assert!(
        item.summary.contains("[platform:"),
        "Expected [platform:] tag in summary: {}",
        item.summary
    );
    assert!(
        item.summary.contains("[sender:"),
        "Expected [sender:] tag in summary: {}",
        item.summary
    );
    assert!(
        item.summary.contains("[topic:"),
        "Expected [topic:] tag in summary: {}",
        item.summary
    );

    // Check expected keywords in the description (after the tag line)
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

/// Assert that no items were created.
fn assert_no_item(items: &[Item], message: &str) {
    assert!(
        items.is_empty(),
        "Expected NO item for \"{}\", but got {} item(s): {:?}",
        message,
        items.len(),
        items.iter().map(|i| &i.summary).collect::<Vec<_>>()
    );
}

/// Assert next_check_at is in the future and within a reasonable range.
fn assert_next_check_at_in_range(item: &Item, min_offset_secs: i64, max_offset_secs: i64) {
    let check_at = item.next_check_at.expect("next_check_at should be set") as i64;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let offset = check_at - now;
    assert!(
        offset >= min_offset_secs && offset <= max_offset_secs,
        "next_check_at offset is {}s, expected between {}s and {}s. check_at={}, now={}",
        offset,
        min_offset_secs,
        max_offset_secs,
        check_at,
        now,
    );
}

// =============================================================================
// 1. Unit tests (no LLM needed)
// =============================================================================

#[tokio::test]
async fn test_short_message_skip() {
    // Messages under 10 chars should be skipped without any LLM call
    let state = create_test_state();
    let params = TestUserParams::finland_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    for short_msg in &["ok", "hi", "thanks", "lol", "sure", ""] {
        let result = check_message_trackable_items(
            &state,
            user.id,
            "whatsapp",
            "!room:matrix.org",
            "john",
            short_msg,
        )
        .await;
        assert!(
            result.is_ok(),
            "Short message '{}' should succeed: {:?}",
            short_msg,
            result.err()
        );

        let items = get_user_items(&state, user.id);
        assert!(
            items.is_empty(),
            "Short message '{}' should not create item",
            short_msg
        );
    }
}

#[tokio::test]
async fn test_exactly_10_chars_not_skipped() {
    // A message of exactly 10 chars should NOT be skipped by the length check.
    // It will fail at the LLM call (no real API key in test state), which proves
    // it got past the short-message guard.
    let state = create_test_state();
    let params = TestUserParams::finland_user(10.0, 5.0);
    let user = create_test_user(&state, &params);
    set_plan_type(&state, user.id, "autopilot");

    state
        .user_core
        .ensure_user_info_exists(user.id)
        .expect("user_info");

    let ten_char_msg = "0123456789"; // exactly 10 chars
    let result = check_message_trackable_items(
        &state,
        user.id,
        "whatsapp",
        "!room:matrix.org",
        "john",
        ten_char_msg,
    )
    .await;

    // Should fail because test state has no real API key - proving we got past the length check
    assert!(
        result.is_err(),
        "10-char message should attempt LLM call (and fail without API key)"
    );
}

// =============================================================================
// 2. Trackable messages - SHOULD create items
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_trackable_invoice() {
    let (_state, _user_id, items) = check_message_expect_item(
        "whatsapp",
        "john",
        "Hey, please pay invoice #4521 for $450. Due by March 15th.",
    )
    .await;

    assert_tracking_item(&items, &["invoice"]);
}

#[tokio::test]
#[ignore]
async fn test_trackable_delivery() {
    let (_state, _user_id, items) = check_message_expect_item(
        "whatsapp",
        "amazon_notify",
        "Your package has shipped! Tracking number: 1Z999AA10123456784. Expected delivery: Thursday.",
    )
    .await;

    assert_tracking_item(&items, &["tracking"]);
}

#[tokio::test]
#[ignore]
async fn test_trackable_question_needing_response() {
    let (_state, _user_id, items) = check_message_expect_item(
        "telegram",
        "boss",
        "Can you send me the Q4 report by end of day tomorrow? I need it for the board meeting.",
    )
    .await;

    assert_tracking_item(&items, &["report"]);
}

#[tokio::test]
#[ignore]
async fn test_trackable_appointment() {
    let (_state, _user_id, items) = check_message_expect_item(
        "signal",
        "dentist_office",
        "Reminder: Your dental appointment is scheduled for Thursday at 2:30 PM. Please arrive 15 minutes early.",
    )
    .await;

    assert_tracking_item(&items, &["dental"]);
}

#[tokio::test]
#[ignore]
async fn test_trackable_deadline() {
    let (_state, _user_id, items) = check_message_expect_item(
        "whatsapp",
        "project_lead",
        "The proposal deadline has been moved to Friday 5pm. Make sure your section is done by then.",
    )
    .await;

    assert_tracking_item(&items, &["deadline", "friday"]);
}

#[tokio::test]
#[ignore]
async fn test_trackable_commitment_from_someone() {
    let (_state, _user_id, items) = check_message_expect_item(
        "whatsapp",
        "contractor",
        "I'll send you the signed contract and the first milestone estimate by Wednesday morning.",
    )
    .await;

    assert_tracking_item(&items, &["contract"]);
}

// =============================================================================
// 3. Non-trackable messages - should NOT create items
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_not_trackable_greeting() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let items = check_message_expect_no_item(
        &state,
        user.id,
        "whatsapp",
        "!room:matrix.org",
        "friend",
        "Hey! How are you doing?",
    )
    .await;

    assert_no_item(&items, "Hey! How are you doing?");
}

#[tokio::test]
#[ignore]
async fn test_not_trackable_acknowledgement() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let items = check_message_expect_no_item(
        &state,
        user.id,
        "whatsapp",
        "!room:matrix.org",
        "friend",
        "Ok got it, thanks for letting me know!",
    )
    .await;

    assert_no_item(&items, "Ok got it, thanks for letting me know!");
}

#[tokio::test]
#[ignore]
async fn test_not_trackable_casual_chat() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let items = check_message_expect_no_item(
        &state,
        user.id,
        "telegram",
        "!room:matrix.org",
        "buddy",
        "lol that movie was hilarious, we should watch the sequel",
    )
    .await;

    assert_no_item(&items, "lol that movie was hilarious");
}

#[tokio::test]
#[ignore]
async fn test_not_trackable_simple_reaction() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    let items = check_message_expect_no_item(
        &state,
        user.id,
        "signal",
        "!room:matrix.org",
        "coworker",
        "Haha nice one! That's great.",
    )
    .await;

    assert_no_item(&items, "Haha nice one! That's great.");
}

// =============================================================================
// 4. Item field verification
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_item_fields_correct() {
    let (state, user_id, items) = check_message_expect_item(
        "whatsapp",
        "supplier",
        "Invoice #8899 for office supplies - $230 due next Monday.",
    )
    .await;

    assert_eq!(items.len(), 1, "Expected exactly 1 item");
    let item = &items[0];

    // monitor=true, priority=0
    assert!(item.monitor, "Should be monitor=true");
    assert_eq!(item.priority, 0, "Should be priority 0 (silent)");

    // source_id format: msg_whatsapp_{room_id}_{topic}
    let source_id = item.source_id.as_ref().expect("source_id should be set");
    assert!(
        source_id.starts_with("msg_whatsapp_"),
        "source_id should start with 'msg_whatsapp_', got: {}",
        source_id
    );

    // Tags on first line
    let first_line = item.summary.lines().next().unwrap_or("");
    assert!(
        first_line.contains("[type:tracking]"),
        "First line should have [type:tracking]"
    );
    assert!(
        first_line.contains("[notify:silent]"),
        "First line should have [notify:silent]"
    );
    assert!(
        first_line.contains("[platform:whatsapp]"),
        "First line should have [platform:whatsapp]"
    );
    assert!(
        first_line.contains("[sender:supplier]"),
        "First line should have [sender:supplier]"
    );

    // Description on second line (after tags)
    let second_line = item.summary.lines().nth(1);
    assert!(
        second_line.is_some() && !second_line.unwrap().is_empty(),
        "Expected description on second line of summary"
    );

    // next_check_at should be set and in reasonable range (1 hour to 30 days)
    assert_next_check_at_in_range(item, 3600, 30 * 86400);

    // Verify the item is retrievable by source_id (dedup works)
    let exists = state
        .item_repository
        .item_exists_by_source(user_id, source_id)
        .expect("item_exists_by_source failed");
    assert!(exists, "Item should be findable by source_id for dedup");
}

#[tokio::test]
#[ignore]
async fn test_platform_tag_matches_service() {
    // Verify platform tag is set correctly for different services
    for (service, sender, msg) in &[
        (
            "telegram",
            "alice",
            "Please review the PR and merge it by tomorrow end of day",
        ),
        (
            "signal",
            "bob",
            "Your car insurance renewal is due March 1st, $890 payment needed",
        ),
    ] {
        let state = create_llm_test_state();
        let user = setup_user(&state);

        let mut items = Vec::new();
        for attempt in 0..MAX_RETRIES {
            match try_check_message(&state, user.id, service, "!room:matrix.org", sender, msg).await
            {
                Ok(result) if !result.is_empty() => {
                    items = result;
                    break;
                }
                Ok(_) => {
                    eprintln!(
                        "  Attempt {}/{}: no item for {}",
                        attempt + 1,
                        MAX_RETRIES,
                        service
                    );
                }
                Err(e) => {
                    eprintln!(
                        "  Attempt {}/{}: API error: {}",
                        attempt + 1,
                        MAX_RETRIES,
                        e
                    );
                }
            }
        }

        if !items.is_empty() {
            let tag = format!("[platform:{}]", service);
            assert!(
                items[0].summary.contains(&tag),
                "Expected {} in summary for service {}, got: {}",
                tag,
                service,
                items[0].summary
            );
        }
    }
}

// =============================================================================
// 5. Dedup behavior
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_dedup_same_topic_same_room() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    let room_id = "!dedup_test:matrix.org";

    // First message about a delivery - should create item
    let mut items = Vec::new();
    for attempt in 0..MAX_RETRIES {
        match try_check_message(
            &state,
            user.id,
            "whatsapp",
            room_id,
            "shop",
            "Your order #5567 has shipped! Tracking: 1Z999BB10123456784",
        )
        .await
        {
            Ok(result) if !result.is_empty() => {
                items = result;
                break;
            }
            Ok(_) => eprintln!("  Attempt {}/{}: no item", attempt + 1, MAX_RETRIES),
            Err(e) => eprintln!(
                "  Attempt {}/{}: API error: {}",
                attempt + 1,
                MAX_RETRIES,
                e
            ),
        }
    }
    assert!(
        !items.is_empty(),
        "First delivery message should create an item"
    );
    assert_eq!(items.len(), 1);

    // Second message about delivery in same room - should NOT create duplicate
    for attempt in 0..MAX_RETRIES {
        match try_check_message(
            &state,
            user.id,
            "whatsapp",
            room_id,
            "shop",
            "Update: your order #5567 is out for delivery today",
        )
        .await
        {
            Ok(_) => break,
            Err(e) => eprintln!(
                "  Dedup attempt {}/{}: API error: {}",
                attempt + 1,
                MAX_RETRIES,
                e
            ),
        }
    }
    let items_after = get_user_items(&state, user.id);
    assert_eq!(
        items_after.len(),
        1,
        "Second delivery message in same room should be deduped. Got {} items: {:?}",
        items_after.len(),
        items_after.iter().map(|i| &i.summary).collect::<Vec<_>>()
    );
}

#[tokio::test]
#[ignore]
async fn test_dedup_different_topics_same_room() {
    let state = create_llm_test_state();
    let user = setup_user(&state);
    let room_id = "!multi_topic:matrix.org";

    // First: delivery message
    let mut items = Vec::new();
    for attempt in 0..MAX_RETRIES {
        match try_check_message(
            &state,
            user.id,
            "whatsapp",
            room_id,
            "assistant",
            "Your laptop order shipped, tracking #9Z123456",
        )
        .await
        {
            Ok(result) if !result.is_empty() => {
                items = result;
                break;
            }
            Ok(_) => eprintln!("  Attempt {}/{}: no item", attempt + 1, MAX_RETRIES),
            Err(e) => eprintln!(
                "  Attempt {}/{}: API error: {}",
                attempt + 1,
                MAX_RETRIES,
                e
            ),
        }
    }
    assert!(!items.is_empty(), "Delivery message should create item");

    // Second: invoice message (different topic) - should create a SECOND item
    for attempt in 0..MAX_RETRIES {
        match try_check_message(
            &state,
            user.id,
            "whatsapp",
            room_id,
            "assistant",
            "Invoice #3344 for consulting services, $2000 due by March 20th",
        )
        .await
        {
            Ok(result) if result.len() >= 2 => break,
            Ok(_) => eprintln!(
                "  Attempt {}/{}: less than 2 items",
                attempt + 1,
                MAX_RETRIES
            ),
            Err(e) => eprintln!(
                "  Attempt {}/{}: API error: {}",
                attempt + 1,
                MAX_RETRIES,
                e
            ),
        }
    }
    let final_items = get_user_items(&state, user.id);
    assert!(
        final_items.len() >= 2,
        "Different topics in same room should create separate items. Got {} item(s): {:?}",
        final_items.len(),
        final_items.iter().map(|i| &i.summary).collect::<Vec<_>>()
    );
}

#[tokio::test]
#[ignore]
async fn test_dedup_same_topic_different_rooms() {
    let state = create_llm_test_state();
    let user = setup_user(&state);

    // Delivery in room A
    let mut items_a = Vec::new();
    for attempt in 0..MAX_RETRIES {
        match try_check_message(
            &state,
            user.id,
            "whatsapp",
            "!room_a:matrix.org",
            "shop_a",
            "Your order has shipped! Tracking: 1Z111AA10123456784",
        )
        .await
        {
            Ok(result) if !result.is_empty() => {
                items_a = result;
                break;
            }
            Ok(_) => eprintln!("  Attempt {}/{}: no item", attempt + 1, MAX_RETRIES),
            Err(e) => eprintln!(
                "  Attempt {}/{}: API error: {}",
                attempt + 1,
                MAX_RETRIES,
                e
            ),
        }
    }
    assert!(!items_a.is_empty(), "Room A delivery should create item");

    // Delivery in room B - different room, should create separate item
    for attempt in 0..MAX_RETRIES {
        match try_check_message(
            &state,
            user.id,
            "whatsapp",
            "!room_b:matrix.org",
            "shop_b",
            "Your package has been dispatched! Tracking: 1Z222BB10123456784",
        )
        .await
        {
            Ok(result) if result.len() >= 2 => break,
            Ok(_) => eprintln!(
                "  Attempt {}/{}: less than 2 items",
                attempt + 1,
                MAX_RETRIES
            ),
            Err(e) => eprintln!(
                "  Attempt {}/{}: API error: {}",
                attempt + 1,
                MAX_RETRIES,
                e
            ),
        }
    }
    let final_items = get_user_items(&state, user.id);
    assert!(
        final_items.len() >= 2,
        "Same topic from different rooms should create separate items. Got {} item(s): {:?}",
        final_items.len(),
        final_items.iter().map(|i| &i.summary).collect::<Vec<_>>()
    );
}

// =============================================================================
// 6. next_check_at timing
// =============================================================================

#[tokio::test]
#[ignore]
async fn test_next_check_at_urgent_item() {
    // Overdue invoice - should have a near-term check (within 1-3 days)
    let (_, _, items) = check_message_expect_item(
        "whatsapp",
        "billing",
        "URGENT: Your invoice #7788 for $1200 is overdue. Please pay immediately to avoid service interruption.",
    )
    .await;

    assert!(!items.is_empty(), "Urgent invoice should create item");
    // Urgent: expect check within 1 hour to 3 days
    assert_next_check_at_in_range(&items[0], 3600, 3 * 86400);
}

#[tokio::test]
#[ignore]
async fn test_next_check_at_non_urgent_item() {
    // Package in transit, no rush - should have a longer check window
    let (_, _, items) = check_message_expect_item(
        "whatsapp",
        "courier",
        "Your package is on its way! Estimated delivery in 5-7 business days. Tracking: ABC123.",
    )
    .await;

    assert!(!items.is_empty(), "Delivery message should create item");
    // Non-urgent: expect check in 2 days to 14 days
    assert_next_check_at_in_range(&items[0], 2 * 86400, 14 * 86400);
}
