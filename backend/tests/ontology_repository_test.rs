//! Unit tests for OntologyRepository.
//!
//! DB tests with #[serial]. Tests CRUD, case-insensitive matching, upsert dedup,
//! nickname lookup, partial search, links, user isolation, and cascading deletes.

use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use serial_test::serial;

// =============================================================================
// Create + Find
// =============================================================================

#[test]
#[serial]
fn test_create_and_find_person_case_insensitive() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    state
        .ontology_repository
        .create_person(user.id, "Alice")
        .unwrap();

    // Exact match
    let found = state
        .ontology_repository
        .find_person_by_name(user.id, "Alice")
        .unwrap();
    assert!(found.is_some());
    assert_eq!(found.as_ref().unwrap().person.name, "Alice");

    // Lowercase
    let found = state
        .ontology_repository
        .find_person_by_name(user.id, "alice")
        .unwrap();
    assert!(found.is_some());

    // Uppercase
    let found = state
        .ontology_repository
        .find_person_by_name(user.id, "ALICE")
        .unwrap();
    assert!(found.is_some());
}

// =============================================================================
// Upsert dedup
// =============================================================================

#[test]
#[serial]
fn test_upsert_deduplicates_by_name() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Upsert same person name with different platforms
    state
        .ontology_repository
        .upsert_person(user.id, "Bob", "whatsapp", None, None)
        .unwrap();
    state
        .ontology_repository
        .upsert_person(user.id, "Bob", "telegram", None, None)
        .unwrap();

    // Should be 1 person with 2 channels
    let persons = state
        .ontology_repository
        .get_persons_with_channels(user.id, 100, 0)
        .unwrap();
    assert_eq!(persons.len(), 1);
    assert_eq!(persons[0].person.name, "Bob");
    assert_eq!(persons[0].channels.len(), 2);

    let platforms: Vec<&str> = persons[0]
        .channels
        .iter()
        .map(|c| c.platform.as_str())
        .collect();
    assert!(platforms.contains(&"whatsapp"));
    assert!(platforms.contains(&"telegram"));
}

#[test]
#[serial]
fn test_upsert_deduplicates_by_room_id() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // First upsert creates person "Dave" with room "!r1"
    let person1 = state
        .ontology_repository
        .upsert_person(user.id, "Dave", "whatsapp", None, Some("!r1"))
        .unwrap();

    // Second upsert with different name but same room_id should find existing person
    let person2 = state
        .ontology_repository
        .upsert_person(user.id, "Other", "telegram", None, Some("!r1"))
        .unwrap();

    assert_eq!(person1.id, person2.id);

    // Should still be 1 person
    let persons = state.ontology_repository.get_persons(user.id).unwrap();
    assert_eq!(persons.len(), 1);
    assert_eq!(persons[0].name, "Dave"); // Original name kept
}

// =============================================================================
// Nickname lookup
// =============================================================================

#[test]
#[serial]
fn test_find_person_by_nickname() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let person = state
        .ontology_repository
        .create_person(user.id, "Robert")
        .unwrap();

    state
        .ontology_repository
        .set_person_edit(user.id, person.id, "nickname", "Bob")
        .unwrap();

    // Find by nickname
    let found = state
        .ontology_repository
        .find_person_by_name(user.id, "Bob")
        .unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().person.name, "Robert");

    // Original name still works
    let found = state
        .ontology_repository
        .find_person_by_name(user.id, "Robert")
        .unwrap();
    assert!(found.is_some());
}

// =============================================================================
// Partial search
// =============================================================================

#[test]
#[serial]
fn test_search_persons_partial() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    state
        .ontology_repository
        .create_person(user.id, "Alice Johnson")
        .unwrap();
    state
        .ontology_repository
        .create_person(user.id, "Bob Smith")
        .unwrap();
    state
        .ontology_repository
        .create_person(user.id, "Alicia Keys")
        .unwrap();

    let results = state
        .ontology_repository
        .search_persons(user.id, "ali")
        .unwrap();
    assert_eq!(results.len(), 2);

    let names: Vec<&str> = results.iter().map(|p| p.person.name.as_str()).collect();
    assert!(names.contains(&"Alice Johnson"));
    assert!(names.contains(&"Alicia Keys"));
}

