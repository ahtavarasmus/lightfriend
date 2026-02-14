//! Unit tests for task repository operations.
//!
//! Tests task CRUD operations, limits, recurrence calculations,
//! and completion/rescheduling logic.

use backend::models::user_models::NewTask;
use backend::test_utils::{
    create_test_state, create_test_task, create_test_user, get_user_tasks, TestTaskParams,
    TestUserParams,
};

// =============================================================================
// Task CRUD Tests
// =============================================================================

#[test]
fn test_create_task_success() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let params = TestTaskParams::once_task(user.id, now + 3600);
    let task = create_test_task(&state, &params);

    assert_eq!(task.user_id, user.id);
    assert_eq!(task.action, "test_action");
    assert_eq!(task.status, Some("active".to_string()));
}

#[test]
fn test_create_task_at_limit() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create 49 tasks (below limit)
    for i in 0..49 {
        let params = TestTaskParams::once_task(user.id, now + 3600 + i);
        create_test_task(&state, &params);
    }

    // 50th task should succeed (at limit)
    let params = TestTaskParams::once_task(user.id, now + 5000);
    let task = create_test_task(&state, &params);
    assert_eq!(task.user_id, user.id);

    let tasks = get_user_tasks(&state, user.id);
    assert_eq!(tasks.len(), 50);
}

#[test]
fn test_create_task_exceeds_limit() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create 50 tasks (at limit)
    for i in 0..50 {
        let new_task = NewTask {
            user_id: user.id,
            trigger: format!("once_{}", now + 3600 + i),
            condition: None,
            action: "test_action".to_string(),
            notification_type: Some("sms".to_string()),
            status: "active".to_string(),
            created_at: now,
            is_permanent: Some(0),
            recurrence_rule: None,
            recurrence_time: None,
            sources: None,
            end_time: None,
        };
        state
            .user_repository
            .create_task(&new_task)
            .expect("Should create task");
    }

    // 51st task should fail
    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", now + 10000),
        condition: None,
        action: "test_action".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: now,
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
        end_time: None,
    };
    let result = state.user_repository.create_task(&new_task);

    assert!(result.is_err());
    if let Err(diesel::result::Error::DatabaseError(kind, _)) = result {
        assert!(matches!(
            kind,
            diesel::result::DatabaseErrorKind::CheckViolation
        ));
    } else {
        panic!("Expected CheckViolation error");
    }
}

#[test]
fn test_get_user_tasks_returns_active_only() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create two tasks
    let params1 = TestTaskParams::once_task(user.id, now + 3600);
    let task1 = create_test_task(&state, &params1);
    let params2 = TestTaskParams::once_task(user.id, now + 7200);
    let _task2 = create_test_task(&state, &params2);

    // Cancel one task
    state
        .user_repository
        .cancel_task(user.id, task1.id.unwrap())
        .expect("Failed to cancel task");

    // Should only return active tasks
    let tasks = get_user_tasks(&state, user.id);
    assert_eq!(tasks.len(), 1);
    assert_ne!(tasks[0].id, task1.id);
}

#[test]
fn test_cancel_task_updates_status() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let params = TestTaskParams::once_task(user.id, now + 3600);
    let task = create_test_task(&state, &params);

    let result = state
        .user_repository
        .cancel_task(user.id, task.id.unwrap())
        .expect("Failed to cancel task");

    assert!(result); // Returns true if task was cancelled

    // Task should no longer appear in active tasks
    let tasks = get_user_tasks(&state, user.id);
    assert!(tasks.is_empty());
}

#[test]
fn test_cancel_task_wrong_user() {
    let state = create_test_state();
    let user1 = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let mut params2 = TestUserParams::finland_user(10.0, 5.0);
    params2.email = "user2@example.com".to_string();
    let user2 = create_test_user(&state, &params2);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create task for user1
    let params = TestTaskParams::once_task(user1.id, now + 3600);
    let task = create_test_task(&state, &params);

    // User2 tries to cancel user1's task
    let result = state
        .user_repository
        .cancel_task(user2.id, task.id.unwrap())
        .expect("Cancel should not error");

    assert!(!result); // Returns false because no matching task

    // Task should still exist for user1
    let tasks = get_user_tasks(&state, user1.id);
    assert_eq!(tasks.len(), 1);
}

// =============================================================================
// Recurrence Calculation Tests
// =============================================================================

#[test]
fn test_complete_nonpermanent_task() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let params = TestTaskParams::once_task(user.id, now + 3600);
    let task = create_test_task(&state, &params);

    // Complete the task
    let rescheduled = state
        .user_repository
        .complete_or_reschedule_task(&task, "UTC")
        .expect("Failed to complete task");

    assert!(!rescheduled); // Non-permanent task should be completed, not rescheduled

    // Task should no longer appear in active tasks
    let tasks = get_user_tasks(&state, user.id);
    assert!(tasks.is_empty());
}

#[test]
fn test_reschedule_permanent_daily() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams::permanent_daily(user.id, "09:00");
    let task = create_test_task(&state, &params);

    let original_trigger = task.trigger.clone();

    // Complete/reschedule the task
    let rescheduled = state
        .user_repository
        .complete_or_reschedule_task(&task, "UTC")
        .expect("Failed to reschedule task");

    assert!(rescheduled); // Permanent task should be rescheduled

    // Task should still be active with new trigger
    let tasks = get_user_tasks(&state, user.id);
    assert_eq!(tasks.len(), 1);
    assert_ne!(tasks[0].trigger, original_trigger);
    assert!(tasks[0].trigger.starts_with("once_"));
}

