//! Unit tests for action executor.
//!
//! Tests action parsing, source fetching, condition evaluation,
//! and the execution pipeline.
//!
//! Note: Some tests are marked as integration tests since they would
//! require mocking external services (AI, Twilio). These are structured
//! to document expected behavior.

use backend::models::user_models::{NewContactProfile, NewTask};
use backend::test_utils::{
    create_test_state, create_test_task, create_test_user, TestTaskParams, TestUserParams,
};
use backend::utils::action_executor::{
    is_sender_trusted, parse_action_structured, ActionResult, SenderContext, StructuredAction,
};

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
// Action Parsing - New JSON Format
// =============================================================================

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
// parse_action_structured Tests
// =============================================================================

#[test]
fn test_parse_structured_json_format() {
    let action = r#"{"tool":"send_reminder","params":{"message":"Call mom"}}"#;
    let result = parse_action_structured(action);
    assert_eq!(result.tool, "send_reminder");
    let msg = result.params.unwrap();
    assert_eq!(msg["message"], "Call mom");
}

#[test]
fn test_parse_structured_old_format() {
    let result = parse_action_structured("send_reminder(Call mom)");
    assert_eq!(result.tool, "send_reminder");
    let params = result.params.unwrap();
    assert_eq!(params["message"], "Call mom");
}

#[test]
fn test_parse_structured_old_tesla() {
    let result = parse_action_structured("control_tesla(climate_on)");
    assert_eq!(result.tool, "control_tesla");
    let params = result.params.unwrap();
    assert_eq!(params["command"], "climate_on");
}

#[test]
fn test_parse_structured_simple_tool() {
    let result = parse_action_structured("generate_digest");
    assert_eq!(result.tool, "generate_digest");
    assert!(result.params.is_none());
}

#[test]
fn test_parse_structured_empty() {
    let result = parse_action_structured("");
    assert_eq!(result.tool, "");
    assert!(result.params.is_none());
}

#[test]
fn test_structured_to_action_string() {
    let action = StructuredAction {
        tool: "send_reminder".to_string(),
        params: Some(serde_json::json!({"message": "Call mom"})),
    };
    assert_eq!(action.to_action_string(), "send_reminder(Call mom)");
}

#[test]
fn test_structured_to_action_string_no_params() {
    let action = StructuredAction {
        tool: "generate_digest".to_string(),
        params: None,
    };
    assert_eq!(action.to_action_string(), "generate_digest");
}

#[test]
fn test_roundtrip_json_serialize() {
    let action = StructuredAction {
        tool: "control_tesla".to_string(),
        params: Some(serde_json::json!({"command": "climate_on"})),
    };
    let json = serde_json::to_string(&action).unwrap();
    let parsed = parse_action_structured(&json);
    assert_eq!(parsed.tool, "control_tesla");
    assert_eq!(parsed.params.unwrap()["command"], "climate_on");
}

// =============================================================================
// Sender Trust Tier Tests
// =============================================================================

#[test]
fn test_registry_restricted_tools() {
    let registry = backend::build_tool_registry();
    // These tools perform external actions and should be restricted
    assert!(registry.is_restricted("send_email"));
    assert!(registry.is_restricted("respond_to_email"));
    assert!(registry.is_restricted("send_chat_message"));
    assert!(registry.is_restricted("control_tesla"));
    // Safe/read-only tools should NOT be restricted
    assert!(!registry.is_restricted("get_weather"));
    assert!(!registry.is_restricted("fetch_calendar_events"));
    assert!(!registry.is_restricted("fetch_emails"));
    assert!(!registry.is_restricted("direct_response"));
    // Unknown tools should not be restricted (fall through)
    assert!(!registry.is_restricted("nonexistent_tool"));
}

#[test]
fn test_sender_trust_time_based_always_trusted() {
    let state = create_test_state();
    assert!(is_sender_trusted(&state, 999, &SenderContext::TimeBased));
}

