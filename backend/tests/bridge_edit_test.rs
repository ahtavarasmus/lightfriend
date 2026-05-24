//! Verifies that the `m.replace` (edit) detection in `handle_bridge_message`
//! correctly identifies edits emitted by mautrix-style bridges.
//!
//! Real-world trigger: a user edits a message on WhatsApp within the 15-minute
//! edit window. mautrix-whatsapp bridges that as a Matrix `m.room.message`
//! event whose content carries `m.relates_to.rel_type = "m.replace"`. Without
//! the gate, our ingest treats it as a fresh message and creates a duplicate
//! `ont_messages` row. The gate matches `Relation::Replacement` and returns
//! before any DB write.
//!
//! These tests deserialize the raw JSON payload (same shape the homeserver
//! delivers) and assert the SDK + our pattern still classify it correctly.

use matrix_sdk::ruma::events::room::message::{Relation, RoomMessageEventContent};

#[test]
fn plain_message_is_not_a_replacement() {
    let json = serde_json::json!({
        "msgtype": "m.text",
        "body": "hello world",
    });
    let content: RoomMessageEventContent = serde_json::from_value(json).unwrap();
    assert!(
        !matches!(content.relates_to.as_ref(), Some(Relation::Replacement(_))),
        "plain message should not be flagged as m.replace edit"
    );
}

#[test]
fn replace_event_is_detected() {
    let json = serde_json::json!({
        "msgtype": "m.text",
        "body": "* edited body",
        "m.new_content": {
            "msgtype": "m.text",
            "body": "edited body",
        },
        "m.relates_to": {
            "rel_type": "m.replace",
            "event_id": "$original-event-id:example.com",
        },
    });
    let content: RoomMessageEventContent = serde_json::from_value(json).unwrap();
    assert!(
        matches!(content.relates_to.as_ref(), Some(Relation::Replacement(_))),
        "m.replace event should match Relation::Replacement"
    );
}

#[test]
fn reply_relation_is_not_a_replacement() {
    // Replies use `m.in_reply_to`, not `m.replace`. Our gate must not
    // accidentally drop replies.
    let json = serde_json::json!({
        "msgtype": "m.text",
        "body": "> quoted\n\nactual reply",
        "m.relates_to": {
            "m.in_reply_to": {
                "event_id": "$prior:example.com",
            },
        },
    });
    let content: RoomMessageEventContent = serde_json::from_value(json).unwrap();
    assert!(
        !matches!(content.relates_to.as_ref(), Some(Relation::Replacement(_))),
        "reply should not be flagged as m.replace edit"
    );
}