#[test]
fn test_reschedule_permanent_weekly() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create a weekly task (Mon, Wed, Fri = 1,3,5)
    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", now),
        condition: None,
        action: "weekly_action".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: now,
        is_permanent: Some(1),
        recurrence_rule: Some("weekly:1,3,5".to_string()),
        recurrence_time: Some("10:00".to_string()),
        sources: None,
        end_time: None,
    };
    state
        .user_repository
        .create_task(&new_task)
        .expect("Failed to create task");

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];

    // Complete/reschedule the task
    let rescheduled = state
        .user_repository
        .complete_or_reschedule_task(task, "UTC")
        .expect("Failed to reschedule task");

    assert!(rescheduled);

    // Verify task was rescheduled with new trigger
    let updated_tasks = get_user_tasks(&state, user.id);
    assert_eq!(updated_tasks.len(), 1);
    assert!(updated_tasks[0].trigger.starts_with("once_"));
}

#[test]
fn test_reschedule_permanent_monthly() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create a monthly task (15th of each month)
    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", now),
        condition: None,
        action: "monthly_action".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: now,
        is_permanent: Some(1),
        recurrence_rule: Some("monthly:15".to_string()),
        recurrence_time: Some("14:00".to_string()),
        sources: None,
        end_time: None,
    };
    state
        .user_repository
        .create_task(&new_task)
        .expect("Failed to create task");

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];

    // Complete/reschedule the task
    let rescheduled = state
        .user_repository
        .complete_or_reschedule_task(task, "UTC")
        .expect("Failed to reschedule task");

    assert!(rescheduled);

    // Verify task was rescheduled
    let updated_tasks = get_user_tasks(&state, user.id);
    assert_eq!(updated_tasks.len(), 1);
    assert!(updated_tasks[0].trigger.starts_with("once_"));
}

#[test]
fn test_reschedule_with_timezone() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams::permanent_daily(user.id, "09:00");
    let task = create_test_task(&state, &params);

    // Test with different timezones
    let rescheduled_utc = state
        .user_repository
        .complete_or_reschedule_task(&task, "UTC")
        .expect("Failed to reschedule task");
    assert!(rescheduled_utc);

    // Get updated task for next test
    let tasks = get_user_tasks(&state, user.id);
    let updated_task = &tasks[0];

    // Reschedule again with different timezone
    let rescheduled_la = state
        .user_repository
        .complete_or_reschedule_task(updated_task, "America/Los_Angeles")
        .expect("Failed to reschedule task");
    assert!(rescheduled_la);
}

#[test]
fn test_no_reschedule_without_recurrence() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create a permanent task WITHOUT recurrence settings
    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", now),
        condition: None,
        action: "test_action".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: now,
        is_permanent: Some(1),
        recurrence_rule: None, // No recurrence rule
        recurrence_time: None, // No recurrence time
        sources: None,
        end_time: None,
    };
    state
        .user_repository
        .create_task(&new_task)
        .expect("Failed to create task");

    let tasks = get_user_tasks(&state, user.id);
    let task = &tasks[0];

    // Should complete (not reschedule) because recurrence settings are missing
    let rescheduled = state
        .user_repository
        .complete_or_reschedule_task(task, "UTC")
        .expect("Failed to complete task");

    assert!(!rescheduled);
    let tasks = get_user_tasks(&state, user.id);
    assert!(tasks.is_empty());
}

// =============================================================================
// Due Tasks Tests
// =============================================================================

#[test]
fn test_get_due_once_tasks() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create a past task (due)
    let params_past = TestTaskParams::once_task(user.id, now - 100);
    create_test_task(&state, &params_past);

    // Create a future task (not due)
    let params_future = TestTaskParams::once_task(user.id, now + 3600);
    create_test_task(&state, &params_future);

    // Get due tasks
    let due_tasks = state
        .user_repository
        .get_due_once_tasks(now)
        .expect("Failed to get due tasks");

    assert_eq!(due_tasks.len(), 1);
    assert!(due_tasks[0].trigger.contains(&(now - 100).to_string()));
}

#[test]
fn test_get_due_once_tasks_empty() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create only future tasks
    let params = TestTaskParams::once_task(user.id, now + 3600);
    create_test_task(&state, &params);

    let due_tasks = state
        .user_repository
        .get_due_once_tasks(now)
        .expect("Failed to get due tasks");

    assert!(due_tasks.is_empty());
}

// =============================================================================
// Task with Sources Tests
// =============================================================================

#[test]
fn test_create_task_with_sources() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams::digest_task(user.id, "08:00");
    let task = create_test_task(&state, &params);

    assert_eq!(task.action, "generate_digest");
    assert_eq!(
        task.sources,
        Some("email,whatsapp,telegram,signal,calendar".to_string())
    );
    assert_eq!(task.is_permanent, Some(1));
    assert_eq!(task.recurrence_rule, Some("daily".to_string()));
}

#[test]
fn test_create_task_with_condition() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let params = TestTaskParams::once_task(user.id, now + 3600)
        .with_condition("urgent emails present")
        .with_sources("email");
    let task = create_test_task(&state, &params);

    assert_eq!(task.condition, Some("urgent emails present".to_string()));
    assert_eq!(task.sources, Some("email".to_string()));
}

// =============================================================================
// Task Cleanup Tests
// =============================================================================

#[test]
fn test_delete_old_tasks() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create and complete a task with old completed_at
    let params = TestTaskParams::once_task(user.id, now - 1000);
    let task = create_test_task(&state, &params);

    state
        .user_repository
        .update_task_status(task.id.unwrap(), "completed")
        .expect("Failed to complete task");

    // Delete tasks completed before now
    let deleted = state
        .user_repository
        .delete_old_tasks(now + 1)
        .expect("Failed to delete old tasks");

    assert_eq!(deleted, 1);
}
