//! DB tests for the redaction-driven message delete path used by
//! `handle_bridge_redaction` in `backend/src/utils/bridge.rs`.
//!
//! When a mautrix bridge propagates a "delete for everyone" from the source
//! platform, our redaction handler calls
//! `OntologyRepository::delete_message_by_matrix_event_id` to remove the
//! mirrored `ont_messages` row. These tests verify that path:
//!  - deletes the correct row when matrix_event_id matches,
//!  - leaves other rows untouched (different event_id, different user),
//!  - is idempotent (deleting an already-deleted event returns 0).

use backend::models::ontology_models::NewOntMessage;
use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use serial_test::serial;

fn new_msg(user_id: i32, content: &str, matrix_event_id: Option<&str>) -> NewOntMessage {
    NewOntMessage {
        user_id,
        room_id: "!room:example.com".to_string(),
        platform: "whatsapp".to_string(),
        sender_name: "Alice".to_string(),
        sender_key: None,
        content: content.to_string(),
        person_id: None,
        created_at: 1_000_000,
        matrix_event_id: matrix_event_id.map(|s| s.to_string()),
    }
}

#[test]
#[serial]
fn deletes_matching_row_and_returns_count() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let (target, _) = state
        .ontology_repository
        .insert_message(&new_msg(
            user.id,
            "to be redacted",
            Some("$evt-a:example.com"),
        ))
        .unwrap();
    let (keeper, _) = state
        .ontology_repository
        .insert_message(&new_msg(user.id, "survives", Some("$evt-b:example.com")))
        .unwrap();

    let deleted = state
        .ontology_repository
        .delete_message_by_matrix_event_id(user.id, "$evt-a:example.com")
        .unwrap();
    assert_eq!(deleted, 1, "should delete exactly the targeted row");

    let remaining = state
        .ontology_repository
        .get_messages_for_room(user.id, "!room:example.com", 100)
        .unwrap();
    let remaining_ids: Vec<i64> = remaining.iter().map(|m| m.id).collect();
    assert!(
        !remaining_ids.contains(&target.id),
        "target row should be gone"
    );
    assert!(
        remaining_ids.contains(&keeper.id),
        "non-targeted row should still be present"
    );
}

#[test]
#[serial]
fn delete_is_idempotent_and_safe_for_unknown_event_ids() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    state
        .ontology_repository
        .insert_message(&new_msg(user.id, "hi", Some("$evt:example.com")))
        .unwrap();

    let first = state
        .ontology_repository
        .delete_message_by_matrix_event_id(user.id, "$evt:example.com")
        .unwrap();
    assert_eq!(first, 1);

    // Second call on the same event_id: nothing left to delete.
    let second = state
        .ontology_repository
        .delete_message_by_matrix_event_id(user.id, "$evt:example.com")
        .unwrap();
    assert_eq!(second, 0);

    // Unknown event_id: no error, no rows touched.
    let unknown = state
        .ontology_repository
        .delete_message_by_matrix_event_id(user.id, "$never-seen:example.com")
        .unwrap();
    assert_eq!(unknown, 0);
}

#[test]
#[serial]
fn delete_is_scoped_per_user() {
    // Two users happen to have rows tagged with the same Matrix event_id.
    // This is theoretically impossible if matrix_event_ids are globally
    // unique, but our partial unique index is scoped to (user_id,
    // matrix_event_id), so we must scope deletes the same way to avoid
    // ever clobbering a different user's row.
    let state = create_test_state();
    let user_a = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let user_b = create_test_user(&state, &TestUserParams::finland_user(10.0, 5.0));

    let shared_event = "$shared-event:example.com";
    let (msg_a, _) = state
        .ontology_repository
        .insert_message(&new_msg(user_a.id, "from A", Some(shared_event)))
        .unwrap();
    let (msg_b, _) = state
        .ontology_repository
        .insert_message(&new_msg(user_b.id, "from B", Some(shared_event)))
        .unwrap();

    let deleted = state
        .ontology_repository
        .delete_message_by_matrix_event_id(user_a.id, shared_event)
        .unwrap();
    assert_eq!(deleted, 1);

    // User A's row gone, user B's row preserved.
    let a_rows = state
        .ontology_repository
        .get_messages_for_room(user_a.id, "!room:example.com", 100)
        .unwrap();
    assert!(!a_rows.iter().any(|m| m.id == msg_a.id));

    let b_rows = state
        .ontology_repository
        .get_messages_for_room(user_b.id, "!room:example.com", 100)
        .unwrap();
    assert!(b_rows.iter().any(|m| m.id == msg_b.id));
}