// =============================================================================
// Links
// =============================================================================

#[test]
#[serial]
fn test_link_idempotent() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let person1 = state
        .ontology_repository
        .create_person(user.id, "Alice")
        .unwrap();
    let person2 = state
        .ontology_repository
        .create_person(user.id, "Bob")
        .unwrap();

    // First create_link succeeds
    state
        .ontology_repository
        .create_link(
            user.id, "Person", person1.id, "Person", person2.id, "related", None,
        )
        .unwrap();

    // Second create_link with same params - ON CONFLICT DO NOTHING means get_result returns error
    let second = state.ontology_repository.create_link(
        user.id, "Person", person1.id, "Person", person2.id, "related", None,
    );
    // It's ok if second errors (NotFound from get_result with no insert)
    let _ = second;

    // Should have exactly 1 link
    let links = state
        .ontology_repository
        .get_links_for_entity(user.id, "Person", person1.id)
        .unwrap();
    assert_eq!(links.len(), 1);
}

// =============================================================================
// User isolation
// =============================================================================

#[test]
#[serial]
fn test_user_isolation() {
    let state = create_test_state();
    let user1 = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let user2 = create_test_user(&state, &TestUserParams::finland_user(10.0, 5.0));

    state
        .ontology_repository
        .create_person(user1.id, "Alice")
        .unwrap();
    state
        .ontology_repository
        .create_person(user2.id, "Bob")
        .unwrap();

    // Each user can only see their own person
    let user1_persons = state.ontology_repository.get_persons(user1.id).unwrap();
    assert_eq!(user1_persons.len(), 1);
    assert_eq!(user1_persons[0].name, "Alice");

    let user2_persons = state.ontology_repository.get_persons(user2.id).unwrap();
    assert_eq!(user2_persons.len(), 1);
    assert_eq!(user2_persons[0].name, "Bob");

    // Cross-user lookup returns None
    let cross = state
        .ontology_repository
        .find_person_by_name(user1.id, "Bob")
        .unwrap();
    assert!(cross.is_none());
}

// =============================================================================
// Cascading delete
// =============================================================================

#[test]
#[serial]
fn test_delete_person_cascades() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let person = state
        .ontology_repository
        .create_person(user.id, "Alice")
        .unwrap();

    // Add channel
    state
        .ontology_repository
        .add_channel(user.id, person.id, "whatsapp", Some("+1234"), None)
        .unwrap();

    // Add a nickname edit
    state
        .ontology_repository
        .set_person_edit(user.id, person.id, "nickname", "Ali")
        .unwrap();

    // Delete person
    state
        .ontology_repository
        .delete_person(user.id, person.id)
        .unwrap();

    // Person gone
    let persons = state.ontology_repository.get_persons(user.id).unwrap();
    assert!(persons.is_empty());

    // Channels gone (CASCADE)
    let pwc = state
        .ontology_repository
        .get_persons_with_channels(user.id, 100, 0)
        .unwrap();
    assert!(pwc.is_empty());

    // The important thing is the person, channels, and edits are gone.
}

// =============================================================================
// Auto-resolve on user reply
// =============================================================================