#[test]
fn test_sender_trust_email_no_profiles() {
    let state = create_test_state();
    // No contact profiles exist, so any email sender is untrusted
    let trusted = is_sender_trusted(
        &state,
        999,
        &SenderContext::Email {
            from_email: "attacker@evil.com",
            from_display: "Attacker",
        },
    );
    assert!(!trusted);
}

#[test]
fn test_sender_trust_email_matching_profile() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let profile = NewContactProfile {
        user_id: user.id,
        nickname: "Mom".to_string(),
        whatsapp_chat: None,
        telegram_chat: None,
        signal_chat: None,
        email_addresses: Some("mom@family.com".to_string()),
        notification_mode: "all".to_string(),
        notification_type: "sms".to_string(),
        notify_on_call: 0,
        created_at: 0,
        whatsapp_room_id: None,
        telegram_room_id: None,
        signal_room_id: None,
        notes: None,
    };
    state
        .user_repository
        .create_contact_profile(&profile)
        .unwrap();

    // Matching sender is trusted
    assert!(is_sender_trusted(
        &state,
        user.id,
        &SenderContext::Email {
            from_email: "mom@family.com",
            from_display: "Mom",
        },
    ));

    // Non-matching sender is untrusted
    assert!(!is_sender_trusted(
        &state,
        user.id,
        &SenderContext::Email {
            from_email: "stranger@unknown.com",
            from_display: "Stranger",
        },
    ));
}

#[test]
fn test_sender_trust_email_case_insensitive() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let profile = NewContactProfile {
        user_id: user.id,
        nickname: "Boss".to_string(),
        whatsapp_chat: None,
        telegram_chat: None,
        signal_chat: None,
        email_addresses: Some("Boss@Work.com".to_string()),
        notification_mode: "all".to_string(),
        notification_type: "sms".to_string(),
        notify_on_call: 0,
        created_at: 0,
        whatsapp_room_id: None,
        telegram_room_id: None,
        signal_room_id: None,
        notes: None,
    };
    state
        .user_repository
        .create_contact_profile(&profile)
        .unwrap();

    assert!(is_sender_trusted(
        &state,
        user.id,
        &SenderContext::Email {
            from_email: "boss@work.com",
            from_display: "boss",
        },
    ));
}

#[test]
fn test_sender_trust_email_multiple_addresses() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let profile = NewContactProfile {
        user_id: user.id,
        nickname: "Partner".to_string(),
        whatsapp_chat: None,
        telegram_chat: None,
        signal_chat: None,
        email_addresses: Some("partner@gmail.com, partner@work.com".to_string()),
        notification_mode: "all".to_string(),
        notification_type: "sms".to_string(),
        notify_on_call: 0,
        created_at: 0,
        whatsapp_room_id: None,
        telegram_room_id: None,
        signal_room_id: None,
        notes: None,
    };
    state
        .user_repository
        .create_contact_profile(&profile)
        .unwrap();

    // Both addresses should match
    assert!(is_sender_trusted(
        &state,
        user.id,
        &SenderContext::Email {
            from_email: "partner@gmail.com",
            from_display: "",
        },
    ));
    assert!(is_sender_trusted(
        &state,
        user.id,
        &SenderContext::Email {
            from_email: "partner@work.com",
            from_display: "",
        },
    ));
}

#[test]
fn test_sender_trust_messaging_by_room_id() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let profile = NewContactProfile {
        user_id: user.id,
        nickname: "Alice".to_string(),
        whatsapp_chat: Some("Alice (WA)".to_string()),
        telegram_chat: None,
        signal_chat: None,
        email_addresses: None,
        notification_mode: "all".to_string(),
        notification_type: "sms".to_string(),
        notify_on_call: 0,
        created_at: 0,
        whatsapp_room_id: Some("!room123:matrix.org".to_string()),
        telegram_room_id: None,
        signal_room_id: None,
        notes: None,
    };
    state
        .user_repository
        .create_contact_profile(&profile)
        .unwrap();

    // Matching room_id is trusted
    assert!(is_sender_trusted(
        &state,
        user.id,
        &SenderContext::Messaging {
            service: "whatsapp",
            room_id: "!room123:matrix.org",
            room_name: "Alice",
        },
    ));

    // Non-matching room_id and name is untrusted
    assert!(!is_sender_trusted(
        &state,
        user.id,
        &SenderContext::Messaging {
            service: "whatsapp",
            room_id: "!different:matrix.org",
            room_name: "UnknownPerson",
        },
    ));
}

