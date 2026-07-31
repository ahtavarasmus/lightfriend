use backend::repositories::bridge_login_repository::bridge_database_env_name;
use backend::utils::disconnected_bridge_cleanup::{
    bridge_cleanup_execution_allowed, build_delete_room_url, build_room_members_url,
    orphan_absence_ready_for_verification, ORPHAN_LOGIN_ABSENCE_OBSERVED,
    ORPHAN_LOGIN_ABSENT_VERIFIED,
};
use backend::utils::tuwunel_event_cleanup::{
    available_purge_submission_slots, build_purge_history_url, build_purge_status_url,
    build_server_rooms_url, historical_backfill_execution_kind,
    historical_backfill_requires_proof_scan, historical_event_requires_proof, is_matrix_event_id,
    is_tuwunel_admin_redaction_reason, next_backfill_scan_timestamp, parse_server_room_list_page,
    purge_history_request, purge_history_timestamp_request, purge_task_status_is_missing,
    room_history_error_is_already_clean, select_portal_census_room_batch,
};
use reqwest::StatusCode;
use serde_json::json;

#[test]
fn builds_encoded_room_history_purge_url() {
    assert_eq!(
        build_purge_history_url("http://localhost:8008/", "!room:localhost"),
        "http://localhost:8008/_synapse/admin/v1/purge_history/%21room%3Alocalhost"
    );
}

#[test]
fn builds_paginated_server_room_census_url() {
    assert_eq!(
        build_server_rooms_url("http://localhost:8008/", 5000, 5000),
        "http://localhost:8008/_synapse/admin/v1/rooms?from=5000&limit=5000"
    );
}

#[test]
fn parses_server_room_census_page() {
    let body = json!({
        "rooms": [
            {"room_id": "!a:localhost", "name": "A"},
            {"room_id": "!b:localhost", "joined_members": 2}
        ],
        "offset": 0,
        "total_rooms": 3,
        "next_batch": 2,
        "prev_batch": null
    })
    .to_string();

    let (rooms, offset, total, next) = parse_server_room_list_page(&body).unwrap();
    assert_eq!(rooms, vec!["!a:localhost", "!b:localhost"]);
    assert_eq!(offset, 0);
    assert_eq!(total, 3);
    assert_eq!(next, Some(2));
}

