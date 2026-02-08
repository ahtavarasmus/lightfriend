//! Tests for quiet mode via the tasks system.
//!
//! Pure function tests for is_quiet_mode_task helper and
//! MockUserCore stateful tests for set/get quiet mode.

use backend::handlers::dashboard_handlers::{is_digest_task, is_quiet_mode_task};
use backend::test_utils::mock_user_core::MockUserCore;
use backend::UserCoreOps;

// =============================================================================
// Pure function tests: is_quiet_mode_task
// =============================================================================

#[test]
fn test_is_quiet_mode_task_json_format() {
    assert!(is_quiet_mode_task(r#"{"tool":"quiet_mode"}"#));
}

#[test]
fn test_is_quiet_mode_task_old_format() {
    assert!(is_quiet_mode_task("quiet_mode"));
}

#[test]
fn test_is_quiet_mode_task_regular_task() {
    assert!(!is_quiet_mode_task(
        r#"{"tool":"send_reminder","params":{"message":"hello"}}"#
    ));
}

#[test]
fn test_is_quiet_mode_task_digest() {
    assert!(!is_quiet_mode_task(r#"{"tool":"generate_digest"}"#));
}

#[test]
fn test_is_quiet_mode_task_empty() {
    assert!(!is_quiet_mode_task(""));
}

// Verify is_digest_task doesn't match quiet_mode
#[test]
fn test_is_digest_task_does_not_match_quiet_mode() {
    assert!(!is_digest_task(r#"{"tool":"quiet_mode"}"#));
    assert!(!is_digest_task("quiet_mode"));
}

// =============================================================================
// MockUserCore stateful tests
// =============================================================================

#[test]
fn test_mock_set_and_get_quiet_mode_timed() {
    let mock = MockUserCore::new();
    let user_id = 1;
    let until_ts = 1707300000;

    mock.set_quiet_mode(user_id, Some(until_ts)).unwrap();
    let result = mock.get_quiet_mode(user_id).unwrap();
    assert_eq!(result, Some(until_ts));
}

#[test]
fn test_mock_set_and_get_quiet_mode_indefinite() {
    let mock = MockUserCore::new();
    let user_id = 1;

    mock.set_quiet_mode(user_id, Some(0)).unwrap();
    let result = mock.get_quiet_mode(user_id).unwrap();
    assert_eq!(result, Some(0));
}

#[test]
fn test_mock_disable_quiet_mode() {
    let mock = MockUserCore::new();
    let user_id = 1;

    // Enable then disable
    mock.set_quiet_mode(user_id, Some(0)).unwrap();
    mock.set_quiet_mode(user_id, None).unwrap();
    let result = mock.get_quiet_mode(user_id).unwrap();
    assert_eq!(result, None);
}

#[test]
fn test_mock_quiet_mode_default_off() {
    let mock = MockUserCore::new();
    let result = mock.get_quiet_mode(1).unwrap();
    assert_eq!(result, None);
}