#[test]
fn test_sender_trust_messaging_chat_name_fallback() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let profile = NewContactProfile {
        user_id: user.id,
        nickname: "Bob".to_string(),
        whatsapp_chat: None,
        telegram_chat: Some("Bob Smith (Telegram)".to_string()),
        signal_chat: None,
        email_addresses: None,
        notification_mode: "all".to_string(),
        notification_type: "sms".to_string(),
        notify_on_call: 0,
        created_at: 0,
        whatsapp_room_id: None,
        telegram_room_id: None, // No room_id - forces name fallback
        signal_room_id: None,
        notes: None,
    };
    state
        .user_repository
        .create_contact_profile(&profile)
        .unwrap();

    // Name match should work even without room_id
    assert!(is_sender_trusted(
        &state,
        user.id,
        &SenderContext::Messaging {
            service: "telegram",
            room_id: "!unknown:matrix.org",
            room_name: "Bob Smith",
        },
    ));

    // Wrong service should not match
    assert!(!is_sender_trusted(
        &state,
        user.id,
        &SenderContext::Messaging {
            service: "whatsapp",
            room_id: "!unknown:matrix.org",
            room_name: "Bob Smith",
        },
    ));
}

// =============================================================================
// Sender Trust - execute_action_spec integration tests
// =============================================================================

#[tokio::test]
async fn test_untrusted_sender_blocked_from_restricted_action() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Untrusted sender tries to send email via action spec
    let action_spec =
        r#"{"tool":"send_email","params":{"to":"victim@example.com","subject":"hi"}}"#;
    let result = backend::utils::action_executor::execute_action_spec(
        &state,
        user.id,
        action_spec,
        "sms",
        None,
        None,
        None,
        false, // untrusted sender
    )
    .await;

    match result {
        ActionResult::Failed { error } => {
            assert!(
                error.contains("blocked"),
                "Error should mention 'blocked': {}",
                error
            );
            assert!(
                error.contains("send_email"),
                "Error should mention tool name: {}",
                error
            );
        }
        ActionResult::Success { message } => {
            panic!("Expected ActionResult::Failed, got Success: {}", message)
        }
        ActionResult::Skipped { reason } => {
            panic!("Expected ActionResult::Failed, got Skipped: {}", reason)
        }
    }
}

#[tokio::test]
async fn test_untrusted_sender_allowed_safe_action() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Untrusted sender can use send_reminder (not restricted)
    let action_spec = r#"{"tool":"send_reminder","params":{"message":"Call mom"}}"#;
    let result = backend::utils::action_executor::execute_action_spec(
        &state,
        user.id,
        action_spec,
        "sms",
        None,
        None,
        None,
        false, // untrusted sender
    )
    .await;

    match result {
        ActionResult::Success { message } => {
            assert_eq!(message, "Call mom");
        }
        ActionResult::Failed { error } => {
            panic!("Expected ActionResult::Success, got Failed: {}", error)
        }
        ActionResult::Skipped { reason } => {
            panic!("Expected ActionResult::Success, got Skipped: {}", reason)
        }
    }
}

#[tokio::test]
async fn test_trusted_sender_allowed_restricted_action() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Trusted sender tries control_tesla - should NOT get "blocked" error.
    // May fail for other reasons (no tesla credentials), but trust gate passes.
    let action_spec = r#"{"tool":"control_tesla","params":{"command":"status"}}"#;
    let result = backend::utils::action_executor::execute_action_spec(
        &state,
        user.id,
        action_spec,
        "sms",
        None,
        None,
        None,
        true, // trusted sender
    )
    .await;

    if let ActionResult::Failed { error } = result {
        assert!(
            !error.contains("blocked"),
            "Trusted sender should not be blocked: {}",
            error
        );
    }
}
