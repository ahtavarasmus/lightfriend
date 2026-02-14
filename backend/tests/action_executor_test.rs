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

/// Fixed reference timestamp for deterministic tests.
/// T - 60 = "60s ago", T + 3600 = "1h later".
const T: i32 = 1_750_000_000;

// =============================================================================
// parse_action Tests
// =============================================================================

// Note: parse_action is a private function in action_executor.rs
// These tests document the expected behavior through integration tests

#[test]
fn test_task_with_simple_action() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams::once_task(user.id, T + 3600).with_action("generate_digest");
    let task = create_test_task(&state, &params);

    // Simple action: "generate_digest" -> ("generate_digest", None)
    assert_eq!(task.action, "generate_digest");
    assert!(!task.action.contains('('));
}

#[test]
fn test_task_with_parameterized_action() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Create task with parameterized action
    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", T + 3600),
        condition: None,
        action: "control_tesla(climate_on)".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: T,
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
        end_time: None,
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

    // Create task with empty action (notification only)
    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", T + 3600),
        condition: None,
        action: "".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: T,
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: Some("email".to_string()),
        end_time: None,
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

    // Create task with empty sources string
    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", T + 3600),
        condition: None,
        action: "test_action".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: T,
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: Some("".to_string()),
        end_time: None,
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

    let params = TestTaskParams::once_task(user.id, T + 3600)
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

    // Create task with call notification
    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", T + 3600),
        condition: None,
        action: "urgent_alert".to_string(),
        notification_type: Some("call".to_string()),
        status: "active".to_string(),
        created_at: T,
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
        end_time: None,
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

    let params =
        TestTaskParams::once_task(user.id, T + 3600).with_action("control_tesla(climate_on)");

    // Note: This uses the builder pattern but we need to create the task manually
    // because with_action returns Self, not creating in DB
    let new_task = NewTask {
        user_id: params.user_id,
        trigger: params.trigger.clone(),
        condition: params.condition.clone(),
        action: "control_tesla(climate_on)".to_string(),
        notification_type: params.notification_type.clone(),
        status: "active".to_string(),
        created_at: T,
        is_permanent: params.is_permanent,
        recurrence_rule: params.recurrence_rule.clone(),
        recurrence_time: params.recurrence_time.clone(),
        sources: params.sources.clone(),
        end_time: None,
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

    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", T + 3600),
        condition: None,
        action: "get_weather(Helsinki)".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: T,
        is_permanent: Some(1),
        recurrence_rule: Some("daily".to_string()),
        recurrence_time: Some("07:00".to_string()),
        sources: None,
        end_time: None,
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

    let new_task = NewTask {
        user_id: user.id,
        trigger: format!("once_{}", T + 3600),
        condition: None,
        action: "fetch_calendar_events".to_string(),
        notification_type: Some("sms".to_string()),
        status: "active".to_string(),
        created_at: T,
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
        end_time: None,
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

    // 1. Create one-time task
    let params = TestTaskParams::once_task(user.id, T + 3600);
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

// =============================================================================
// Action Parsing - New JSON Format
// =============================================================================

use backend::utils::action_executor::parse_action_structured;

/// Query a task by id regardless of status (get_user_tasks only returns active tasks)
fn get_task_by_id(
    state: &std::sync::Arc<backend::AppState>,
    task_id: i32,
) -> backend::models::user_models::Task {
    use backend::schema::tasks;
    use diesel::prelude::*;

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    tasks::table
        .filter(tasks::id.eq(task_id))
        .first::<backend::models::user_models::Task>(&mut conn)
        .expect("Failed to find task by id")
}

#[test]
fn test_parse_action_json_send_reminder() {
    let action =
        parse_action_structured(r#"{"tool":"send_reminder","params":{"message":"Call mom"}}"#);
    assert_eq!(action.tool, "send_reminder");
    let params = action.params.expect("params should exist");
    assert_eq!(params["message"], "Call mom");
}

#[test]
fn test_parse_action_json_control_tesla() {
    let action =
        parse_action_structured(r#"{"tool":"control_tesla","params":{"command":"climate_on"}}"#);
    assert_eq!(action.tool, "control_tesla");
    let params = action.params.expect("params should exist");
    assert_eq!(params["command"], "climate_on");
}

#[test]
fn test_parse_action_json_send_chat_message() {
    let action = parse_action_structured(
        r#"{"tool":"send_chat_message","params":{"platform":"whatsapp","contact":"Wife","message":"Running late"}}"#,
    );
    assert_eq!(action.tool, "send_chat_message");
    let params = action.params.expect("params should exist");
    assert_eq!(params["platform"], "whatsapp");
    assert_eq!(params["contact"], "Wife");
    assert_eq!(params["message"], "Running late");
}

#[test]
fn test_parse_action_json_no_params() {
    let action = parse_action_structured(r#"{"tool":"generate_digest"}"#);
    assert_eq!(action.tool, "generate_digest");
    assert!(action.params.is_none());
}

// =============================================================================
// Action Parsing - Old Dynamic Format
// =============================================================================

#[test]
fn test_parse_action_old_simple() {
    let action = parse_action_structured("generate_digest");
    assert_eq!(action.tool, "generate_digest");
    assert!(action.params.is_none());
}

#[test]
fn test_parse_action_old_with_param() {
    let action = parse_action_structured("control_tesla(climate_on)");
    assert_eq!(action.tool, "control_tesla");
    let params = action.params.expect("params should exist");
    assert_eq!(params["command"], "climate_on");
}

#[test]
fn test_parse_action_old_send_reminder() {
    let action = parse_action_structured("send_reminder(Call mom)");
    assert_eq!(action.tool, "send_reminder");
    let params = action.params.expect("params should exist");
    assert_eq!(params["message"], "Call mom");
}

#[test]
fn test_parse_action_old_chat_message() {
    let action = parse_action_structured("send_chat_message(whatsapp, Wife, Running late)");
    assert_eq!(action.tool, "send_chat_message");
    let params = action.params.expect("params should exist");
    assert_eq!(params["platform"], "whatsapp");
    assert_eq!(params["contact"], "Wife");
    assert_eq!(params["message"], "Running late");
}

#[test]
fn test_parse_action_empty() {
    let action = parse_action_structured("");
    assert_eq!(action.tool, "");
    assert!(action.params.is_none());
}

// =============================================================================
// Due Task Finding
// =============================================================================

#[test]
fn test_get_due_once_tasks_past_trigger() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Task triggered 60 seconds ago
    let params = TestTaskParams::once_task(user.id, T - 60);
    create_test_task(&state, &params);

    let due = state
        .user_repository
        .get_due_once_tasks(T)
        .expect("Failed to get due tasks");
    assert_eq!(due.len(), 1);
    assert!(due[0].trigger.starts_with("once_"));
}

#[test]
fn test_get_due_once_tasks_future_trigger() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Task scheduled 1 hour in the future
    let params = TestTaskParams::once_task(user.id, T + 3600);
    create_test_task(&state, &params);

    let due = state
        .user_repository
        .get_due_once_tasks(T)
        .expect("Failed to get due tasks");
    assert!(due.is_empty(), "Future task should not be due");
}

#[test]
fn test_get_due_once_tasks_skips_recurring() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Create a recurring_email task (not a once_ task)
    let params = TestTaskParams {
        user_id: user.id,
        trigger: "recurring_email".to_string(),
        action: "test_action".to_string(),
        notification_type: Some("sms".to_string()),
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
        condition: Some("from HR".to_string()),
        end_time: None,
    };
    create_test_task(&state, &params);

    let due = state
        .user_repository
        .get_due_once_tasks(T)
        .expect("Failed to get due tasks");
    assert!(
        due.is_empty(),
        "recurring_email task should not appear in once_ due tasks"
    );
}

// =============================================================================
// Task Lifecycle for Recurring Triggers
// =============================================================================

#[test]
fn test_recurring_email_task_lifecycle() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Create recurring_email task
    let params = TestTaskParams {
        user_id: user.id,
        trigger: "recurring_email".to_string(),
        action: "test_action".to_string(),
        notification_type: Some("sms".to_string()),
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
        condition: Some("from HR".to_string()),
        end_time: None,
    };
    let task = create_test_task(&state, &params);
    assert_eq!(task.status, Some("active".to_string()));
    assert_eq!(task.trigger, "recurring_email");

    // complete_or_reschedule_task on a recurring_email task:
    // - not permanent+recurrence+once_, so it marks completed
    // In production, recurring_email tasks are evaluated inline on each
    // incoming email and are NOT processed through complete_or_reschedule.
    // This test verifies the fallthrough behavior.
    let rescheduled = state
        .user_repository
        .complete_or_reschedule_task(&task, "UTC")
        .expect("Failed to complete task");
    assert!(
        !rescheduled,
        "recurring_email should not be rescheduled via complete_or_reschedule"
    );

    let updated = get_task_by_id(&state, task.id.unwrap());
    assert_eq!(updated.status, Some("completed".to_string()));
}

#[test]
fn test_recurring_messaging_task_lifecycle() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Create recurring_messaging task
    let params = TestTaskParams {
        user_id: user.id,
        trigger: "recurring_messaging".to_string(),
        action: "test_action".to_string(),
        notification_type: Some("sms".to_string()),
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
        condition: Some("from mom".to_string()),
        end_time: None,
    };
    let task = create_test_task(&state, &params);
    assert_eq!(task.status, Some("active".to_string()));
    assert_eq!(task.trigger, "recurring_messaging");

    // Same fallthrough as recurring_email
    let rescheduled = state
        .user_repository
        .complete_or_reschedule_task(&task, "UTC")
        .expect("Failed to complete task");
    assert!(
        !rescheduled,
        "recurring_messaging should not be rescheduled via complete_or_reschedule"
    );

    let updated = get_task_by_id(&state, task.id.unwrap());
    assert_eq!(updated.status, Some("completed".to_string()));
}

