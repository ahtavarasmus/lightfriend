//! Tests for task-to-item migration logic.
//!
//! Covers:
//! - Pure summary-building functions (unit tests)
//! - Tag parsing roundtrips (verify migrated summaries are parseable)
//! - Source mapping
//! - Full migration integration tests (using real DB)

use backend::jobs::scheduler::{
    build_digest_migration_summary, build_digest_task_summary, build_monitor_task_summary,
    build_oneshot_task_summary, build_quiet_mode_summary, build_recurring_task_summary,
    map_sources_to_fetch, weekday_number_to_name,
};
use backend::proactive::utils::parse_summary_tags;

// =============================================================================
// Unit tests: summary building
// =============================================================================

#[test]
fn test_digest_task_produces_tagged_summary() {
    let (summary, _monitor, priority) =
        build_digest_task_summary(Some("sms"), Some("09:00"), Some("email,whatsapp,calendar"));

    assert!(summary.starts_with("[type:recurring]"));
    assert!(summary.contains("[notify:sms]"));
    assert!(summary.contains("[repeat:daily 09:00]"));
    assert!(summary.contains("[fetch:"));
    assert_eq!(priority, 1);
    // Description on second line
    assert!(summary.contains('\n'));
}

#[test]
fn test_digest_task_call_notification() {
    let (summary, _, priority) = build_digest_task_summary(Some("call"), Some("08:00"), None);

    assert!(summary.contains("[notify:call]"));
    assert_eq!(priority, 2);
}

#[test]
fn test_digest_task_defaults() {
    // No notification type, no time, no sources - should use defaults
    let (summary, monitor, priority) = build_digest_task_summary(None, None, None);

    assert!(summary.contains("[notify:sms]"));
    assert!(summary.contains("[repeat:daily 08:00]"));
    assert!(summary.contains("[fetch:email,chat,calendar,items]"));
    assert!(!monitor);
    assert_eq!(priority, 1);
}

#[test]
fn test_monitor_task_produces_tagged_summary() {
    let (summary, monitor, priority) = build_monitor_task_summary(
        "recurring_email_check",
        Some("package delivery updates"),
        Some("sms"),
    );

    assert!(summary.contains("[type:tracking]"));
    assert!(summary.contains("[notify:sms]"));
    assert!(summary.contains("[platform:email]"));
    assert!(summary.contains("[sender:any]"));
    assert!(summary.contains("[topic:package delivery updates]"));
    assert!(monitor);
    assert_eq!(priority, 1);
}

#[test]
fn test_monitor_task_messaging_platform() {
    let (summary, monitor, _) =
        build_monitor_task_summary("recurring_messaging_check", Some("stock alerts"), None);

    assert!(summary.contains("[platform:chat]"));
    assert!(monitor);
}

#[test]
fn test_recurring_reminder_produces_tagged_summary() {
    let items =
        build_recurring_task_summary("Take medication", Some("daily"), Some("09:00"), Some("sms"));

    assert_eq!(items.len(), 1);
    let (summary, monitor, priority) = &items[0];
    assert!(summary.contains("[type:recurring]"));
    assert!(summary.contains("[notify:sms]"));
    assert!(summary.contains("[repeat:daily 09:00]"));
    assert!(summary.contains("Take medication"));
    assert!(!monitor);
    assert_eq!(*priority, 1);
}

#[test]
fn test_recurring_weekly_single_day() {
    let items = build_recurring_task_summary(
        "Weekly review",
        Some("weekly:1"),
        Some("10:00"),
        Some("sms"),
    );

    assert_eq!(items.len(), 1);
    let (summary, _, _) = &items[0];
    assert!(summary.contains("[repeat:weekly Monday 10:00]"));
}

#[test]
fn test_recurring_weekly_multiple_days() {
    let items =
        build_recurring_task_summary("Exercise", Some("weekly:1,3,5"), Some("07:00"), Some("sms"));

    assert_eq!(items.len(), 3);
    assert!(items[0].0.contains("[repeat:weekly Monday 07:00]"));
    assert!(items[1].0.contains("[repeat:weekly Wednesday 07:00]"));
    assert!(items[2].0.contains("[repeat:weekly Friday 07:00]"));
}

#[test]
fn test_recurring_weekdays_shortcut() {
    // Mon-Fri (1,2,3,4,5) should collapse to "weekdays"
    let items = build_recurring_task_summary(
        "Stand up",
        Some("weekly:1,2,3,4,5"),
        Some("09:00"),
        Some("sms"),
    );

    assert_eq!(items.len(), 1);
    assert!(items[0].0.contains("[repeat:weekdays 09:00]"));
}

