use backend::utils::disconnected_bridge_cleanup::{build_delete_room_url, build_room_members_url};
use backend::utils::tuwunel_event_cleanup::{
    build_purge_history_url, build_purge_status_url, historical_event_requires_proof,
    is_matrix_event_id, is_tuwunel_admin_redaction_reason, next_backfill_scan_timestamp,
    purge_history_request,
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
fn builds_encoded_disconnected_room_admin_urls() {
    assert_eq!(
        build_room_members_url("http://localhost:8008/", "!room:localhost"),
        "http://localhost:8008/_synapse/admin/v1/rooms/%21room%3Alocalhost/members"
    );
    assert_eq!(
        build_delete_room_url("http://localhost:8008", "!room:localhost"),
        "http://localhost:8008/_synapse/admin/v1/rooms/%21room%3Alocalhost"
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

#[test]
fn historical_backfill_drains_full_batches_at_worker_poll_speed() {
    assert_eq!(
        next_backfill_scan_timestamp(1_000, 25, 25, 30, 3_600),
        1_030
    );
    assert_eq!(
        next_backfill_scan_timestamp(1_000, 24, 25, 30, 3_600),
        4_600
    );
}

#[test]
fn historical_backfill_timestamp_saturates() {
    assert_eq!(
        next_backfill_scan_timestamp(i32::MAX - 5, 25, 25, 30, 3_600),
        i32::MAX
    );
}

#[test]
fn historical_audit_requires_proof_for_payload_events() {
    assert!(historical_event_requires_proof("m.room.message", false));
    assert!(historical_event_requires_proof("m.room.encrypted", false));
    assert!(historical_event_requires_proof("m.sticker", false));
    assert!(historical_event_requires_proof(
        "com.example.bridge_payload",
        false
    ));
}

#[test]
fn historical_audit_allows_state_and_non_payload_housekeeping() {
    assert!(!historical_event_requires_proof("m.room.create", true));
    assert!(!historical_event_requires_proof("m.room.member", true));
    assert!(!historical_event_requires_proof("m.reaction", false));
    assert!(!historical_event_requires_proof("m.room.redaction", false));
}
