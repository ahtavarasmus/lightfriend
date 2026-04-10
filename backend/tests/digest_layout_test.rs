//! Tests for the new sectioned digest layout helpers and queries.
//!
//! Pure-function tests cover the category scoring rules. DB-backed tests
//! verify the new repository queries used to fetch each section's content
//! (events due today, messages by urgency, with the seen/replied filters).

use backend::jobs::scheduler::{build_digest_for_user, category_score, strip_sender_prefix};
use backend::models::ontology_models::{NewOntEvent, NewOntMessage};
use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use backend::UserCoreOps;
use serial_test::serial;

// =============================================================================
// category_score: pure function
// =============================================================================

#[test]
fn category_score_orders_critical_categories_first() {
    assert!(category_score(Some("emergency")) > category_score(Some("financial")));
    assert!(category_score(Some("financial")) > category_score(Some("work")));
    assert!(category_score(Some("work")) > category_score(Some("social")));
    assert!(category_score(Some("social")) > category_score(Some("spam")));
}

#[test]
fn category_score_health_matches_financial() {
    // Health and financial are both 60 - both treated as high-stakes life domains
    assert_eq!(
        category_score(Some("health")),
        category_score(Some("financial"))
    );
}

#[test]
fn category_score_spam_is_strongly_negative() {
    let spam = category_score(Some("spam"));
    assert!(spam < -100, "spam should sort to the bottom, got {}", spam);
}

// =============================================================================
// strip_sender_prefix
// =============================================================================

#[test]
fn strip_sender_prefix_removes_simple_restatement() {
    assert_eq!(
        strip_sender_prefix("Mom is feeling unwell", "Mom"),
        "feeling unwell"
    );
    assert_eq!(
        strip_sender_prefix("Bank reports payment declined", "Bank"),
        "reports payment declined"
    );
}

#[test]
fn strip_sender_prefix_handles_colon_separator() {
    assert_eq!(
        strip_sender_prefix("Mom: feeling unwell", "Mom"),
        "feeling unwell"
    );
    assert_eq!(
        strip_sender_prefix("Boss: needs PRD review", "Boss"),
        "needs PRD review"
    );
}

#[test]
fn strip_sender_prefix_handles_possessive() {
    assert_eq!(
        strip_sender_prefix("Mom's project update", "Mom"),
        "project update"
    );
}

#[test]
fn strip_sender_prefix_strips_connector_verbs() {
    assert_eq!(
        strip_sender_prefix("Sarah was asking about Friday", "Sarah"),
        "asking about Friday"
    );
    assert_eq!(
        strip_sender_prefix("John has sent the doc", "John"),
        "sent the doc"
    );
}

#[test]
fn strip_sender_prefix_case_insensitive() {
    assert_eq!(
        strip_sender_prefix("MOM is feeling unwell", "mom"),
        "feeling unwell"
    );
}

#[test]
fn strip_sender_prefix_leaves_unrelated_summaries_alone() {
    // Summary doesn't start with the sender — return as-is.
    assert_eq!(
        strip_sender_prefix("payment declined", "Bank"),
        "payment declined"
    );
    assert_eq!(
        strip_sender_prefix("asking about dinner plans", "Mom"),
        "asking about dinner plans"
    );
}

#[test]
fn strip_sender_prefix_falls_back_when_strip_empties() {
    // If stripping the sender would leave nothing, keep the original.
    assert_eq!(strip_sender_prefix("Mom", "Mom"), "Mom");
}

#[test]
fn category_score_unknown_is_neutral() {
    // Unknown / unclassified categories sit between social and work,
    // so they don't dominate but also don't disappear.
    let unknown = category_score(None);
    assert!(unknown < category_score(Some("work")));
    assert!(unknown > category_score(Some("social")));
}

// =============================================================================
// get_pending_messages_by_urgency
// =============================================================================