#[test]
fn test_task_lifecycle_with_json_action() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Create a one-time task with JSON action
    let params = TestTaskParams::once_task(user.id, T - 60)
        .with_action(r#"{"tool":"send_reminder","params":{"message":"test"}}"#);
    let task = create_test_task(&state, &params);
    assert_eq!(task.status, Some("active".to_string()));

    // One-time task (is_permanent=0) should complete, not reschedule
    let rescheduled = state
        .user_repository
        .complete_or_reschedule_task(&task, "UTC")
        .expect("Failed to complete task");
    assert!(!rescheduled, "One-time task should not be rescheduled");

    let updated = get_task_by_id(&state, task.id.unwrap());
    assert_eq!(updated.status, Some("completed".to_string()));
}

// =============================================================================
// Action Parsing - Corner Cases
// =============================================================================

#[test]
fn test_parse_action_whitespace_padded() {
    // Code calls action.trim() - verify that works
    let action = parse_action_structured("  \t send_reminder(Call mom) \n ");
    assert_eq!(action.tool, "send_reminder");
    let params = action.params.expect("params should exist");
    assert_eq!(params["message"], "Call mom");
}

#[test]
fn test_parse_action_whitespace_only() {
    let action = parse_action_structured("   \t\n  ");
    assert_eq!(action.tool, "");
    assert!(action.params.is_none());
}

