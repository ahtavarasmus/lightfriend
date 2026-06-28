use backend::utils::tuwunel_event_cleanup::{
    build_delete_media_by_event_command, build_redact_event_command, is_command_safe_event_id,
    is_tuwunel_admin_redaction_reason,
};

#[test]
fn builds_scoped_media_delete_by_event_admin_command() {
    let command = build_delete_media_by_event_command("$abc123:lightfriend.local");

    assert_eq!(
        command,
        "!admin media delete-by-event --event-id $abc123:lightfriend.local"
    );
    assert!(!command.contains("delete-range"));
}

#[test]
fn builds_scoped_redact_event_admin_command() {
    let command = build_redact_event_command("$abc123:lightfriend.local");

    assert_eq!(
        command,
        "!admin users redact-event $abc123:lightfriend.local"
    );
    assert!(!command.contains("delete-range"));
}

#[test]
fn rejects_event_ids_that_could_change_admin_command_shape() {
    assert!(is_command_safe_event_id("$abc123:lightfriend.local"));
    assert!(!is_command_safe_event_id(
        "$abc123:lightfriend.local --help"
    ));
    assert!(!is_command_safe_event_id("abc123:lightfriend.local"));
    assert!(!is_command_safe_event_id(
        "$abc123\n!admin media delete-range"
    ));
}

#[test]
fn detects_tuwunel_admin_redaction_reason() {
    assert!(is_tuwunel_admin_redaction_reason(Some(
        "The administrator(s) of lightfriend.local has redacted this user's message."
    )));
    assert!(!is_tuwunel_admin_redaction_reason(Some(
        "Message deleted by source platform"
    )));
    assert!(!is_tuwunel_admin_redaction_reason(None));
}