#[test]
#[serial]
fn pending_messages_by_urgency_filters_by_level() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = 2_000_000;

    // Insert one message of each urgency
    let crit = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "!r1".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Alice".to_string(),
            content: "emergency".to_string(),
            person_id: None,
            created_at: now,
        })
        .unwrap();
    state
        .ontology_repository
        .update_message_classification(crit.id, "critical", "emergency", None, None, None)
        .unwrap();

    let high = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "!r2".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Bob".to_string(),
            content: "urgent".to_string(),
            person_id: None,
            created_at: now,
        })
        .unwrap();
    state
        .ontology_repository
        .update_message_classification(high.id, "high", "work", None, None, None)
        .unwrap();

    let med = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "!r3".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Carol".to_string(),
            content: "fyi".to_string(),
            person_id: None,
            created_at: now,
        })
        .unwrap();
    state
        .ontology_repository
        .update_message_classification(med.id, "medium", "social", None, None, None)
        .unwrap();

    // Critical-only query returns only critical
    let crits = state
        .ontology_repository
        .get_pending_messages_by_urgency(user.id, &["critical"], now - 86400, 100)
        .unwrap();
    assert_eq!(crits.len(), 1);
    assert_eq!(crits[0].id, crit.id);

    // High-only returns only high
    let highs = state
        .ontology_repository
        .get_pending_messages_by_urgency(user.id, &["high"], now - 86400, 100)
        .unwrap();
    assert_eq!(highs.len(), 1);
    assert_eq!(highs[0].id, high.id);

    // Medium-only returns only medium
    let meds = state
        .ontology_repository
        .get_pending_messages_by_urgency(user.id, &["medium"], now - 86400, 100)
        .unwrap();
    assert_eq!(meds.len(), 1);
    assert_eq!(meds[0].id, med.id);

    // Multi-level query returns all three
    let all = state
        .ontology_repository
        .get_pending_messages_by_urgency(user.id, &["critical", "high", "medium"], now - 86400, 100)
        .unwrap();
    assert_eq!(all.len(), 3);
}

#[test]
#[serial]
fn pending_messages_by_urgency_excludes_already_delivered() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = 2_100_000;

    let msg = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "!r1".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Alice".to_string(),
            content: "hey".to_string(),
            person_id: None,
            created_at: now,
        })
        .unwrap();
    state
        .ontology_repository
        .update_message_classification(msg.id, "medium", "social", None, None, None)
        .unwrap();

    // Mark as delivered
    state
        .ontology_repository
        .mark_digest_delivered(&[msg.id], now)
        .unwrap();

    // Should no longer appear
    let pending = state
        .ontology_repository
        .get_pending_messages_by_urgency(user.id, &["medium"], now - 86400, 100)
        .unwrap();
    assert!(pending.is_empty());
}

#[test]
#[serial]
fn pending_messages_by_urgency_respects_since_window() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = 2_300_000;

    // Old message (3 days ago)
    let old = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "!r1".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Alice".to_string(),
            content: "stale".to_string(),
            person_id: None,
            created_at: now - 3 * 86400,
        })
        .unwrap();
    state
        .ontology_repository
        .update_message_classification(old.id, "medium", "social", None, None, None)
        .unwrap();

    // Recent message (1 hour ago)
    let recent = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "!r2".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Bob".to_string(),
            content: "recent".to_string(),
            person_id: None,
            created_at: now - 3600,
        })
        .unwrap();
    state
        .ontology_repository
        .update_message_classification(recent.id, "medium", "social", None, None, None)
        .unwrap();

    // 24h window: only the recent one
    let pending = state
        .ontology_repository
        .get_pending_messages_by_urgency(user.id, &["medium"], now - 86400, 100)
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, recent.id);
}

// =============================================================================
// get_events_due_on_local_day
// =============================================================================

