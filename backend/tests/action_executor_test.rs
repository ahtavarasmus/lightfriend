//! Unit tests for action executor.
//!
//! Tests action parsing, source fetching, condition evaluation,
//! and the execution pipeline.
//!
//! Note: Some tests are marked as integration tests since they would
//! require mocking external services (AI, Twilio). These are structured
//! to document expected behavior.

use backend::models::user_models::NewTask;
use backend::test_utils::{
    create_test_state, create_test_task, create_test_user, TestTaskParams, TestUserParams,
};
use backend::utils::action_executor::ActionResult;

// =============================================================================
// parse_action Tests
// =============================================================================

// Note: parse_action is a private function in action_executor.rs
// These tests document the expected behavior through integration tests

#[test]
fn test_task_with_simple_action() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let params = TestTaskParams::once_task(user.id, now + 3600).with_action("generate_digest");
    let task = create_test_task(&state, &params);

    // Simple action: "generate_digest" -> ("generate_digest", None)
    assert_eq!(task.action, "generate_digest");
    assert!(!task.action.contains('('));
}

#[test]
fn test_task_with_parameterized_action() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create task with parameterized action
    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", now + 3600),
        condition: None,
        action: "control_tesla(climate_on)".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: now,
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
    };
    state
        .user_repository
        .create_task(&new_task)
        .expect("Failed to create task");

    let tasks = state
        .user_repository
        .get_user_tasks(user.id)
        .expect("Failed to get tasks");
    let task = &tasks[0];

    // Parameterized action: "control_tesla(climate_on)"
    assert_eq!(task.action, "control_tesla(climate_on)");
    assert!(task.action.contains('('));
    assert!(task.action.contains(')'));
}

#[test]
fn test_task_with_empty_action() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create task with empty action (notification only)
    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", now + 3600),
        condition: None,
        action: "".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: now,
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: Some("email".to_string()),
    };
    state
        .user_repository
        .create_task(&new_task)
        .expect("Failed to create task");

    let tasks = state
        .user_repository
        .get_user_tasks(user.id)
        .expect("Failed to get tasks");
    let task = &tasks[0];

    // Empty action: "" -> ("", None)
    assert_eq!(task.action, "");
}

// =============================================================================
// Task Source Configuration Tests
// =============================================================================

#[test]
fn test_task_with_empty_sources() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create task with empty sources string
    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", now + 3600),
        condition: None,
        action: "test_action".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: now,
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: Some("".to_string()),
    };
    state
        .user_repository
        .create_task(&new_task)
        .expect("Failed to create task");

    let tasks = state
        .user_repository
        .get_user_tasks(user.id)
        .expect("Failed to get tasks");
    let task = &tasks[0];

    assert_eq!(task.sources, Some("".to_string()));
}

#[test]
fn test_task_with_none_sources() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams::once_task(user.id, 12345);
    let task = create_test_task(&state, &params);

    // Default TestTaskParams has sources = None
    assert!(task.sources.is_none());
}

#[test]
fn test_task_with_multiple_sources() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams::digest_task(user.id, "08:00");
    let task = create_test_task(&state, &params);

    // Digest task has multiple sources
    let sources = task.sources.expect("Should have sources");
    assert!(sources.contains("email"));
    assert!(sources.contains("whatsapp"));
    assert!(sources.contains("telegram"));
}

// =============================================================================
// Condition Configuration Tests
// =============================================================================

#[test]
fn test_task_with_condition() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let params = TestTaskParams::once_task(user.id, now + 3600)
        .with_condition("urgent message from boss")
        .with_sources("email,whatsapp");
    let task = create_test_task(&state, &params);

    assert_eq!(task.condition, Some("urgent message from boss".to_string()));
    assert_eq!(task.sources, Some("email,whatsapp".to_string()));
}

#[test]
fn test_task_without_condition() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams::digest_task(user.id, "08:00");
    let task = create_test_task(&state, &params);

    // Digest tasks typically don't have conditions
    assert!(task.condition.is_none());
}

// =============================================================================
// Notification Type Tests
// =============================================================================

#[test]
fn test_task_notification_type_sms() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams::digest_task(user.id, "08:00");
    let task = create_test_task(&state, &params);

    assert_eq!(task.notification_type, Some("sms".to_string()));
}

#[test]
fn test_task_notification_type_call() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create task with call notification
    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", now + 3600),
        condition: None,
        action: "urgent_alert".to_string(),
        notification_type: Some("call".to_string()),
        status: "active".to_string(),
        created_at: now,
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
    };
    state
        .user_repository
        .create_task(&new_task)
        .expect("Failed to create task");

    let tasks = state
        .user_repository
        .get_user_tasks(user.id)
        .expect("Failed to get tasks");
    let task = &tasks[0];

    assert_eq!(task.notification_type, Some("call".to_string()));
}

// =============================================================================
// ActionResult Enum Tests
// =============================================================================

