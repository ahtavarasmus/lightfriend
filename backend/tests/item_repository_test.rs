//! Unit tests for ItemRepository.
//!
//! Tests CRUD operations, dedup, triggered items query, tracking items query,
//! dashboard ordering, snooze, complete, reschedule, bulk update, and cleanup.

use backend::models::user_models::NewItem;
use backend::test_utils::{
    create_test_item, create_test_state, create_test_user, get_user_items, TestItemParams,
    TestUserParams,
};

/// Fixed reference timestamp for deterministic tests.
const T: i32 = 1_750_000_000;

// =============================================================================
// Create + Read
// =============================================================================

#[test]
fn test_create_and_read_item() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let item = create_test_item(&state, &TestItemParams::reminder(user.id, "Buy groceries"));

    assert_eq!(item.user_id, user.id);
    assert_eq!(item.summary, "Buy groceries");
    assert_eq!(item.priority, 0);
}

#[test]
fn test_get_item_with_ownership_check() {
    let state = create_test_state();
    let user1 = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let user2 = create_test_user(&state, &TestUserParams::finland_user(10.0, 5.0));

    let item = create_test_item(&state, &TestItemParams::reminder(user1.id, "User1 item"));
    let item_id = item.id.unwrap();

    // Owner can access
    let found = state.item_repository.get_item(item_id, user1.id).unwrap();
    assert!(found.is_some());

    // Non-owner cannot access
    let not_found = state.item_repository.get_item(item_id, user2.id).unwrap();
    assert!(not_found.is_none());
}

// =============================================================================
// Dedup
// =============================================================================

#[test]
fn test_item_exists_by_source() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    create_test_item(
        &state,
        &TestItemParams::from_email(user.id, "Invoice from AWS", "email_123"),
    );

    assert!(state
        .item_repository
        .item_exists_by_source(user.id, "email_123")
        .unwrap());
    assert!(!state
        .item_repository
        .item_exists_by_source(user.id, "email_999")
        .unwrap());
}

// =============================================================================
// Triggered Items Query (due_at)
// =============================================================================

#[test]
fn test_get_triggered_items() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Item in the past - should trigger
    create_test_item(
        &state,
        &TestItemParams::scheduled_reminder(user.id, "Past reminder", T - 60),
    );

    // Item in the future - should NOT trigger
    create_test_item(
        &state,
        &TestItemParams::scheduled_reminder(user.id, "Future reminder", T + 3600),
    );

    // Item with no due_at - should NOT trigger
    create_test_item(&state, &TestItemParams::reminder(user.id, "No schedule"));

    let triggered = state.item_repository.get_triggered_items(T).unwrap();
    assert_eq!(triggered.len(), 1);
    assert_eq!(triggered[0].summary, "Past reminder");
}

// =============================================================================
// Tracking Items Query ([type:tracking] in summary)
// =============================================================================

#[test]
fn test_get_tracking_items() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Tracking item (has [type:tracking] tag)
    create_test_item(
        &state,
        &TestItemParams::tracking(user.id, "Watch for AWS invoices"),
    );

    // Non-tracking item
    create_test_item(
        &state,
        &TestItemParams::reminder(user.id, "Just a reminder"),
    );

    let tracking = state.item_repository.get_tracking_items(user.id).unwrap();
    assert_eq!(tracking.len(), 1);
    assert!(tracking[0].summary.contains("Watch for AWS invoices"));
    assert!(tracking[0].summary.contains("[type:tracking]"));
}

// =============================================================================
// Dashboard Items Query (ordering)
// =============================================================================

#[test]
fn test_dashboard_items_ordered_by_priority() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    create_test_item(
        &state,
        &TestItemParams::reminder(user.id, "Low priority").with_priority(0),
    );
    create_test_item(
        &state,
        &TestItemParams::reminder(user.id, "High priority").with_priority(2),
    );
    create_test_item(
        &state,
        &TestItemParams::reminder(user.id, "Medium priority").with_priority(1),
    );

    let items = state.item_repository.get_dashboard_items(user.id).unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].summary, "High priority");
    assert_eq!(items[1].summary, "Medium priority");
    assert_eq!(items[2].summary, "Low priority");
}

// =============================================================================
// Snooze (update due_at to future)
// =============================================================================

#[test]
fn test_snooze_item() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let item = create_test_item(
        &state,
        &TestItemParams::scheduled_reminder(user.id, "Snooze me", T - 60),
    );
    let item_id = item.id.unwrap();

    // Item should currently trigger
    let triggered = state.item_repository.get_triggered_items(T).unwrap();
    assert_eq!(triggered.len(), 1);

    // Snooze to future
    let updated = state
        .item_repository
        .update_due_at(item_id, Some(T + 3600))
        .unwrap();
    assert!(updated);

    // Should no longer trigger
    let triggered = state.item_repository.get_triggered_items(T).unwrap();
    assert_eq!(triggered.len(), 0);

    // Should trigger in the future
    let triggered = state.item_repository.get_triggered_items(T + 7200).unwrap();
    assert_eq!(triggered.len(), 1);
}