#[test]
#[serial]
fn events_due_on_local_day_filters_by_window() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Define a fake "today" window: 2026-04-07 00:00 UTC -> 2026-04-08 00:00 UTC
    let day_start: i32 = 1_775_865_600; // 2026-04-07 00:00:00 UTC
    let day_end: i32 = day_start + 86400;

    // Event due today at noon
    state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Lunch with Sam".to_string(),
            remind_at: None,
            due_at: Some(day_start + 12 * 3600),
            status: "active".to_string(),
            created_at: day_start,
            updated_at: day_start,
        })
        .unwrap();

    // Event due yesterday
    state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Yesterday thing".to_string(),
            remind_at: None,
            due_at: Some(day_start - 3600),
            status: "active".to_string(),
            created_at: day_start - 86400,
            updated_at: day_start - 86400,
        })
        .unwrap();

    // Event due tomorrow
    state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Tomorrow thing".to_string(),
            remind_at: None,
            due_at: Some(day_end + 3600),
            status: "active".to_string(),
            created_at: day_start,
            updated_at: day_start,
        })
        .unwrap();

    // Event due today but already done — should be excluded
    state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Already done".to_string(),
            remind_at: None,
            due_at: Some(day_start + 9 * 3600),
            status: "completed".to_string(),
            created_at: day_start,
            updated_at: day_start,
        })
        .unwrap();

    let due_today = state
        .ontology_repository
        .get_events_due_on_local_day(user.id, day_start, day_end)
        .unwrap();

    assert_eq!(due_today.len(), 1);
    assert_eq!(due_today[0].description, "Lunch with Sam");
}

#[test]
#[serial]
fn events_due_on_local_day_excludes_events_with_no_due_date() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let day_start: i32 = 1_775_865_600;
    let day_end: i32 = day_start + 86400;

    // Event with no due_at - should be excluded
    state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "No deadline".to_string(),
            remind_at: None,
            due_at: None,
            status: "active".to_string(),
            created_at: day_start,
            updated_at: day_start,
        })
        .unwrap();

    let due_today = state
        .ontology_repository
        .get_events_due_on_local_day(user.id, day_start, day_end)
        .unwrap();
    assert!(due_today.is_empty());
}

// =============================================================================
// build_digest_for_user — full end-to-end format check
// =============================================================================

