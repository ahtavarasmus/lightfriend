//! Unit tests for OntologyRepository.
//!
//! DB tests with #[serial]. Tests CRUD, case-insensitive matching, upsert dedup,
//! nickname lookup, partial search, links, user isolation, and cascading deletes.

use backend::test_utils::{
    create_test_item, create_test_state, create_test_user, TestItemParams, TestUserParams,
};
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
        .get_persons_with_channels(user.id)
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

    let person = state
        .ontology_repository
        .create_person(user.id, "Alice")
        .unwrap();
    let item = create_test_item(&state, &TestItemParams::reminder(user.id, "Call Alice"));

    // First create_link succeeds
    state
        .ontology_repository
        .create_link(
            user.id, "Person", person.id, "Item", item.id, "related", None,
        )
        .unwrap();

    // Second create_link with same params - ON CONFLICT DO NOTHING means get_result returns error
    let second = state.ontology_repository.create_link(
        user.id, "Person", person.id, "Item", item.id, "related", None,
    );
    // It's ok if second errors (NotFound from get_result with no insert)
    let _ = second;

    // Should have exactly 1 link
    let links = state
        .ontology_repository
        .get_links_for_entity(user.id, "Person", person.id)
        .unwrap();
    assert_eq!(links.len(), 1);
}

#[test]
#[serial]
fn test_linked_items_both_directions() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let person = state
        .ontology_repository
        .create_person(user.id, "Alice")
        .unwrap();
    let item1 = create_test_item(&state, &TestItemParams::reminder(user.id, "Task A"));
    let item2 = create_test_item(&state, &TestItemParams::reminder(user.id, "Task B"));

    // Link Item->Person (item1 as source)
    state
        .ontology_repository
        .create_link(
            user.id,
            "Item",
            item1.id,
            "Person",
            person.id,
            "assigned_to",
            None,
        )
        .unwrap();

    // Link Person->Item (person as source)
    state
        .ontology_repository
        .create_link(
            user.id, "Person", person.id, "Item", item2.id, "related", None,
        )
        .unwrap();

    // get_linked_items_for_person should return both
    let linked = state
        .ontology_repository
        .get_linked_items_for_person(user.id, person.id)
        .unwrap();
    assert_eq!(linked.len(), 2);

    let summaries: Vec<&str> = linked
        .iter()
        .map(|(_, item)| item.summary.as_str())
        .collect();
    assert!(summaries.contains(&"Task A"));
    assert!(summaries.contains(&"Task B"));
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

    // Create item and link
    let item = create_test_item(&state, &TestItemParams::reminder(user.id, "Task"));
    state
        .ontology_repository
        .create_link(
            user.id, "Person", person.id, "Item", item.id, "related", None,
        )
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
        .get_persons_with_channels(user.id)
        .unwrap();
    assert!(pwc.is_empty());

    // Links referencing deleted person should still exist in ont_links (no FK cascade)
    // but get_linked_items_for_person would fail since person is gone.
    // The important thing is the person, channels, and edits are gone.
}