// =============================================================================
// Complete (delete)
// =============================================================================

#[test]
fn test_complete_item_deletes_it() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let item = create_test_item(&state, &TestItemParams::reminder(user.id, "Complete me"));
    let item_id = item.id.unwrap();

    // Item exists
    assert!(state
        .item_repository
        .get_item(item_id, user.id)
        .unwrap()
        .is_some());

    // Delete
    let deleted = state.item_repository.delete_item(item_id, user.id).unwrap();
    assert!(deleted);

    // Item is gone
    assert!(state
        .item_repository
        .get_item(item_id, user.id)
        .unwrap()
        .is_none());
}

#[test]
fn test_delete_item_requires_ownership() {
    let state = create_test_state();
    let user1 = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let user2 = create_test_user(&state, &TestUserParams::finland_user(10.0, 5.0));

    let item = create_test_item(
        &state,
        &TestItemParams::reminder(user1.id, "Protected item"),
    );
    let item_id = item.id.unwrap();

    // User2 cannot delete user1's item
    let deleted = state
        .item_repository
        .delete_item(item_id, user2.id)
        .unwrap();
    assert!(!deleted);

    // Item still exists for user1
    assert!(state
        .item_repository
        .get_item(item_id, user1.id)
        .unwrap()
        .is_some());
}

// =============================================================================
// Reschedule (update due_at for recurring)
// =============================================================================

#[test]
fn test_reschedule_recurring_item() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let item = create_test_item(
        &state,
        &TestItemParams::scheduled_reminder(user.id, "Daily digest at 08:00", T - 60),
    );
    let item_id = item.id.unwrap();

    // Fires now
    let triggered = state.item_repository.get_triggered_items(T).unwrap();
    assert_eq!(triggered.len(), 1);

    // Reschedule to tomorrow
    let tomorrow = T + 86400;
    state
        .item_repository
        .update_due_at(item_id, Some(tomorrow))
        .unwrap();

    // No longer fires now
    let triggered = state.item_repository.get_triggered_items(T).unwrap();
    assert_eq!(triggered.len(), 0);

    // Fires tomorrow
    let triggered = state
        .item_repository
        .get_triggered_items(tomorrow + 1)
        .unwrap();
    assert_eq!(triggered.len(), 1);
}

// =============================================================================
// Bulk Update (update_item)
// =============================================================================

#[test]
fn test_update_item_bulk() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let item = create_test_item(
        &state,
        &TestItemParams::reminder(user.id, "Original summary"),
    );
    let item_id = item.id.unwrap();

    let updated = state
        .item_repository
        .update_item(
            item_id,
            user.id,
            "Updated summary with context",
            Some(T + 3600),
            2,
        )
        .unwrap();
    assert!(updated);

    let item = state
        .item_repository
        .get_item(item_id, user.id)
        .unwrap()
        .unwrap();
    assert_eq!(item.summary, "Updated summary with context");
    assert_eq!(item.due_at, Some(T + 3600));
    assert_eq!(item.priority, 2);
}

// =============================================================================
// Cleanup Old Items
// =============================================================================

#[test]
fn test_delete_old_items() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Create old item directly (with old created_at)
    let old_item = NewItem {
        user_id: user.id,
        summary: "Old item".to_string(),
        due_at: None,
        priority: 0,
        source_id: None,
        created_at: T - 86400 * 30, // 30 days ago
    };
    state.item_repository.create_item(&old_item).unwrap();

    // Create recent item
    create_test_item(&state, &TestItemParams::reminder(user.id, "Recent item"));

    let all_items = get_user_items(&state, user.id);
    assert_eq!(all_items.len(), 2);

    // Delete items older than 7 days
    let deleted = state
        .item_repository
        .delete_old_items(T - 86400 * 7)
        .unwrap();
    assert_eq!(deleted, 1);

    let remaining = get_user_items(&state, user.id);
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].summary, "Recent item");
}

// =============================================================================
// Delete Items By Source
// =============================================================================

#[test]
fn test_delete_items_by_source() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    create_test_item(
        &state,
        &TestItemParams::from_email(user.id, "Email item 1", "room_abc"),
    );
    create_test_item(
        &state,
        &TestItemParams::from_email(user.id, "Email item 2", "room_abc"),
    );
    create_test_item(
        &state,
        &TestItemParams::from_email(user.id, "Different source", "room_xyz"),
    );

    let deleted = state
        .item_repository
        .delete_items_by_source(user.id, "room_abc")
        .unwrap();
    assert_eq!(deleted, 2);

    let remaining = get_user_items(&state, user.id);
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].summary, "Different source");
}