/// Helper: insert a classified message into the ontology in one shot.
#[allow(clippy::too_many_arguments)]
fn insert_classified_message(
    state: &std::sync::Arc<backend::AppState>,
    user_id: i32,
    room_id: &str,
    sender: &str,
    content: &str,
    urgency: &str,
    category: &str,
    summary: &str,
    created_at: i32,
) -> i64 {
    let msg = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id,
            room_id: room_id.to_string(),
            platform: "whatsapp".to_string(),
            sender_name: sender.to_string(),
            content: content.to_string(),
            person_id: None,
            created_at,
        })
        .unwrap();
    state
        .ontology_repository
        .update_message_classification(msg.id, urgency, category, Some(summary), None, None)
        .unwrap();
    msg.id
}

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn full_digest_renders_all_sections() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Set the user up: critical pushes OFF (so digest acts as fallback for crit+high),
    // digests enabled.
    state
        .user_core
        .update_critical_enabled(user.id, Some("off".to_string()))
        .unwrap();
    state
        .user_core
        .update_digest_enabled(user.id, true)
        .unwrap();

    // Use a fixed reference "now" so the test is deterministic.
    let now: i32 = 1_775_894_400; // 2026-04-07 08:00:00 UTC
    let tz_offset_secs: i32 = 0;

    // -- Today's events: one with a time, one without. One yesterday (excluded). --
    state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Doctor appointment".to_string(),
            remind_at: None,
            due_at: Some(now + 7 * 3600), // 15:00 today
            status: "active".to_string(),
            created_at: now - 86400,
            updated_at: now - 86400,
        })
        .unwrap();
    state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Buy milk".to_string(),
            remind_at: None,
            due_at: Some(now + 4 * 3600), // 12:00 today
            status: "active".to_string(),
            created_at: now - 86400,
            updated_at: now - 86400,
        })
        .unwrap();

    // -- Critical: 1 message (will appear because critical_enabled = "off") --
    insert_classified_message(
        &state,
        user.id,
        "!room_mom",
        "Mom",
        "Are you ok? Heart racing again",
        "critical",
        "health",
        "Mom is feeling unwell",
        now - 1800,
    );

    // -- Important (high): 2 messages --
    insert_classified_message(
        &state,
        user.id,
        "!room_boss",
        "Boss",
        "Need PRD review by EOD",
        "high",
        "work",
        "Boss needs PRD review by end of day",
        now - 3600,
    );
    insert_classified_message(
        &state,
        user.id,
        "!room_bank",
        "Bank",
        "Payment of $1200 declined",
        "high",
        "financial",
        "Bank reports payment declined",
        now - 7200,
    );

    // -- FYI (medium): 4 messages spanning categories incl spam --
    insert_classified_message(
        &state,
        user.id,
        "!room_sarah",
        "Sarah",
        "Project update",
        "medium",
        "work",
        "Sarah pushed an update on the project",
        now - 5400,
    );
    insert_classified_message(
        &state,
        user.id,
        "!room_john",
        "John",
        "lol that meme",
        "medium",
        "social",
        "John shared a meme",
        now - 600,
    );
    insert_classified_message(
        &state,
        user.id,
        "!room_newsletter",
        "Newsletter",
        "Your weekly digest",
        "medium",
        "logistics",
        "Weekly newsletter arrived",
        now - 9000,
    );
    insert_classified_message(
        &state,
        user.id,
        "!room_spam",
        "Spammer",
        "WIN A FREE IPHONE NOW!!!",
        "medium",
        "spam",
        "Spam message about free iPhone",
        now - 200,
    );

    // -- Recently added tracked item (last 6h) --
    state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Buy birthday gift for Mike".to_string(),
            remind_at: None,
            due_at: Some(now + 3 * 86400),
            status: "active".to_string(),
            created_at: now - 1800,
            updated_at: now - 1800,
        })
        .unwrap();

    // -- Recently completed event (last 6h) --
    // Insert directly as completed so we can control updated_at; the regular
    // update_event_status helper uses real wall-clock time which would be far
    // outside our fixed-future test window.
    state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Reply to John about Friday".to_string(),
            remind_at: None,
            due_at: None,
            status: "completed".to_string(),
            created_at: now - 7200,
            updated_at: now - 1800,
        })
        .unwrap();

    // Build the digest
    let settings = state.user_core.get_user_settings(user.id).unwrap();
    let result = build_digest_for_user(&state, user.id, &settings, now, tz_offset_secs).await;

    let (digest_text, message_ids) = result.expect("digest should be built");

    // Print so the human can eyeball the layout when running with --nocapture
    println!(
        "\n=== DIGEST OUTPUT ({} chars, {} segments) ===\n{}\n=== END DIGEST ===\n",
        digest_text.len(),
        digest_text.len().div_ceil(160),
        digest_text
    );
    println!("message_ids included: {:?}", message_ids);

    // Section ordering: header → Today → Critical → Important → FYI → New → Done → Reply
    let today_pos = digest_text.find("Today:").expect("Today section missing");
    let crit_pos = digest_text
        .find("Critical:")
        .expect("Critical section missing");
    let imp_pos = digest_text
        .find("Important:")
        .expect("Important section missing");
    let fyi_pos = digest_text.find("FYI (").expect("FYI section missing");
    let new_pos = digest_text.find("New:").expect("New section missing");
    let done_pos = digest_text.find("Done:").expect("Done section missing");
    let cta_pos = digest_text
        .find("Reply to dig in")
        .expect("CTA footer missing");

    assert!(today_pos < crit_pos);
    assert!(crit_pos < imp_pos);
    assert!(imp_pos < fyi_pos);
    assert!(fyi_pos < new_pos);
    assert!(new_pos < done_pos);
    assert!(done_pos < cta_pos);

    // Today's events present (inline format with times)
    assert!(digest_text.contains("Doctor appointment"));
    assert!(digest_text.contains("Buy milk"));
    assert!(digest_text.contains("12:00") && digest_text.contains("15:00"));

    // Critical message present (sender + teaser)
    assert!(digest_text.contains("- Mom:"));

    // Important: both senders, sorted by category score (financial before work)
    assert!(digest_text.contains("- Bank:"));
    assert!(digest_text.contains("- Boss:"));
    let bank_pos = digest_text.find("- Bank:").unwrap();
    let boss_pos = digest_text.find("- Boss:").unwrap();
    assert!(
        bank_pos < boss_pos,
        "financial should rank above work in same urgency tier"
    );

    // Spam excluded
    assert!(!digest_text.contains("WIN A FREE IPHONE"));
    assert!(!digest_text.contains("Spammer"));

    // FYI is collapsed to a single sender list
    assert!(digest_text.contains("Sarah"));
    assert!(digest_text.contains("John"));
    assert!(digest_text.contains("Newsletter"));
    // FYI has the count format "FYI (3 msgs):"
    assert!(digest_text.contains("FYI (3 msgs)"));

    // Recently added + Just done are inline single-line lists
    assert!(digest_text.contains("Buy birthday gift"));
    assert!(digest_text.contains("Reply to John about Friday"));

    // Header counts: 6 messages + 2 today
    assert!(
        digest_text.contains("6 msgs") && digest_text.contains("2 events today"),
        "header should mention 6 msgs and 2 events today, got: {}",
        digest_text.lines().next().unwrap_or("")
    );

    // The full set of msg IDs (6) gets marked delivered, even if some overflowed
    // the visible count, so they don't reappear next cycle.
    assert_eq!(message_ids.len(), 6);

    // Sanity: total length should comfortably fit in 4 segments (640 chars)
    assert!(
        digest_text.len() < 640,
        "digest should fit in 4 segments, got {} chars",
        digest_text.len()
    );
}

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn digest_skips_critical_section_when_pushes_enabled() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // critical_enabled = "sms" (default-ish; pushes are ON)
    state
        .user_core
        .update_critical_enabled(user.id, Some("sms".to_string()))
        .unwrap();
    state
        .user_core
        .update_digest_enabled(user.id, true)
        .unwrap();

    let now: i32 = 1_775_894_400;
    let tz_offset_secs: i32 = 0;

    // Insert one of each urgency
    insert_classified_message(
        &state,
        user.id,
        "!room1",
        "Mom",
        "emergency",
        "critical",
        "emergency",
        "Mom emergency",
        now - 600,
    );
    insert_classified_message(
        &state,
        user.id,
        "!room2",
        "Boss",
        "urgent",
        "high",
        "work",
        "Boss urgent",
        now - 600,
    );
    insert_classified_message(
        &state,
        user.id,
        "!room3",
        "Friend",
        "fyi",
        "medium",
        "social",
        "Friend fyi",
        now - 600,
    );

    let settings = state.user_core.get_user_settings(user.id).unwrap();
    let (digest_text, _) = build_digest_for_user(&state, user.id, &settings, now, tz_offset_secs)
        .await
        .expect("digest");

    println!(
        "\n=== DIGEST (critical_enabled=sms, {} chars) ===\n{}\n=== END ===\n",
        digest_text.len(),
        digest_text
    );

    // No Critical or Important section - those went via instant SMS
    assert!(!digest_text.contains("Critical:"));
    assert!(!digest_text.contains("Important:"));
    // FYI section still present (collapsed list format)
    assert!(digest_text.contains("FYI ("));
    assert!(digest_text.contains("Friend"));
    // Mom and Boss were filtered (their content shouldn't appear)
    assert!(!digest_text.contains("- Mom"));
    assert!(!digest_text.contains("- Boss"));
}

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn digest_stays_compact_on_busy_day() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    state
        .user_core
        .update_critical_enabled(user.id, Some("off".to_string()))
        .unwrap();
    state
        .user_core
        .update_digest_enabled(user.id, true)
        .unwrap();

    let now: i32 = 1_775_894_400;

    // 5 critical, 8 important, 25 FYI from 12 distinct senders → naturally
    // capped by the format itself rather than truncation.
    for i in 0..5 {
        insert_classified_message(
            &state,
            user.id,
            &format!("!crit{}", i),
            &format!("CritSender{}", i),
            "x",
            "critical",
            "emergency",
            &format!("crit thing {}", i),
            now - 600 - i,
        );
    }
    for i in 0..8 {
        insert_classified_message(
            &state,
            user.id,
            &format!("!imp{}", i),
            &format!("ImpSender{}", i),
            "x",
            "high",
            if i % 2 == 0 { "financial" } else { "work" },
            &format!("important thing {}", i),
            now - 600 - i,
        );
    }
    let names = [
        "Alice", "Bob", "Carol", "Dave", "Eve", "Frank", "Grace", "Henry", "Ivy", "Jack", "Kate",
        "Liam",
    ];
    for i in 0..25 {
        let sender = names[i % names.len()];
        insert_classified_message(
            &state,
            user.id,
            &format!("!fyi{}", i),
            sender,
            "x",
            "medium",
            "social",
            &format!("fyi {}", i),
            now - 600 - i as i32,
        );
    }

    let settings = state.user_core.get_user_settings(user.id).unwrap();
    let (digest_text, message_ids) = build_digest_for_user(&state, user.id, &settings, now, 0)
        .await
        .expect("digest");

    println!(
        "\n=== BUSY DIGEST ({} chars, {} segments) ===\n{}\n=== END ===\n",
        digest_text.len(),
        digest_text.len().div_ceil(160),
        digest_text
    );

    // Critical caps at 3 with overflow indicator
    assert!(digest_text.contains("Critical:"));
    assert!(
        digest_text.contains("+ 2 more"),
        "5 crit → 3 shown + '+2 more'"
    );

    // Important caps at 3 with overflow indicator
    assert!(digest_text.contains("Important:"));
    assert!(
        digest_text.contains("+ 5 more"),
        "8 imp → 3 shown + '+5 more'"
    );

    // FYI shows count + sender list (no per-item lines)
    assert!(digest_text.contains("FYI (25 msgs)"));
    // Sender list capped at 5 names + "+N more"
    assert!(
        digest_text.contains("+7 more"),
        "12 unique senders → 5 shown + '+7 more'"
    );

    // Stays comfortably under 4 segments even with 38 total messages
    assert!(
        digest_text.len() < 640,
        "busy digest should still fit in 4 segments, got {} chars",
        digest_text.len()
    );

    // All 38 messages get marked delivered (so they don't loop back)
    assert_eq!(message_ids.len(), 38);
}

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn digest_returns_none_when_everything_is_filtered() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    state
        .user_core
        .update_critical_enabled(user.id, Some("off".to_string()))
        .unwrap();
    state
        .user_core
        .update_digest_enabled(user.id, true)
        .unwrap();

    let now: i32 = 1_775_894_400;

    // Only spam → after spam filter, nothing remains
    insert_classified_message(
        &state,
        user.id,
        "!room_spam",
        "Spammer",
        "BUY NOW",
        "medium",
        "spam",
        "Spam",
        now - 600,
    );

    let settings = state.user_core.get_user_settings(user.id).unwrap();
    let result = build_digest_for_user(&state, user.id, &settings, now, 0).await;
    assert!(
        result.is_none(),
        "digest should be None when only spam exists"
    );
}
