//! Tests for the tasks -> items startup migration.
//!
//! Verifies that each task type is correctly converted into an item
//! with the right summary, kind, due_at, and next_check_at values,
//! and that migrated tasks are marked as "migrated" (idempotency).

use backend::jobs::scheduler::migrate_tasks_to_items;
use backend::test_utils::{
    create_test_state, create_test_task, create_test_user, get_user_items, get_user_tasks,
    set_user_timezone, TestTaskParams, TestUserParams,
};
use backend::UserCoreOps;

// =============================================================================
// One-shot reminder
// =============================================================================

#[tokio::test]
async fn test_migrate_once_task() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let trigger_ts = 1_750_000_000;
    create_test_task(
        &state,
        &TestTaskParams::once_task(user.id, trigger_ts).with_action("Pick up dry cleaning"),
    );

    migrate_tasks_to_items(&state).await;

    let items = get_user_items(&state, user.id);
    assert_eq!(items.len(), 1);
    assert!(items[0].summary.contains("Pick up dry cleaning"));
    assert_eq!(items[0].kind, "reminder");
    assert_eq!(items[0].due_at, Some(trigger_ts));
    assert_eq!(items[0].next_check_at, Some(trigger_ts));

    // Task should be marked as migrated (no longer active)
    let tasks = get_user_tasks(&state, user.id);
    assert_eq!(
        tasks.len(),
        0,
        "Active tasks should be empty after migration"
    );
}

// =============================================================================
// Digest task
// =============================================================================

#[tokio::test]
async fn test_migrate_digest_task() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    // Set timezone so calculate_next_trigger works
    state.user_core.ensure_user_info_exists(user.id).unwrap();
    set_user_timezone(&state, user.id, "UTC");

    create_test_task(&state, &TestTaskParams::digest_task(user.id, "08:00"));

    migrate_tasks_to_items(&state).await;

    let items = get_user_items(&state, user.id);
    assert_eq!(items.len(), 1);
    assert!(items[0].summary.contains("Daily digest"));
    assert!(items[0]
        .summary
        .contains("email,whatsapp,telegram,signal,calendar"));
    assert!(items[0].summary.contains("daily"));
    assert!(items[0].summary.contains("08:00"));
    assert_eq!(items[0].kind, "digest");
    assert!(
        items[0].next_check_at.is_some(),
        "Digest should have next_check_at"
    );
    assert!(items[0].due_at.is_none());
}

// =============================================================================
// Monitor task (recurring_email with condition)
// =============================================================================

#[tokio::test]
async fn test_migrate_monitor_task() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Build a monitor task manually (trigger = "recurring_email")
    let params = TestTaskParams {
        user_id: user.id,
        trigger: "recurring_email".to_string(),
        action: "send_reminder".to_string(),
        notification_type: Some("sms".to_string()),
        is_permanent: None,
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
        condition: Some("invoice from AWS about payment".to_string()),
        end_time: None,
    };
    create_test_task(&state, &params);

    migrate_tasks_to_items(&state).await;

    let items = get_user_items(&state, user.id);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, "monitor");
    assert!(items[0].summary.contains("Monitor:"));
    assert!(items[0].summary.contains("invoice from AWS about payment"));
    assert!(items[0].due_at.is_none());
    assert!(items[0].next_check_at.is_none());
}

#[tokio::test]
async fn test_migrate_messaging_monitor_task() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams {
        user_id: user.id,
        trigger: "recurring_messaging".to_string(),
        action: "send_reminder".to_string(),
        notification_type: Some("sms".to_string()),
        is_permanent: None,
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
        condition: Some("message about project deadline".to_string()),
        end_time: None,
    };
    create_test_task(&state, &params);

    migrate_tasks_to_items(&state).await;

    let items = get_user_items(&state, user.id);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, "monitor");
    assert!(items[0].summary.contains("message about project deadline"));
}

