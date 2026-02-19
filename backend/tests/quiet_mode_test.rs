//! Tests for quiet mode via MockUserCore stateful tests for set/get quiet mode.

use backend::test_utils::mock_user_core::MockUserCore;
use backend::UserCoreOps;

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