#[test]
fn test_parse_action_malformed_json_falls_to_old_format() {
    // Input: {"broken json(oops) - invalid JSON, has '(' and ends with ')'
    // JSON parse fails, then old format: tool={"broken json, params={"raw":"oops"}
    let action = parse_action_structured(r#"{"broken json(oops)"#);
    assert_eq!(action.tool, r#"{"broken json"#);
    let params = action.params.expect("params should exist");
    assert_eq!(params["raw"], "oops");
}

#[test]
fn test_parse_action_malformed_json_no_parens() {
    // Invalid JSON without parens - treated as simple tool name
    let action = parse_action_structured(r#"{"not_valid_json"#);
    assert_eq!(action.tool, r#"{"not_valid_json"#);
    assert!(action.params.is_none());
}

#[test]
fn test_parse_action_json_wrong_keys_falls_through() {
    // Valid JSON object, but missing "tool" key - serde deser fails, falls to old format
    let input = r#"{"action":"send_reminder","data":"hello"}"#;
    let action = parse_action_structured(input);
    // No '(' found, so it's treated as a simple tool name
    assert_eq!(action.tool, input);
    assert!(action.params.is_none());
}

#[test]
fn test_parse_action_old_format_open_paren_no_close() {
    // Has '(' but doesn't end with ')' - falls to simple tool name
    let action = parse_action_structured("send_reminder(Call mom");
    assert_eq!(action.tool, "send_reminder(Call mom");
    assert!(action.params.is_none());
}

#[test]
fn test_parse_action_old_format_empty_parens() {
    // tool() - empty param string
    let action = parse_action_structured("control_tesla()");
    assert_eq!(action.tool, "control_tesla");
    let params = action.params.expect("params should exist");
    // Empty string maps to {"command": ""} for control_tesla
    assert_eq!(params["command"], "");
}

#[test]
fn test_parse_action_old_chat_message_too_few_parts() {
    // send_chat_message with < 3 comma-separated parts falls to {"raw": ...}
    let action = parse_action_structured("send_chat_message(whatsapp, Wife)");
    assert_eq!(action.tool, "send_chat_message");
    let params = action.params.expect("params should exist");
    assert_eq!(params["raw"], "whatsapp, Wife");
    // Should NOT have platform/contact keys
    assert!(params.get("platform").is_none());
}

#[test]
fn test_parse_action_old_unknown_tool_gets_raw_param() {
    // Unknown tool name maps param to {"raw": ...}
    let action = parse_action_structured("future_tool(some_param)");
    assert_eq!(action.tool, "future_tool");
    let params = action.params.expect("params should exist");
    assert_eq!(params["raw"], "some_param");
}

#[test]
fn test_parse_action_json_params_is_array() {
    // params can be any JSON value - test with array
    let action = parse_action_structured(r#"{"tool":"multi","params":["a","b","c"]}"#);
    assert_eq!(action.tool, "multi");
    let params = action.params.expect("params should exist");
    assert!(params.is_array());
    assert_eq!(params.as_array().unwrap().len(), 3);
}

#[test]
fn test_parse_action_json_params_is_string() {
    // params as a raw string value
    let action = parse_action_structured(r#"{"tool":"echo","params":"hello world"}"#);
    assert_eq!(action.tool, "echo");
    let params = action.params.expect("params should exist");
    assert_eq!(params.as_str().unwrap(), "hello world");
}

#[test]
fn test_parse_action_json_extra_fields_ignored() {
    // serde should ignore unknown fields (default behavior with no deny_unknown_fields)
    let action = parse_action_structured(
        r#"{"tool":"send_reminder","params":{"message":"hi"},"extra":"ignored","version":2}"#,
    );
    assert_eq!(action.tool, "send_reminder");
    let params = action.params.expect("params should exist");
    assert_eq!(params["message"], "hi");
}

#[test]
fn test_parse_action_old_format_param_with_nested_parens() {
    // Param containing nested parens - only outer paren pair stripped
    // send_reminder(Call mom (urgent)) - find('(') gets first, ends_with(')') is true
    // param = "Call mom (urgent)"
    let action = parse_action_structured("send_reminder(Call mom (urgent))");
    assert_eq!(action.tool, "send_reminder");
    let params = action.params.expect("params should exist");
    assert_eq!(params["message"], "Call mom (urgent)");
}

// =============================================================================
// Action Parsing - Roundtrip (parse -> to_action_string -> parse)
// =============================================================================

use backend::utils::action_executor::StructuredAction;

#[test]
fn test_roundtrip_json_to_display_to_parse() {
    // Parse JSON format
    let original =
        parse_action_structured(r#"{"tool":"send_reminder","params":{"message":"Call mom"}}"#);
    assert_eq!(original.tool, "send_reminder");

    // Convert to old display format
    let display = original.to_action_string();
    assert_eq!(display, "send_reminder(Call mom)");

    // Parse the display format back
    let reparsed = parse_action_structured(&display);
    assert_eq!(reparsed.tool, "send_reminder");
    let params = reparsed.params.expect("params should exist");
    assert_eq!(params["message"], "Call mom");
}

#[test]
fn test_to_action_string_no_params() {
    let action = StructuredAction {
        tool: "generate_digest".to_string(),
        params: None,
    };
    assert_eq!(action.to_action_string(), "generate_digest");
}

#[test]
fn test_to_action_string_numeric_param_no_display() {
    // Params with non-string values - as_str() returns None, so falls to just tool name
    let action = StructuredAction {
        tool: "some_tool".to_string(),
        params: Some(serde_json::json!({"count": 42})),
    };
    assert_eq!(action.to_action_string(), "some_tool");
}

#[test]
fn test_to_action_string_empty_object() {
    // Empty params object - no values, falls to just tool name
    let action = StructuredAction {
        tool: "some_tool".to_string(),
        params: Some(serde_json::json!({})),
    };
    assert_eq!(action.to_action_string(), "some_tool");
}

// =============================================================================
// Due Task Finding - Corner Cases
// =============================================================================

#[test]
fn test_get_due_once_tasks_exact_boundary() {
    // Task trigger timestamp exactly == T (ts <= T should match)
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams::once_task(user.id, T);
    create_test_task(&state, &params);

    let due = state
        .user_repository
        .get_due_once_tasks(T)
        .expect("Failed to get due tasks");
    assert_eq!(due.len(), 1, "Task at exact now boundary should be due");
}

#[test]
fn test_get_due_once_tasks_corrupted_trigger() {
    // Task with trigger "once_notanumber" - parse::<i32>() fails, filtered out
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams {
        user_id: user.id,
        trigger: "once_notanumber".to_string(),
        action: "test_action".to_string(),
        notification_type: Some("sms".to_string()),
        is_permanent: Some(0),
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
        condition: None,
        end_time: None,
    };
    create_test_task(&state, &params);

    let due = state
        .user_repository
        .get_due_once_tasks(T)
        .expect("Failed to get due tasks");
    assert!(due.is_empty(), "Corrupted trigger should be filtered out");
}

#[test]
fn test_get_due_once_tasks_completed_not_returned() {
    // Completed tasks should not appear in due list (status filter)
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams::once_task(user.id, T - 60);
    let task = create_test_task(&state, &params);

    // Mark it completed
    state
        .user_repository
        .update_task_status(task.id.unwrap(), "completed")
        .expect("Failed to update status");

    let due = state
        .user_repository
        .get_due_once_tasks(T)
        .expect("Failed to get due tasks");
    assert!(due.is_empty(), "Completed task should not be due");
}

#[test]
fn test_get_due_once_tasks_cancelled_not_returned() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams::once_task(user.id, T - 60);
    let task = create_test_task(&state, &params);

    // Cancel it
    state
        .user_repository
        .cancel_task(user.id, task.id.unwrap())
        .expect("Failed to cancel task");

    let due = state
        .user_repository
        .get_due_once_tasks(T)
        .expect("Failed to get due tasks");
    assert!(due.is_empty(), "Cancelled task should not be due");
}

#[test]
fn test_get_due_once_tasks_mixed_due_and_future() {
    // Multiple tasks across users - only past ones returned
    let state = create_test_state();
    let user1 = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let user2 = create_test_user(&state, &TestUserParams::finland_user(10.0, 5.0));

    // User1: due task (past)
    create_test_task(&state, &TestTaskParams::once_task(user1.id, T - 120));
    // User1: future task
    create_test_task(&state, &TestTaskParams::once_task(user1.id, T + 3600));
    // User2: due task (past)
    create_test_task(&state, &TestTaskParams::once_task(user2.id, T - 30));
    // User2: recurring (should not appear)
    create_test_task(
        &state,
        &TestTaskParams {
            user_id: user2.id,
            trigger: "recurring_messaging".to_string(),
            action: "test_action".to_string(),
            notification_type: Some("sms".to_string()),
            is_permanent: Some(0),
            recurrence_rule: None,
            recurrence_time: None,
            sources: None,
            condition: Some("test".to_string()),
            end_time: None,
        },
    );

    let due = state
        .user_repository
        .get_due_once_tasks(T)
        .expect("Failed to get due tasks");
    assert_eq!(due.len(), 2, "Should find exactly 2 due tasks across users");
}

// =============================================================================
// Task Lifecycle - Corner Cases
// =============================================================================

#[test]
fn test_lifecycle_permanent_without_recurrence_rule() {
    // is_permanent=1 but recurrence_rule is None - has_recurrence is false, completes
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams {
        user_id: user.id,
        trigger: format!("once_{}", T - 60),
        action: "test_action".to_string(),
        notification_type: Some("sms".to_string()),
        is_permanent: Some(1),
        recurrence_rule: None, // Missing!
        recurrence_time: Some("09:00".to_string()),
        sources: None,
        condition: None,
        end_time: None,
    };
    let task = create_test_task(&state, &params);

    let rescheduled = state
        .user_repository
        .complete_or_reschedule_task(&task, "UTC")
        .expect("Failed to complete task");
    assert!(
        !rescheduled,
        "Permanent without recurrence_rule should complete, not reschedule"
    );

    let updated = get_task_by_id(&state, task.id.unwrap());
    assert_eq!(updated.status, Some("completed".to_string()));
}

#[test]
fn test_lifecycle_permanent_without_recurrence_time() {
    // is_permanent=1, has rule but no time - has_recurrence is false, completes
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams {
        user_id: user.id,
        trigger: format!("once_{}", T - 60),
        action: "test_action".to_string(),
        notification_type: Some("sms".to_string()),
        is_permanent: Some(1),
        recurrence_rule: Some("daily".to_string()),
        recurrence_time: None, // Missing!
        sources: None,
        condition: None,
        end_time: None,
    };
    let task = create_test_task(&state, &params);

    let rescheduled = state
        .user_repository
        .complete_or_reschedule_task(&task, "UTC")
        .expect("Failed to complete task");
    assert!(
        !rescheduled,
        "Permanent without recurrence_time should complete, not reschedule"
    );

    let updated = get_task_by_id(&state, task.id.unwrap());
    assert_eq!(updated.status, Some("completed".to_string()));
}

#[test]
fn test_lifecycle_is_permanent_none_treated_as_not_permanent() {
    // is_permanent=None (unwrap_or(0) == 0) - should complete
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams {
        user_id: user.id,
        trigger: format!("once_{}", T - 60),
        action: "test_action".to_string(),
        notification_type: Some("sms".to_string()),
        is_permanent: None, // NULL in DB
        recurrence_rule: Some("daily".to_string()),
        recurrence_time: Some("09:00".to_string()),
        sources: None,
        condition: None,
        end_time: None,
    };
    let task = create_test_task(&state, &params);

    let rescheduled = state
        .user_repository
        .complete_or_reschedule_task(&task, "UTC")
        .expect("Failed to complete task");
    assert!(
        !rescheduled,
        "is_permanent=None should be treated as non-permanent"
    );

    let updated = get_task_by_id(&state, task.id.unwrap());
    assert_eq!(updated.status, Some("completed".to_string()));
}

#[test]
fn test_lifecycle_task_with_no_id_errors() {
    // Task with id=None should return NotFound error
    let state = create_test_state();

    let task = backend::models::user_models::Task {
        id: None,
        user_id: 1,
        trigger: "once_0".to_string(),
        condition: None,
        action: "test".to_string(),
        notification_type: None,
        status: Some("active".to_string()),
        created_at: 0,
        completed_at: None,
        is_permanent: None,
        recurrence_rule: None,
        recurrence_time: None,
        sources: None,
        end_time: None,
    };

    let result = state
        .user_repository
        .complete_or_reschedule_task(&task, "UTC");
    assert!(result.is_err(), "Task with no id should error");
}

#[test]
fn test_lifecycle_completed_at_is_set_on_completion() {
    // Verify completed_at timestamp is populated when task completes
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let params = TestTaskParams::once_task(user.id, T - 60);
    let task = create_test_task(&state, &params);
    assert!(
        task.completed_at.is_none(),
        "Fresh task should have no completed_at"
    );

    // Bracket the completion call so completed_at is guaranteed between before and after
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    state
        .user_repository
        .complete_or_reschedule_task(&task, "UTC")
        .expect("Failed to complete task");
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let updated = get_task_by_id(&state, task.id.unwrap());
    assert_eq!(updated.status, Some("completed".to_string()));
    assert!(
        updated.completed_at.is_some(),
        "Completed task must have completed_at"
    );
    let completed_ts = updated.completed_at.unwrap();
    assert!(
        completed_ts >= before && completed_ts <= after,
        "completed_at ({}) should be between before ({}) and after ({})",
        completed_ts,
        before,
        after
    );
}