// =============================================================================
// Update Individual Fields
// =============================================================================

#[test]
fn test_update_summary() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let item = create_test_item(&state, &TestItemParams::reminder(user.id, "Original"));
    let item_id = item.id.unwrap();

    state
        .item_repository
        .update_summary(item_id, "Updated with new context")
        .unwrap();

    let item = state
        .item_repository
        .get_item(item_id, user.id)
        .unwrap()
        .unwrap();
    assert_eq!(item.summary, "Updated with new context");
}

#[test]
fn test_update_priority() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let item = create_test_item(&state, &TestItemParams::reminder(user.id, "Escalate me"));
    let item_id = item.id.unwrap();

    state.item_repository.update_priority(item_id, 2).unwrap();

    let item = state
        .item_repository
        .get_item(item_id, user.id)
        .unwrap()
        .unwrap();
    assert_eq!(item.priority, 2);
}

// =============================================================================
// Monitor Lifecycle Tests
// =============================================================================

#[test]
fn test_item_limit_enforcement() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Create 100 items (the limit)
    for i in 0..100 {
        create_test_item(
            &state,
            &TestItemParams::reminder(user.id, &format!("Item {}", i)),
        );
    }

    // 101st should fail
    let result = state.item_repository.create_item(&NewItem {
        user_id: user.id,
        summary: "Item 101".to_string(),
        due_at: None,
        priority: 0,
        source_id: None,
        created_at: T,
    });
    assert!(result.is_err());
}

#[test]
fn test_tracking_with_due_at_triggers() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Tracking item with due_at in the past should appear in triggered items
    create_test_item(
        &state,
        &TestItemParams::tracking(user.id, "Watch for invoices").with_due_at(T - 60),
    );

    // Tracking without due_at should NOT trigger
    create_test_item(
        &state,
        &TestItemParams::tracking(user.id, "Watch for messages"),
    );

    let triggered = state.item_repository.get_triggered_items(T).unwrap();
    assert_eq!(triggered.len(), 1);
    assert!(triggered[0].summary.contains("Watch for invoices"));
    assert!(triggered[0].summary.contains("[type:tracking]"));
}

#[test]
fn test_tracking_survives_update() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(user.id, "Watch for package delivery"),
    );
    let item_id = item.id.unwrap();

    // Update via update_item
    state
        .item_repository
        .update_item(
            item_id,
            user.id,
            "[type:tracking] Package shipped - tracking #12345",
            Some(T + 2 * 86400),
            1,
        )
        .unwrap();

    // Item still exists with updated fields
    let updated = state
        .item_repository
        .get_item(item_id, user.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        updated.summary,
        "[type:tracking] Package shipped - tracking #12345"
    );
    assert_eq!(updated.due_at, Some(T + 2 * 86400));
    assert_eq!(updated.priority, 1);

    // Still shows as a tracking item
    let tracking = state.item_repository.get_tracking_items(user.id).unwrap();
    assert_eq!(tracking.len(), 1);
    assert_eq!(tracking[0].id, Some(item_id));
}

#[test]
fn test_tracking_escalation() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(user.id, "Invoice $500 due Feb 28").with_priority(0),
    );
    let item_id = item.id.unwrap();

    // Escalate priority 0 -> 2
    state
        .item_repository
        .update_item(
            item_id,
            user.id,
            "Invoice $500 OVERDUE since Feb 28",
            Some(T + 86400),
            2,
        )
        .unwrap();

    let updated = state
        .item_repository
        .get_item(item_id, user.id)
        .unwrap()
        .unwrap();
    assert_eq!(updated.priority, 2);
}

#[test]
fn test_tracking_resolution_deletes() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let item = create_test_item(
        &state,
        &TestItemParams::tracking(user.id, "Watch for payment confirmation").with_due_at(T - 60),
    );
    let item_id = item.id.unwrap();

    // Should appear in tracking items and triggered
    assert_eq!(
        state
            .item_repository
            .get_tracking_items(user.id)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        state.item_repository.get_triggered_items(T).unwrap().len(),
        1
    );

    // Resolve (delete)
    state.item_repository.delete_item(item_id, user.id).unwrap();

    // Gone from both queries
    assert_eq!(
        state
            .item_repository
            .get_tracking_items(user.id)
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        state.item_repository.get_triggered_items(T).unwrap().len(),
        0
    );
}

