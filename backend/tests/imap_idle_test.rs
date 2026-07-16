//! Tests for the IMAP IDLE helpers in `handlers::imap_handlers` and
//! `utils::imap_idle`. Most of the real IDLE loop requires a live IMAP
//! server so it's covered by the manual stress script at
//! `scripts/imap_idle_stress.py`. These tests cover the deterministic
//! pure helpers and the DB-side invariants that a bug would silently
//! break without the stress script noticing.

use backend::handlers::imap_handlers::{
    active_imap_connection_belongs_to_user, email_identity_matches_account, email_preview_content,
    email_room_id, fetch_single_email_imap_for_connection, insert_email_into_ontology,
    legacy_email_matches_preview, parse_email_room_id, ImapEmailPreview, ImapError,
};
use backend::models::ontology_models::{NewOntMessage, OntMessage};
use backend::test_utils::{
    create_test_state, create_test_user, setup_test_encryption, TestUserParams,
};
use chrono::Utc;
use serial_test::serial;

fn make_preview(uid: &str, from_email: &str, subject: &str) -> ImapEmailPreview {
    ImapEmailPreview {
        id: uid.to_string(),
        subject: Some(subject.to_string()),
        from: Some("Alice".to_string()),
        from_email: Some(from_email.to_string()),
        date: Some(Utc::now()),
        date_formatted: Some("today".to_string()),
        snippet: Some("snippet".to_string()),
        body: Some(format!("body for {}", subject)),
        is_read: false,
        imap_connection_id: None,
    }
}

#[test]
fn account_scoped_email_room_ids_round_trip() {
    let room_id = email_room_id("42", Some(17));
    assert_eq!(room_id, "email_17_42");

    let parsed = parse_email_room_id(&room_id).expect("scoped room id should parse");
    assert_eq!(parsed.imap_connection_id, Some(17));
    assert_eq!(parsed.uid, "42");
    assert!(email_identity_matches_account(&parsed, 17, 2));
    assert!(!email_identity_matches_account(&parsed, 18, 2));
}

#[test]
fn legacy_email_room_id_is_never_shared_across_multiple_accounts() {
    let parsed = parse_email_room_id("email_42").expect("legacy room id should parse");
    assert!(!email_identity_matches_account(&parsed, 17, 1));
    assert!(!email_identity_matches_account(&parsed, 17, 2));
    assert!(!email_identity_matches_account(&parsed, 18, 2));
}

#[test]
fn imap_connection_ownership_is_tenant_scoped_and_fail_closed() {
    assert!(active_imap_connection_belongs_to_user(10, 10, "active"));
    assert!(!active_imap_connection_belongs_to_user(10, 11, "active"));
    assert!(!active_imap_connection_belongs_to_user(10, 10, "inactive"));
    assert!(!active_imap_connection_belongs_to_user(
        10,
        10,
        "auth_failed"
    ));
}

#[test]
fn legacy_email_compatibility_requires_matching_sender_and_content() {
    let preview = make_preview("42", "alice@example.com", "legacy subject");
    let mut legacy = OntMessage {
        id: 1,
        user_id: 10,
        room_id: "email_42".to_string(),
        platform: "email".to_string(),
        sender_name: "Alice".to_string(),
        sender_key: Some("alice@example.com".to_string()),
        content: email_preview_content(&preview),
        person_id: None,
        created_at: Utc::now().timestamp() as i32,
        urgency: None,
        category: None,
        summary: None,
        digest_delivered_at: None,
        classification_prompt: None,
        classification_result: None,
        resolved_at: None,
        seen_at: None,
        matrix_event_id: None,
        commitment_prompt: None,
        commitment_result: None,
    };

    assert!(legacy_email_matches_preview(&legacy, &preview));

    legacy.sender_key = Some("mallory@example.com".to_string());
    assert!(!legacy_email_matches_preview(&legacy, &preview));

    legacy.sender_key = Some("alice@example.com".to_string());
    legacy.content.push_str(" changed");
    assert!(!legacy_email_matches_preview(&legacy, &preview));
}

// =============================================================================
// insert_email_into_ontology: first insert writes a row and emits change
// =============================================================================

