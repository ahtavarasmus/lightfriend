use backend::utils::tuwunel_event_cleanup::{
    build_purge_history_url, build_purge_status_url, is_matrix_event_id,
    is_tuwunel_admin_redaction_reason, purge_history_request,
};
use serde_json::json;

#[test]
fn builds_encoded_room_history_purge_url() {
    assert_eq!(
        build_purge_history_url("http://localhost:8008/", "!room:localhost"),
        "http://localhost:8008/_synapse/admin/v1/purge_history/%21room%3Alocalhost"
    );
}

#[test]
fn builds_encoded_purge_status_url() {
    assert_eq!(
        build_purge_status_url("http://localhost:8008", "task/id"),
        "http://localhost:8008/_synapse/admin/v1/purge_history_status/task%2Fid"
    );
}

#[test]
fn purge_request_deletes_local_events_before_ingested_boundary() {
    assert_eq!(
        purge_history_request("$abc123:localhost"),
        json!({
            "purge_up_to_event_id": "$abc123:localhost",
            "delete_local_events": true
        })
    );
}

#[test]
fn validates_matrix_event_ids_without_command_shape_rules() {
    assert!(is_matrix_event_id("$abc123:localhost"));
    assert!(is_matrix_event_id("$opaque id:localhost"));
    assert!(!is_matrix_event_id("abc123:localhost"));
    assert!(!is_matrix_event_id("$abc123\n:localhost"));
}

#[test]
fn detects_legacy_tuwunel_admin_redaction_reason() {
    assert!(is_tuwunel_admin_redaction_reason(Some(
        "The administrator(s) of localhost has redacted this user's message."
    )));
    assert!(!is_tuwunel_admin_redaction_reason(Some(
        "Message deleted by source platform"
    )));
    assert!(!is_tuwunel_admin_redaction_reason(None));
}