#[test]
fn test_multi_user_tracking_isolation() {
    let state = create_test_state();
    let user_a = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let user_b = create_test_user(&state, &TestUserParams::finland_user(10.0, 5.0));

    create_test_item(
        &state,
        &TestItemParams::tracking(user_a.id, "User A tracking"),
    );
    create_test_item(
        &state,
        &TestItemParams::tracking(user_b.id, "User B tracking"),
    );

    // Each user only sees their own tracking items
    let a_tracking = state.item_repository.get_tracking_items(user_a.id).unwrap();
    let b_tracking = state.item_repository.get_tracking_items(user_b.id).unwrap();
    assert_eq!(a_tracking.len(), 1);
    assert!(a_tracking[0].summary.contains("User A tracking"));
    assert_eq!(b_tracking.len(), 1);
    assert!(b_tracking[0].summary.contains("User B tracking"));
}

#[test]
fn test_multi_user_triggered_isolation() {
    let state = create_test_state();
    let user_a = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let user_b = create_test_user(&state, &TestUserParams::finland_user(10.0, 5.0));

    create_test_item(
        &state,
        &TestItemParams::scheduled_reminder(user_a.id, "User A trigger", T - 60),
    );
    create_test_item(
        &state,
        &TestItemParams::scheduled_reminder(user_b.id, "User B trigger", T - 60),
    );

    // get_triggered_items returns ALL users' items (global query)
    // but each item has user_id for isolation in processing
    let triggered = state.item_repository.get_triggered_items(T).unwrap();
    assert_eq!(triggered.len(), 2);

    // Verify each item belongs to the correct user
    let a_items: Vec<_> = triggered
        .iter()
        .filter(|i| i.user_id == user_a.id)
        .collect();
    let b_items: Vec<_> = triggered
        .iter()
        .filter(|i| i.user_id == user_b.id)
        .collect();
    assert_eq!(a_items.len(), 1);
    assert_eq!(b_items.len(), 1);
    assert_eq!(a_items[0].summary, "User A trigger");
    assert_eq!(b_items[0].summary, "User B trigger");
}

#[test]
fn test_mixed_items_dashboard() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Create monitors and non-monitors with different priorities
    create_test_item(
        &state,
        &TestItemParams::tracking(user.id, "Background tracking").with_priority(0),
    );
    create_test_item(
        &state,
        &TestItemParams::reminder(user.id, "Important reminder").with_priority(1),
    );
    create_test_item(
        &state,
        &TestItemParams::alert(user.id, "System alert").with_priority(1),
    );
    create_test_item(
        &state,
        &TestItemParams::digest(user.id, "Daily digest", T + 3600).with_priority(0),
    );

    // Dashboard shows all items sorted by priority desc
    let items = state.item_repository.get_dashboard_items(user.id).unwrap();
    assert_eq!(items.len(), 4);

    // Priority 1 items first
    assert!(items[0].priority >= items[1].priority);
    assert!(items[1].priority >= items[2].priority);
    assert!(items[2].priority >= items[3].priority);

    // One tracking item and three non-tracking
    let tracking_count = items
        .iter()
        .filter(|i| i.summary.contains("[type:tracking]"))
        .count();
    let non_tracking_count = items
        .iter()
        .filter(|i| !i.summary.contains("[type:tracking]"))
        .count();
    assert_eq!(tracking_count, 1);
    assert_eq!(non_tracking_count, 3);
}

#[test]
fn test_stale_tracking_cleanup() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Stale tracking: due_at >7 days ago
    let stale = NewItem {
        user_id: user.id,
        summary: "[type:tracking] Old invoice".to_string(),
        due_at: Some(T - 8 * 86400), // 8 days ago
        priority: 0,
        source_id: None,
        created_at: T - 10 * 86400,
    };
    state.item_repository.create_item(&stale).unwrap();

    // Active tracking: due_at in the future
    let active = NewItem {
        user_id: user.id,
        summary: "[type:tracking] Active tracking".to_string(),
        due_at: Some(T + 86400), // Future deadline
        priority: 1,
        source_id: None,
        created_at: T - 10 * 86400,
    };
    state.item_repository.create_item(&active).unwrap();

    // Tracking without due_at (should not be cleaned up)
    create_test_item(
        &state,
        &TestItemParams::tracking(user.id, "Match-only tracking"),
    );

    assert_eq!(get_user_items(&state, user.id).len(), 3);

    // Cleanup should only remove the stale one
    let deleted = state
        .item_repository
        .delete_expired_tracking_items(T)
        .unwrap();
    assert_eq!(deleted, 1);

    let remaining = get_user_items(&state, user.id);
    assert_eq!(remaining.len(), 2);
    let summaries: Vec<&str> = remaining.iter().map(|i| i.summary.as_str()).collect();
    assert!(summaries.iter().any(|s| s.contains("Active tracking")));
    assert!(summaries.iter().any(|s| s.contains("Match-only tracking")));
}