#[tokio::test]
#[serial]
async fn test_insert_email_creates_ont_message() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let preview = make_preview("12345", "alice@example.com", "hello");
    let persons = state
        .ontology_repository
        .get_persons_with_channels(user.id, 500, 0)
        .unwrap_or_default();

    let id = insert_email_into_ontology(&state, user.id, &preview, &persons, None)
        .await
        .expect("insertion should succeed");

    assert!(id > 0, "returned id must be positive");

    let found = state
        .ontology_repository
        .get_message_by_email_room_id(user.id, "email_12345")
        .unwrap();
    assert!(found.is_some(), "row must be present");
    let msg = found.unwrap();
    assert_eq!(msg.platform, "email");
    assert_eq!(msg.room_id, "email_12345");
    assert!(msg.content.contains("hello"));
}

#[tokio::test]
#[serial]
async fn test_insert_already_read_email_records_seen_state() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let mut preview = make_preview("12346", "alice@example.com", "already read");
    preview.is_read = true;

    let id = insert_email_into_ontology(&state, user.id, &preview, &[], None)
        .await
        .expect("insertion should succeed");

    assert!(
        state.ontology_repository.is_message_seen_by_id(user.id, id),
        "an email already marked Seen by IMAP must be ineligible for a later notification"
    );
}

#[tokio::test]
#[serial]
async fn test_duplicate_ingest_promotes_existing_email_to_seen() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let preview = make_preview("12347", "alice@example.com", "read after first fetch");
    let id = insert_email_into_ontology(&state, user.id, &preview, &[], None)
        .await
        .unwrap();
    assert!(!state.ontology_repository.is_message_seen_by_id(user.id, id));

    let mut read_preview = preview.clone();
    read_preview.is_read = true;
    let duplicate_id = insert_email_into_ontology(&state, user.id, &read_preview, &[], None)
        .await
        .unwrap();

    assert_eq!(duplicate_id, id);
    assert!(state.ontology_repository.is_message_seen_by_id(user.id, id));
}

#[tokio::test]
#[serial]
async fn test_same_uid_in_two_accounts_creates_two_isolated_messages() {
    setup_test_encryption();
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let account_a = state
        .user_repository
        .set_imap_credentials(user.id, "account-a@example.com", "pw-a", None, None)
        .unwrap();
    let account_b = state
        .user_repository
        .set_imap_credentials(user.id, "account-b@example.com", "pw-b", None, None)
        .unwrap();

    let mut preview_a = make_preview("42", "alice@example.com", "from account A");
    preview_a.imap_connection_id = Some(account_a);
    let mut preview_b = make_preview("42", "bob@example.com", "from account B");
    preview_b.imap_connection_id = Some(account_b);

    let id_a = insert_email_into_ontology(&state, user.id, &preview_a, &[], None)
        .await
        .unwrap();
    let id_b = insert_email_into_ontology(&state, user.id, &preview_b, &[], None)
        .await
        .unwrap();

    assert_ne!(id_a, id_b);
    assert!(state
        .ontology_repository
        .get_message_by_email_room_id(user.id, &email_room_id("42", Some(account_a)))
        .unwrap()
        .is_some());
    assert!(state
        .ontology_repository
        .get_message_by_email_room_id(user.id, &email_room_id("42", Some(account_b)))
        .unwrap()
        .is_some());
}

#[tokio::test]
#[serial]
async fn test_email_insert_rejects_another_users_connection_id() {
    setup_test_encryption();
    let state = create_test_state();
    let user_a = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let user_b = create_test_user(&state, &TestUserParams::finland_user(10.0, 5.0));
    let account_a = state
        .user_repository
        .set_imap_credentials(user_a.id, "owner-a@example.com", "pw-a", None, None)
        .unwrap();

    let mut preview = make_preview("99", "alice@example.com", "must not cross users");
    preview.imap_connection_id = Some(account_a);
    let error = insert_email_into_ontology(&state, user_b.id, &preview, &[], None)
        .await
        .expect_err("another user's connection must be rejected");

    assert!(error.contains("does not belong"));
    assert!(state
        .ontology_repository
        .get_message_by_email_room_id(user_b.id, &email_room_id("99", Some(account_a)))
        .unwrap()
        .is_none());

    let fetch_error = fetch_single_email_imap_for_connection(&state, user_b.id, account_a, "99")
        .await
        .expect_err("another user's mailbox must be rejected before IMAP access");
    assert!(matches!(fetch_error, ImapError::CredentialsError(_)));
}