#[test]
fn test_oneshot_reminder_produces_tagged_summary() {
    let (summary, monitor, priority) = build_oneshot_task_summary("Call the dentist", Some("sms"));

    assert!(summary.contains("[type:oneshot]"));
    assert!(summary.contains("[notify:sms]"));
    assert!(summary.contains("Call the dentist"));
    assert!(!monitor);
    assert_eq!(priority, 1);
}

#[test]
fn test_oneshot_call_notification() {
    let (summary, _, priority) = build_oneshot_task_summary("Urgent meeting", Some("call"));

    assert!(summary.contains("[notify:call]"));
    assert_eq!(priority, 2);
}

#[test]
fn test_quiet_mode_produces_tagged_summary() {
    let (summary, monitor, priority) = build_quiet_mode_summary();

    assert!(summary.contains("[type:oneshot]"));
    assert!(summary.contains("[notify:silent]"));
    assert!(summary.contains("Quiet mode"));
    assert!(!monitor);
    assert_eq!(priority, 0);
}

// =============================================================================
// Tag parsing roundtrip tests
// =============================================================================

#[test]
fn test_migrated_digest_summary_parseable() {
    let (summary, _, _) = build_digest_task_summary(
        Some("sms"),
        Some("09:00"),
        Some("email,whatsapp,telegram,calendar"),
    );

    let tags = parse_summary_tags(&summary);
    assert!(tags.has_tags);
    assert_eq!(tags.item_type.as_deref(), Some("recurring"));
    assert_eq!(tags.notify.as_deref(), Some("sms"));
    assert_eq!(tags.repeat.as_deref(), Some("daily 09:00"));
    assert!(!tags.fetch.is_empty());
}

#[test]
fn test_migrated_monitor_summary_parseable() {
    let (summary, _, _) = build_monitor_task_summary(
        "recurring_email_check",
        Some("shipping notifications"),
        Some("sms"),
    );

    let tags = parse_summary_tags(&summary);
    assert!(tags.has_tags);
    assert_eq!(tags.item_type.as_deref(), Some("tracking"));
    assert_eq!(tags.notify.as_deref(), Some("sms"));
    assert_eq!(tags.platform.as_deref(), Some("email"));
    assert_eq!(tags.sender.as_deref(), Some("any"));
    assert_eq!(tags.topic.as_deref(), Some("shipping notifications"));
}

#[test]
fn test_migrated_recurring_summary_parseable() {
    let items = build_recurring_task_summary(
        "Water the plants",
        Some("daily"),
        Some("08:30"),
        Some("sms"),
    );

    let (summary, _, _) = &items[0];
    let tags = parse_summary_tags(summary);
    assert!(tags.has_tags);
    assert_eq!(tags.item_type.as_deref(), Some("recurring"));
    assert_eq!(tags.repeat.as_deref(), Some("daily 08:30"));
}

#[test]
fn test_migrated_oneshot_summary_parseable() {
    let (summary, _, _) = build_oneshot_task_summary("Pick up dry cleaning", Some("sms"));

    let tags = parse_summary_tags(&summary);
    assert!(tags.has_tags);
    assert_eq!(tags.item_type.as_deref(), Some("oneshot"));
    assert_eq!(tags.notify.as_deref(), Some("sms"));
}

#[test]
fn test_migrated_quiet_mode_parseable() {
    let (summary, _, _) = build_quiet_mode_summary();

    let tags = parse_summary_tags(&summary);
    assert!(tags.has_tags);
    assert_eq!(tags.item_type.as_deref(), Some("oneshot"));
    assert_eq!(tags.notify.as_deref(), Some("silent"));
}

#[test]
fn test_digest_migration_has_fetch_tags() {
    let (summary, _) =
        build_digest_migration_summary("09:00", None, Some("email,whatsapp,telegram,calendar"));

    let tags = parse_summary_tags(&summary);
    assert!(tags.fetch.contains(&"email".to_string()));
    assert!(tags.fetch.contains(&"chat".to_string()));
    assert!(tags.fetch.contains(&"calendar".to_string()));
    assert!(tags.fetch.contains(&"items".to_string()));
}

#[test]
fn test_notification_type_maps_to_priority() {
    // "call" -> priority 2
    let (_, _, p_call) = build_oneshot_task_summary("test", Some("call"));
    assert_eq!(p_call, 2);

    // "sms" -> priority 1
    let (_, _, p_sms) = build_oneshot_task_summary("test", Some("sms"));
    assert_eq!(p_sms, 1);

    // None -> priority 1 (defaults to sms)
    let (_, _, p_none) = build_oneshot_task_summary("test", None);
    assert_eq!(p_none, 1);

    // quiet mode -> priority 0
    let (_, _, p_quiet) = build_quiet_mode_summary();
    assert_eq!(p_quiet, 0);
}

// =============================================================================
// Source mapping tests
// =============================================================================