#[test]
fn rejects_invalid_room_ids_from_server_census() {
    let body = json!({
        "rooms": [{"room_id": "not-a-room"}],
        "offset": 0,
        "total_rooms": 1,
        "next_batch": null
    })
    .to_string();

    assert!(parse_server_room_list_page(&body).is_err());
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
fn disconnected_room_execution_requires_portal_proof_and_separate_orphan_permission() {
    assert!(bridge_cleanup_execution_allowed(
        "explicit_disconnect",
        "confirmed",
        true,
        false,
    ));
    assert!(!bridge_cleanup_execution_allowed(
        "explicit_disconnect",
        "pending",
        true,
        false,
    ));
    assert!(!bridge_cleanup_execution_allowed(
        "orphan_audit",
        "legacy_unverified",
        true,
        true,
    ));
    assert!(!bridge_cleanup_execution_allowed(
        "orphan_audit",
        ORPHAN_LOGIN_ABSENT_VERIFIED,
        true,
        false,
    ));
    assert!(bridge_cleanup_execution_allowed(
        "orphan_audit",
        ORPHAN_LOGIN_ABSENT_VERIFIED,
        true,
        true,
    ));
}

#[test]
fn orphan_deletion_requires_two_separated_absence_observations() {
    assert!(!orphan_absence_ready_for_verification(
        "legacy_unverified",
        None,
        1_000,
        300,
    ));
    assert!(!orphan_absence_ready_for_verification(
        ORPHAN_LOGIN_ABSENCE_OBSERVED,
        Some(900),
        1_000,
        300,
    ));
    assert!(orphan_absence_ready_for_verification(
        ORPHAN_LOGIN_ABSENCE_OBSERVED,
        Some(700),
        1_000,
        300,
    ));
}

#[test]
fn maps_supported_bridges_to_separate_login_databases() {
    assert_eq!(
        bridge_database_env_name("whatsapp"),
        Some("WHATSAPP_BRIDGE_DATABASE_URL")
    );
    assert_eq!(
        bridge_database_env_name("signal"),
        Some("SIGNAL_BRIDGE_DATABASE_URL")
    );
    assert_eq!(
        bridge_database_env_name("telegram"),
        Some("TELEGRAM_BRIDGE_DATABASE_URL")
    );
    assert_eq!(bridge_database_env_name("unknown"), None);
}

#[test]
fn forced_historical_policy_skips_expensive_proof_scan() {
    assert!(!historical_backfill_requires_proof_scan(true, true));
    assert!(historical_backfill_requires_proof_scan(true, false));
    assert!(historical_backfill_requires_proof_scan(false, true));
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
fn forced_historical_purge_uses_timestamp_when_event_boundary_is_missing() {
    assert_eq!(
        purge_history_timestamp_request(1_784_889_600_000),
        json!({
            "purge_up_to_ts": 1_784_889_600_000_u64,
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
fn purge_submission_slots_enforce_a_global_concurrency_limit() {
    assert_eq!(available_purge_submission_slots(4, 0), 4);
    assert_eq!(available_purge_submission_slots(4, 3), 1);
    assert_eq!(available_purge_submission_slots(4, 4), 0);
    assert_eq!(available_purge_submission_slots(4, 346), 0);
    assert_eq!(available_purge_submission_slots(4, -1), 4);
}

#[test]
fn missing_tuwunel_task_status_is_restart_recoverable() {
    assert!(purge_task_status_is_missing(Some(StatusCode::NOT_FOUND)));
    assert!(!purge_task_status_is_missing(Some(
        StatusCode::INTERNAL_SERVER_ERROR
    )));
    assert!(!purge_task_status_is_missing(None));
}

#[test]
fn no_event_before_timestamp_is_an_already_clean_room() {
    assert!(room_history_error_is_already_clean(
        Some(StatusCode::NOT_FOUND),
        r#"{"errcode":"M_NOT_FOUND","error":"No event found before the given timestamp"}"#,
    ));
    assert!(!room_history_error_is_already_clean(
        Some(StatusCode::NOT_FOUND),
        r#"{"errcode":"M_NOT_FOUND","error":"Unknown room"}"#,
    ));
    assert!(!room_history_error_is_already_clean(
        Some(StatusCode::INTERNAL_SERVER_ERROR),
        "No event found before the given timestamp",
    ));
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

#[test]
fn historical_backfill_keeps_verified_and_forced_paths_distinct() {
    assert_eq!(
        historical_backfill_execution_kind(true, true, true, true),
        Some("historical_backfill_verified")
    );
    assert_eq!(
        historical_backfill_execution_kind(false, true, true, true),
        Some("historical_backfill_forced_unverified")
    );
    assert_eq!(
        historical_backfill_execution_kind(false, true, true, false),
        None
    );
    assert_eq!(
        historical_backfill_execution_kind(false, false, true, true),
        None
    );
}

#[test]
fn portal_census_cursor_walks_every_room_without_repeating_a_batch() {
    let rooms = ["!a:localhost", "!b:localhost", "!c:localhost"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

    let (first, cursor) = select_portal_census_room_batch(&rooms, None, 2);
    assert_eq!(first, vec!["!a:localhost", "!b:localhost"]);
    assert_eq!(cursor.as_deref(), Some("!b:localhost"));

    let (second, cursor) = select_portal_census_room_batch(&rooms, cursor.as_deref(), 2);
    assert_eq!(second, vec!["!c:localhost"]);
    assert_eq!(cursor, None);
}

#[test]
fn portal_census_cursor_resets_after_a_full_room_scan() {
    let rooms = ["!a:localhost", "!b:localhost"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let (selected, cursor) = select_portal_census_room_batch(&rooms, None, 100);
    assert_eq!(selected, rooms);
    assert_eq!(cursor, None);
}