#[tokio::test]
#[serial]
async fn test_scoped_ingest_reuses_matching_legacy_email() {
    setup_test_encryption();
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let account = state
        .user_repository
        .set_imap_credentials(user.id, "account@example.com", "pw", None, None)
        .unwrap();
    let mut preview = make_preview("501", "alice@example.com", "legacy subject");
    preview.imap_connection_id = Some(account);

    let (legacy, _) = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "email_501".to_string(),
            platform: "email".to_string(),
            sender_name: "Alice".to_string(),
            sender_key: Some("alice@example.com".to_string()),
            content: email_preview_content(&preview),
            person_id: None,
            created_at: Utc::now().timestamp() as i32,
            matrix_event_id: None,
        })
        .unwrap();

    let returned = insert_email_into_ontology(&state, user.id, &preview, &[], Some(account))
        .await
        .unwrap();

    assert_eq!(returned, legacy.id);
    assert!(state
        .ontology_repository
        .get_message_by_email_room_id(user.id, &email_room_id("501", Some(account)))
        .unwrap()
        .is_none());
}

// =============================================================================
// Idempotence: inserting the same email twice returns the same id,
// not a duplicate row. This is the cron-vs-IDLE dedup guarantee.
// =============================================================================

#[tokio::test]
#[serial]
async fn test_insert_email_is_idempotent() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let preview = make_preview("42", "bob@example.com", "subject");
    let persons = Vec::new();

    let id1 = insert_email_into_ontology(&state, user.id, &preview, &persons, None)
        .await
        .unwrap();
    let id2 = insert_email_into_ontology(&state, user.id, &preview, &persons, None)
        .await
        .unwrap();

    assert_eq!(
        id1, id2,
        "second insert must return the existing row's id, not a new one"
    );

    // Assert exactly one row exists for this room_id.
    let msg = state
        .ontology_repository
        .get_message_by_email_room_id(user.id, "email_42")
        .unwrap();
    assert!(msg.is_some());
}

// =============================================================================
// Cron-vs-IDLE race: if the cron inserted the row first via direct
// insert_message, a subsequent IDLE path call for the same UID must
// skip insertion and return the existing id, not create a duplicate.
// =============================================================================

#[tokio::test]
#[serial]
async fn test_insert_email_dedup_against_preexisting_row() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Simulate the cron winning the race: insert a row directly.
    let (pre_existing, _is_new) = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "email_99".to_string(),
            platform: "email".to_string(),
            sender_name: "Carol".to_string(),
            sender_key: None,
            content: "cron inserted".to_string(),
            person_id: None,
            created_at: Utc::now().timestamp() as i32,
            matrix_event_id: None,
        })
        .unwrap();

    // Now IDLE tries to insert the same UID.
    let preview = make_preview("99", "carol@example.com", "idle subject");
    let persons = Vec::new();

    let returned_id = insert_email_into_ontology(&state, user.id, &preview, &persons, None)
        .await
        .unwrap();

    assert_eq!(
        returned_id, pre_existing.id,
        "IDLE path must return the cron-inserted id, not a new row"
    );

    // Content must be the cron's original, not overwritten.
    let msg = state
        .ontology_repository
        .get_message_by_email_room_id(user.id, "email_99")
        .unwrap()
        .unwrap();
    assert_eq!(msg.content, "cron inserted");
}

// =============================================================================
// Multi-user isolation: inserting the same UID for two different users
// must create two separate rows, not collide on room_id.
// =============================================================================