#[test]
fn test_sources_mapping_chat_consolidation() {
    let fetch = map_sources_to_fetch("whatsapp,telegram,signal");
    // All chat platforms should consolidate to "chat", plus "items" appended
    assert_eq!(fetch, "chat,items");
}

#[test]
fn test_sources_mapping_mixed() {
    let fetch = map_sources_to_fetch("email,whatsapp,telegram,signal,calendar");
    assert_eq!(fetch, "email,chat,calendar,items");
}

#[test]
fn test_sources_mapping_email_only() {
    let fetch = map_sources_to_fetch("email");
    assert_eq!(fetch, "email,items");
}

#[test]
fn test_sources_mapping_always_includes_items() {
    let fetch = map_sources_to_fetch("calendar");
    assert!(fetch.contains("items"));
}

#[test]
fn test_sources_mapping_no_duplicates() {
    let fetch = map_sources_to_fetch("email,email,calendar");
    let parts: Vec<&str> = fetch.split(',').collect();
    let unique: std::collections::HashSet<&str> = parts.iter().copied().collect();
    assert_eq!(parts.len(), unique.len());
}

#[test]
fn test_sources_mapping_messenger() {
    // "messenger" should map to "chat" like other messaging apps
    let fetch = map_sources_to_fetch("messenger,email");
    assert!(fetch.contains("chat"));
    assert!(fetch.contains("email"));
}

// =============================================================================
// Weekday mapping tests
// =============================================================================

#[test]
fn test_weekday_number_to_name() {
    assert_eq!(weekday_number_to_name(0), Some("Sunday"));
    assert_eq!(weekday_number_to_name(1), Some("Monday"));
    assert_eq!(weekday_number_to_name(2), Some("Tuesday"));
    assert_eq!(weekday_number_to_name(3), Some("Wednesday"));
    assert_eq!(weekday_number_to_name(4), Some("Thursday"));
    assert_eq!(weekday_number_to_name(5), Some("Friday"));
    assert_eq!(weekday_number_to_name(6), Some("Saturday"));
    assert_eq!(weekday_number_to_name(7), None);
}

// =============================================================================
// Integration tests (using real DB)
// =============================================================================

use backend::test_utils::{
    create_test_state, create_test_task, create_test_user, TestTaskParams, TestUserParams,
};
use backend::UserCoreOps;

#[tokio::test]
async fn test_full_digest_migration() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(100.0, 100.0));

    // Set digest settings for the user
    state
        .user_core
        .update_digests(user.id, Some("09:00"), None, Some("20:00"))
        .expect("Failed to set digests");

    // Run migration
    backend::jobs::scheduler::migrate_digests_to_items(&state).await;

    // Check items were created
    let items = state
        .item_repository
        .get_items(user.id)
        .expect("Failed to get items");

    // Should have 2 items (morning + evening)
    assert_eq!(
        items.len(),
        2,
        "Expected 2 digest items, got {}",
        items.len()
    );

    // All should have proper tags
    for item in &items {
        let tags = parse_summary_tags(&item.summary);
        assert!(tags.has_tags, "Item should have tags: {}", item.summary);
        assert_eq!(tags.item_type.as_deref(), Some("recurring"));
        assert!(tags.repeat.is_some());
        assert!(!tags.fetch.is_empty());
        assert_eq!(item.priority, 1);
        assert!(!item.monitor);
    }

    // Verify digest settings were cleared
    let (m, d, e) = state.user_core.get_digests(user.id).unwrap();
    assert!(m.is_none());
    assert!(d.is_none());
    assert!(e.is_none());
}

#[tokio::test]
async fn test_full_task_migration_digest_task() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(100.0, 100.0));

    // Create a digest task
    create_test_task(&state, &TestTaskParams::digest_task(user.id, "08:00"));

    // Run migration
    backend::jobs::scheduler::migrate_tasks_to_items(&state).await;

    let items = state.item_repository.get_items(user.id).unwrap();
    assert_eq!(items.len(), 1);

    let tags = parse_summary_tags(&items[0].summary);
    assert_eq!(tags.item_type.as_deref(), Some("recurring"));
    assert!(tags.repeat.as_deref().unwrap().contains("daily"));
    assert!(!tags.fetch.is_empty());
    assert_eq!(items[0].priority, 1);
}

#[tokio::test]
async fn test_full_task_migration_monitor_task() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(100.0, 100.0));

    let params = TestTaskParams {
        user_id: user.id,
        trigger: "recurring_email_check".to_string(),
        action: "".to_string(),
        notification_type: Some("sms".to_string()),
        is_permanent: Some(1),
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
        condition: Some("shipping updates from Amazon".to_string()),
        end_time: None,
    };
    create_test_task(&state, &params);

    backend::jobs::scheduler::migrate_tasks_to_items(&state).await;

    let items = state.item_repository.get_items(user.id).unwrap();
    assert_eq!(items.len(), 1);

    let tags = parse_summary_tags(&items[0].summary);
    assert_eq!(tags.item_type.as_deref(), Some("tracking"));
    assert_eq!(tags.platform.as_deref(), Some("email"));
    assert!(items[0].monitor);
}