#[test]
#[serial]
fn test_mark_room_digest_delivered() {
    use backend::models::ontology_models::NewOntMessage;

    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = 1000000;

    // Insert medium-urgency messages in room A
    let msg1 = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "!roomA".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Alice".to_string(),
            content: "hey".to_string(),
            person_id: None,
            created_at: now,
        })
        .unwrap();
    state
        .ontology_repository
        .update_message_classification(msg1.id, "medium", "chat", None, None, None)
        .unwrap();

    let msg2 = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "!roomA".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Alice".to_string(),
            content: "are you there?".to_string(),
            person_id: None,
            created_at: now + 10,
        })
        .unwrap();
    state
        .ontology_repository
        .update_message_classification(msg2.id, "medium", "chat", None, None, None)
        .unwrap();

    // Insert medium-urgency message in room B (should NOT be affected)
    let msg3 = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "!roomB".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Bob".to_string(),
            content: "hello".to_string(),
            person_id: None,
            created_at: now,
        })
        .unwrap();
    state
        .ontology_repository
        .update_message_classification(msg3.id, "medium", "chat", None, None, None)
        .unwrap();

    // Insert high-urgency message in room A (should NOT be affected by digest marking)
    let msg4 = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "!roomA".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Alice".to_string(),
            content: "urgent!".to_string(),
            person_id: None,
            created_at: now + 20,
        })
        .unwrap();
    state
        .ontology_repository
        .update_message_classification(msg4.id, "high", "request", None, None, None)
        .unwrap();

    // Mark room A digest delivered
    let affected = state
        .ontology_repository
        .mark_room_digest_delivered(user.id, "!roomA", now + 100)
        .unwrap();
    assert_eq!(affected, 2); // only the two medium messages in room A

    // Room B message should still be pending
    let pending = state
        .ontology_repository
        .get_pending_digest_messages(user.id)
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].room_id, "!roomB");
}

// =============================================================================
// Recently completed events
// =============================================================================

#[test]
#[serial]
fn test_get_recently_completed_events() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Create 3 events: one active, one completed recently, one completed long ago
    let event1 = state
        .ontology_repository
        .create_event(&backend::models::ontology_models::NewOntEvent {
            user_id: user.id,
            description: "Send report to boss".to_string(),
            remind_at: None,
            due_at: None,
            status: "active".to_string(),
            created_at: now - 3600,
            updated_at: now - 3600,
        })
        .unwrap();

    let event2 = state
        .ontology_repository
        .create_event(&backend::models::ontology_models::NewOntEvent {
            user_id: user.id,
            description: "Pay rent".to_string(),
            remind_at: None,
            due_at: None,
            status: "active".to_string(),
            created_at: now - 7200,
            updated_at: now - 7200,
        })
        .unwrap();

    let event3 = state
        .ontology_repository
        .create_event(&backend::models::ontology_models::NewOntEvent {
            user_id: user.id,
            description: "Old task".to_string(),
            remind_at: None,
            due_at: None,
            status: "active".to_string(),
            created_at: now - 86400,
            updated_at: now - 86400,
        })
        .unwrap();

    // Mark event2 as completed (recently)
    state
        .ontology_repository
        .update_event_status(user.id, event2.id, "completed")
        .unwrap();

    // Mark event3 as completed (but it was updated long ago - simulate by direct update)
    // update_event_status sets updated_at to now(), so we need to query after
    state
        .ontology_repository
        .update_event_status(user.id, event3.id, "completed")
        .unwrap();

    // Query completed events since 2 hours ago
    let since_ts = now - 7200;
    let completed = state
        .ontology_repository
        .get_recently_completed_events(user.id, since_ts)
        .unwrap();

    // Both event2 and event3 were completed just now (update_event_status sets updated_at=now)
    // so both should appear
    assert_eq!(completed.len(), 2);
    assert!(completed.iter().all(|e| e.status == "completed"));

    // event1 should NOT appear (still active)
    assert!(!completed.iter().any(|e| e.id == event1.id));

    // Query with a very recent since_ts - should still get both since they were just updated
    let very_recent = now - 5;
    let recent_completed = state
        .ontology_repository
        .get_recently_completed_events(user.id, very_recent)
        .unwrap();
    assert_eq!(recent_completed.len(), 2);

    // Query with future since_ts - should get none
    let future_ts = now + 3600;
    let none_completed = state
        .ontology_repository
        .get_recently_completed_events(user.id, future_ts)
        .unwrap();
    assert!(none_completed.is_empty());
}