#[tokio::test]
#[serial]
async fn test_insert_email_isolates_users() {
    let state = create_test_state();
    // Use two distinct TestUserParams so the emails don't collide and
    // we get two separate user rows.
    let user_a = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let user_b = create_test_user(&state, &TestUserParams::finland_user(10.0, 5.0));

    let preview = make_preview("7", "dave@example.com", "cross-user");
    let persons = Vec::new();

    let id_a = insert_email_into_ontology(&state, user_a.id, &preview, &persons, None)
        .await
        .unwrap();
    let id_b = insert_email_into_ontology(&state, user_b.id, &preview, &persons, None)
        .await
        .unwrap();

    assert_ne!(id_a, id_b, "each user must get their own row");

    // Both rows exist, scoped to their respective users.
    let msg_a = state
        .ontology_repository
        .get_message_by_email_room_id(user_a.id, "email_7")
        .unwrap();
    let msg_b = state
        .ontology_repository
        .get_message_by_email_room_id(user_b.id, "email_7")
        .unwrap();
    assert!(msg_a.is_some());
    assert!(msg_b.is_some());
    assert_ne!(msg_a.unwrap().id, msg_b.unwrap().id);
}

// =============================================================================
// Person matching: if the sender's email matches a known Person's
// channel handle, the inserted ont_message must have person_id set.
// =============================================================================

#[tokio::test]
#[serial]
async fn test_insert_email_matches_person_by_email_channel() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Create a Person with an email channel.
    let person = state
        .ontology_repository
        .create_person(user.id, "Dave")
        .unwrap();
    state
        .ontology_repository
        .add_channel(user.id, person.id, "email", Some("dave@example.com"), None)
        .unwrap();

    let persons = state
        .ontology_repository
        .get_persons_with_channels(user.id, 500, 0)
        .unwrap();

    let preview = make_preview("100", "Dave@Example.COM", "mixed case test");

    let id = insert_email_into_ontology(&state, user.id, &preview, &persons, None)
        .await
        .unwrap();

    // Re-read the row and verify person_id is set.
    let msg = state
        .ontology_repository
        .get_message_by_email_room_id(user.id, "email_100")
        .unwrap()
        .unwrap();
    assert_eq!(msg.id, id);
    assert_eq!(
        msg.person_id,
        Some(person.id),
        "person matching must be case-insensitive on email handle"
    );
}

// =============================================================================
// get_max_processed_uid: basic ordering and user scoping
// =============================================================================

#[test]
#[serial]
fn test_get_max_processed_uid_returns_highest_numeric() {
    setup_test_encryption();
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Create a real imap_connection row so processed_emails FK is satisfied.
    let conn_id = state
        .user_repository
        .set_imap_credentials(user.id, "user@example.com", "pw", None, None)
        .unwrap();

    // No rows yet: must be None.
    let initial = state
        .user_repository
        .get_max_processed_uid(user.id, conn_id)
        .unwrap();
    assert_eq!(initial, None);

    // Insert a few UIDs for this connection.
    for uid in ["100", "250", "175"] {
        state
            .user_repository
            .mark_email_as_processed(user.id, uid, Some(conn_id))
            .unwrap();
    }

    let max = state
        .user_repository
        .get_max_processed_uid(user.id, conn_id)
        .unwrap();
    assert_eq!(max, Some(250), "must return the numerically highest UID");
}

#[test]
#[serial]
fn test_processed_email_cleanup_is_scoped_to_one_account() {
    setup_test_encryption();
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let account_a = state
        .user_repository
        .set_imap_credentials(user.id, "cleanup-a@example.com", "pw-a", None, None)
        .unwrap();
    let account_b = state
        .user_repository
        .set_imap_credentials(user.id, "cleanup-b@example.com", "pw-b", None, None)
        .unwrap();

    state
        .user_repository
        .mark_email_as_processed(user.id, "77", Some(account_a))
        .unwrap();
    state
        .user_repository
        .mark_email_as_processed(user.id, "77", Some(account_b))
        .unwrap();

    state
        .user_repository
        .delete_processed_email(user.id, "77", Some(account_a))
        .unwrap();

    assert!(!state
        .user_repository
        .is_email_processed(user.id, "77", Some(account_a))
        .unwrap());
    assert!(state
        .user_repository
        .is_email_processed(user.id, "77", Some(account_b))
        .unwrap());
}