#[tokio::test]
async fn test_full_task_migration_oneshot() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(100.0, 100.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let params = TestTaskParams::once_task(user.id, now + 3600).with_action("Call mom");
    create_test_task(&state, &params);

    backend::jobs::scheduler::migrate_tasks_to_items(&state).await;

    let items = state.item_repository.get_items(user.id).unwrap();
    assert_eq!(items.len(), 1);

    let tags = parse_summary_tags(&items[0].summary);
    assert_eq!(tags.item_type.as_deref(), Some("oneshot"));
    assert!(items[0].summary.contains("Call mom"));
}

#[tokio::test]
async fn test_full_task_migration_quiet_mode() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(100.0, 100.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    create_test_task(
        &state,
        &TestTaskParams::quiet_mode_task(user.id, Some(now + 7200)),
    );

    backend::jobs::scheduler::migrate_tasks_to_items(&state).await;

    let items = state.item_repository.get_items(user.id).unwrap();
    assert_eq!(items.len(), 1);

    let tags = parse_summary_tags(&items[0].summary);
    assert_eq!(tags.item_type.as_deref(), Some("oneshot"));
    assert_eq!(tags.notify.as_deref(), Some("silent"));
    assert_eq!(items[0].priority, 0);
}

#[tokio::test]
async fn test_expired_oneshot_skipped() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(100.0, 100.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create a one-shot task with trigger in the past
    let params =
        TestTaskParams::once_task(user.id, now - 3600).with_action("This should be skipped");
    create_test_task(&state, &params);

    backend::jobs::scheduler::migrate_tasks_to_items(&state).await;

    let items = state.item_repository.get_items(user.id).unwrap();
    assert_eq!(items.len(), 0, "Expired one-shot should not create an item");
}

#[tokio::test]
async fn test_migration_idempotency_digests() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(100.0, 100.0));

    state
        .user_core
        .update_digests(user.id, Some("09:00"), None, None)
        .unwrap();

    // Run migration twice
    backend::jobs::scheduler::migrate_digests_to_items(&state).await;
    // Re-set digest to simulate re-run (first run clears them)
    // Actually the first run clears digests, so second run finds nothing - that IS idempotent.
    // But let's also test the tag-based check: manually re-set digests
    state
        .user_core
        .update_digests(user.id, Some("09:00"), None, None)
        .unwrap();
    backend::jobs::scheduler::migrate_digests_to_items(&state).await;

    let items = state.item_repository.get_items(user.id).unwrap();
    // Should still only have 1 item (second run sees existing tagged items and skips)
    assert_eq!(
        items.len(),
        1,
        "Idempotent migration should not create duplicates"
    );
}

#[tokio::test]
async fn test_already_migrated_tasks_skipped() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(100.0, 100.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let params = TestTaskParams::once_task(user.id, now + 3600).with_action("Do the thing");
    create_test_task(&state, &params);

    // First migration
    backend::jobs::scheduler::migrate_tasks_to_items(&state).await;
    let items_after_first = state.item_repository.get_items(user.id).unwrap();
    assert_eq!(items_after_first.len(), 1);

    // Second migration - task is now "migrated" status, get_user_tasks only returns "active"
    backend::jobs::scheduler::migrate_tasks_to_items(&state).await;
    let items_after_second = state.item_repository.get_items(user.id).unwrap();
    assert_eq!(
        items_after_second.len(),
        1,
        "Second migration should not create duplicate items"
    );
}

#[tokio::test]
async fn test_full_task_migration_recurring_weekly() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(100.0, 100.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let params = TestTaskParams {
        user_id: user.id,
        trigger: format!("once_{}", now + 86400),
        action: "Team standup".to_string(),
        notification_type: Some("sms".to_string()),
        is_permanent: Some(1),
        recurrence_rule: Some("weekly:1,3,5".to_string()),
        recurrence_time: Some("09:30".to_string()),
        sources: None,
        condition: None,
        end_time: None,
    };
    create_test_task(&state, &params);

    backend::jobs::scheduler::migrate_tasks_to_items(&state).await;

    let items = state.item_repository.get_items(user.id).unwrap();
    // weekly:1,3,5 creates 3 items (Mon, Wed, Fri)
    assert_eq!(items.len(), 3, "Expected 3 items for weekly:1,3,5");

    for item in &items {
        let tags = parse_summary_tags(&item.summary);
        assert_eq!(tags.item_type.as_deref(), Some("recurring"));
        assert!(item.summary.contains("Team standup"));
    }
}