// =============================================================================
// Quiet mode task
// =============================================================================

#[tokio::test]
async fn test_migrate_quiet_mode_task() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let end_ts = 1_750_010_000;
    create_test_task(
        &state,
        &TestTaskParams::quiet_mode_task(user.id, Some(end_ts)),
    );

    migrate_tasks_to_items(&state).await;

    let items = get_user_items(&state, user.id);
    assert_eq!(items.len(), 1);
    assert!(items[0].summary.contains("Quiet mode"));
    assert_eq!(items[0].kind, "reminder");
    assert_eq!(items[0].due_at, Some(end_ts));
    assert_eq!(items[0].next_check_at, Some(end_ts));
}

// =============================================================================
// Recurring non-digest task
// =============================================================================

#[tokio::test]
async fn test_migrate_recurring_task() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    state.user_core.ensure_user_info_exists(user.id).unwrap();
    set_user_timezone(&state, user.id, "UTC");

    create_test_task(
        &state,
        &TestTaskParams::permanent_daily(user.id, "09:00").with_action("Check server health"),
    );

    migrate_tasks_to_items(&state).await;

    let items = get_user_items(&state, user.id);
    assert_eq!(items.len(), 1);
    assert!(items[0].summary.contains("Check server health"));
    assert!(items[0].summary.contains("Repeats"));
    assert!(items[0].summary.contains("daily"));
    assert!(items[0].summary.contains("09:00"));
    assert_eq!(items[0].kind, "reminder");
    assert!(items[0].next_check_at.is_some());
}

// =============================================================================
// Idempotency: running migration twice doesn't duplicate
// =============================================================================

#[tokio::test]
async fn test_migrate_is_idempotent() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    create_test_task(
        &state,
        &TestTaskParams::once_task(user.id, 1_750_000_000).with_action("Idempotency test"),
    );

    // Run migration twice
    migrate_tasks_to_items(&state).await;
    migrate_tasks_to_items(&state).await;

    let items = get_user_items(&state, user.id);
    assert_eq!(
        items.len(),
        1,
        "Second migration should not create duplicates"
    );
}

// =============================================================================
// Multiple task types for same user
// =============================================================================

#[tokio::test]
async fn test_migrate_multiple_task_types() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    state.user_core.ensure_user_info_exists(user.id).unwrap();
    set_user_timezone(&state, user.id, "UTC");

    // One-shot reminder
    create_test_task(
        &state,
        &TestTaskParams::once_task(user.id, 1_750_000_000).with_action("Buy milk"),
    );

    // Digest
    create_test_task(&state, &TestTaskParams::digest_task(user.id, "07:00"));

    // Monitor
    let monitor_params = TestTaskParams {
        user_id: user.id,
        trigger: "recurring_email".to_string(),
        action: "send_reminder".to_string(),
        notification_type: Some("sms".to_string()),
        is_permanent: None,
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
        condition: Some("shipping notification".to_string()),
        end_time: None,
    };
    create_test_task(&state, &monitor_params);

    migrate_tasks_to_items(&state).await;

    let items = get_user_items(&state, user.id);
    assert_eq!(items.len(), 3);

    // Check that we have the right mix of types
    let monitors: Vec<_> = items.iter().filter(|i| i.kind == "monitor").collect();
    assert_eq!(monitors.len(), 1);

    let with_due: Vec<_> = items.iter().filter(|i| i.due_at.is_some()).collect();
    assert_eq!(with_due.len(), 1); // Only the one-shot reminder

    // All original tasks should be migrated
    let tasks = get_user_tasks(&state, user.id);
    assert_eq!(tasks.len(), 0);
}

// =============================================================================
// No tasks = no items created
// =============================================================================

#[tokio::test]
async fn test_migrate_no_tasks_is_noop() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    migrate_tasks_to_items(&state).await;

    let items = get_user_items(&state, user.id);
    assert_eq!(items.len(), 0);
}