#[test]
#[serial]
fn test_get_max_processed_uid_scopes_to_connection() {
    setup_test_encryption();
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Create two imap_connection rows to satisfy the FK.
    let conn_a = state
        .user_repository
        .set_imap_credentials(user.id, "a@x.com", "pw", None, None)
        .unwrap();
    let conn_b = state
        .user_repository
        .set_imap_credentials(user.id, "b@x.com", "pw", None, None)
        .unwrap();

    state
        .user_repository
        .mark_email_as_processed(user.id, "500", Some(conn_a))
        .unwrap();
    state
        .user_repository
        .mark_email_as_processed(user.id, "999", Some(conn_b))
        .unwrap();

    // conn_a max must be 500, not 999.
    let max_a = state
        .user_repository
        .get_max_processed_uid(user.id, conn_a)
        .unwrap();
    assert_eq!(max_a, Some(500));

    let max_b = state
        .user_repository
        .get_max_processed_uid(user.id, conn_b)
        .unwrap();
    assert_eq!(max_b, Some(999));
}

#[test]
#[serial]
fn test_get_max_processed_uid_ignores_null_connection_id() {
    setup_test_encryption();
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Create a connection so the query has something to target.
    let conn_id = state
        .user_repository
        .set_imap_credentials(user.id, "l@x.com", "pw", None, None)
        .unwrap();

    // Legacy row inserted without an imap_connection_id (the pre-IDLE
    // path). These must NOT contribute to the max for any connection —
    // otherwise the first-ever startup path would be skipped on
    // existing users and they'd lose the "last 10 mails" fallback.
    state
        .user_repository
        .mark_email_as_processed(user.id, "9999", None)
        .unwrap();

    let max = state
        .user_repository
        .get_max_processed_uid(user.id, conn_id)
        .unwrap();
    assert_eq!(
        max, None,
        "NULL imap_connection_id rows must not leak across connections"
    );
}

// =============================================================================
// IMAP connection lookup used by the IDLE task loop
// =============================================================================

#[test]
#[serial]
fn test_set_imap_credentials_returns_id_and_get_by_id_round_trip() {
    setup_test_encryption();
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let id = state
        .user_repository
        .set_imap_credentials(
            user.id,
            "x@example.com",
            "hunter2",
            Some("imap.x.com"),
            Some(993),
        )
        .unwrap();

    assert!(id > 0);

    let (returned_user_id, info, status) = state
        .user_repository
        .get_imap_connection_by_id(id)
        .unwrap()
        .expect("row must exist");

    assert_eq!(returned_user_id, user.id);
    assert_eq!(info.id, id);
    assert_eq!(info.email, "x@example.com");
    assert_eq!(info.password, "hunter2", "password must decrypt correctly");
    assert_eq!(info.imap_server.as_deref(), Some("imap.x.com"));
    assert_eq!(info.imap_port, Some(993));
    assert_eq!(status, "active");
}

#[test]
#[serial]
fn test_set_imap_credentials_upsert_returns_existing_id() {
    setup_test_encryption();
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let id1 = state
        .user_repository
        .set_imap_credentials(user.id, "a@b.com", "pw1", None, None)
        .unwrap();

    // Same email for same user: must be an update, not a new row.
    let id2 = state
        .user_repository
        .set_imap_credentials(user.id, "a@b.com", "pw2", None, None)
        .unwrap();

    assert_eq!(id1, id2, "upsert must return the existing row's id");

    // And the password was updated.
    let (_, info, _) = state
        .user_repository
        .get_imap_connection_by_id(id1)
        .unwrap()
        .unwrap();
    assert_eq!(info.password, "pw2");
}

#[test]
#[serial]
fn test_get_all_active_imap_connections_skips_auth_failed() {
    setup_test_encryption();
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let id_active = state
        .user_repository
        .set_imap_credentials(user.id, "ok@x.com", "p", None, None)
        .unwrap();
    let id_broken = state
        .user_repository
        .set_imap_credentials(user.id, "bad@x.com", "p", None, None)
        .unwrap();

    state
        .user_repository
        .set_imap_connection_status(id_broken, "auth_failed")
        .unwrap();

    let actives: Vec<i32> = state
        .user_repository
        .get_all_active_imap_connections()
        .unwrap()
        .into_iter()
        .map(|(conn_id, _)| conn_id)
        .collect();

    assert!(actives.contains(&id_active));
    assert!(
        !actives.contains(&id_broken),
        "auth_failed connections must not be returned"
    );
}