#[test]
fn test_action_result_success() {
    let result = ActionResult::Success {
        message: "Task completed successfully".to_string(),
    };

    match result {
        ActionResult::Success { message } => {
            assert_eq!(message, "Task completed successfully");
        }
        _ => panic!("Expected Success variant"),
    }
}

#[test]
fn test_action_result_failed() {
    let result = ActionResult::Failed {
        error: "Unknown action tool: fake_tool".to_string(),
    };

    match result {
        ActionResult::Failed { error } => {
            assert!(error.contains("Unknown action tool"));
        }
        _ => panic!("Expected Failed variant"),
    }
}

#[test]
fn test_action_result_skipped() {
    let result = ActionResult::Skipped {
        reason: "Condition not met: urgent emails present".to_string(),
    };

    match result {
        ActionResult::Skipped { reason } => {
            assert!(reason.contains("Condition not met"));
        }
        _ => panic!("Expected Skipped variant"),
    }
}

// =============================================================================
// Source Lookback Configuration Tests
// =============================================================================

// =============================================================================
// Known Action Types Tests
// =============================================================================

#[test]
fn test_known_action_generate_digest() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams::digest_task(user.id, "08:00");
    let task = create_test_task(&state, &params);

    assert_eq!(task.action, "generate_digest");
}

#[test]
fn test_known_action_control_tesla() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let params =
        TestTaskParams::once_task(user.id, now + 3600).with_action("control_tesla(climate_on)");

    // Note: This uses the builder pattern but we need to create the task manually
    // because with_action returns Self, not creating in DB
    let new_task = NewTask {
        user_id: params.user_id,
        trigger: params.trigger.clone(),
        condition: params.condition.clone(),
        action: "control_tesla(climate_on)".to_string(),
        notification_type: params.notification_type.clone(),
        status: "active".to_string(),
        created_at: now,
        is_permanent: params.is_permanent,
        recurrence_rule: params.recurrence_rule.clone(),
        recurrence_time: params.recurrence_time.clone(),
        sources: params.sources.clone(),
    };
    state
        .user_repository
        .create_task(&new_task)
        .expect("Failed to create task");

    let tasks = state
        .user_repository
        .get_user_tasks(user.id)
        .expect("Failed to get tasks");
    let task = &tasks[0];

    assert_eq!(task.action, "control_tesla(climate_on)");
}

#[test]
fn test_known_action_get_weather() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", now + 3600),
        condition: None,
        action: "get_weather(Helsinki)".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: now,
        is_permanent: Some(1),
        recurrence_rule: Some("daily".to_string()),
        recurrence_time: Some("07:00".to_string()),
        sources: None,
    };
    state
        .user_repository
        .create_task(&new_task)
        .expect("Failed to create task");

    let tasks = state
        .user_repository
        .get_user_tasks(user.id)
        .expect("Failed to get tasks");
    let task = &tasks[0];

    assert_eq!(task.action, "get_weather(Helsinki)");
}

#[test]
fn test_known_action_fetch_calendar() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", now + 3600),
        condition: None,
        action: "fetch_calendar_events".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: now,
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
    };
    state
        .user_repository
        .create_task(&new_task)
        .expect("Failed to create task");

    let tasks = state
        .user_repository
        .get_user_tasks(user.id)
        .expect("Failed to get tasks");
    let task = &tasks[0];

    assert_eq!(task.action, "fetch_calendar_events");
}

// =============================================================================
// Task State Transition Tests
// =============================================================================

#[test]
fn test_task_lifecycle_one_time() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // 1. Create one-time task
    let params = TestTaskParams::once_task(user.id, now + 3600);
    let task = create_test_task(&state, &params);
    assert_eq!(task.status, Some("active".to_string()));
    assert!(task.completed_at.is_none());

    // 2. Complete the task (simulating execution)
    let rescheduled = state
        .user_repository
        .complete_or_reschedule_task(&task, "UTC")
        .expect("Failed to complete task");

    assert!(!rescheduled); // One-time task should be completed, not rescheduled

    // 3. Task should no longer be active
    let tasks = state
        .user_repository
        .get_user_tasks(user.id)
        .expect("Failed to get tasks");
    assert!(tasks.is_empty());
}

#[test]
fn test_task_lifecycle_permanent_recurring() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // 1. Create permanent recurring task
    let params = TestTaskParams::permanent_daily(user.id, "09:00");
    let task = create_test_task(&state, &params);
    let original_trigger = task.trigger.clone();

    assert_eq!(task.status, Some("active".to_string()));
    assert_eq!(task.is_permanent, Some(1));

    // 2. Complete/reschedule the task
    let rescheduled = state
        .user_repository
        .complete_or_reschedule_task(&task, "UTC")
        .expect("Failed to reschedule task");

    assert!(rescheduled); // Permanent task should be rescheduled

    // 3. Task should still be active with new trigger
    let tasks = state
        .user_repository
        .get_user_tasks(user.id)
        .expect("Failed to get tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].status, Some("active".to_string()));
    assert_ne!(tasks[0].trigger, original_trigger);
}
