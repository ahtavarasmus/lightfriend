use backend::models::ontology_models::NewOntEvent;
use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn notified_events_are_still_expirable() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    let event = state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "One-shot reminder".to_string(),
            remind_at: Some(now - 120),
            due_at: Some(now - 60),
            status: "notified".to_string(),
            created_at: now - 300,
            updated_at: now - 300,
        })
        .unwrap();

    let expired = state.ontology_repository.get_expired_events(now).unwrap();

    assert!(expired.iter().any(|e| e.id == event.id));
}

#[tokio::test]
#[serial]
async fn purge_old_events_removes_completed_rows() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    let event = state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Done task".to_string(),
            remind_at: Some(now - 600),
            due_at: Some(now - 300),
            status: "completed".to_string(),
            created_at: now - 600,
            updated_at: now - 600,
        })
        .unwrap();

    let purged = state.ontology_repository.purge_old_events(60).unwrap();

    assert_eq!(purged, 1);
    assert!(state
        .ontology_repository
        .get_event(user.id, event.id)
        .is_err());
}

#[tokio::test]
#[serial]
async fn get_messages_for_event_returns_oldest_to_newest_linked_messages() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    let first_message = state
        .ontology_repository
        .insert_message(&backend::models::ontology_models::NewOntMessage {
            user_id: user.id,
            room_id: "room-1".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Alice".to_string(),
            content: "Original package confirmation".to_string(),
            person_id: None,
            created_at: now - 200,
        })
        .unwrap();
    let second_message = state
        .ontology_repository
        .insert_message(&backend::models::ontology_models::NewOntMessage {
            user_id: user.id,
            room_id: "room-1".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Alice".to_string(),
            content: "Latest delivery update".to_string(),
            person_id: None,
            created_at: now - 50,
        })
        .unwrap();

    let event = state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Track package".to_string(),
            remind_at: Some(now + 60),
            due_at: Some(now + 3600),
            status: "active".to_string(),
            created_at: now - 250,
            updated_at: now - 250,
        })
        .unwrap();

    state
        .ontology_repository
        .create_link(
            user.id,
            "Message",
            second_message.id as i32,
            "Event",
            event.id,
            "updates",
            None,
        )
        .unwrap();
    state
        .ontology_repository
        .create_link(
            user.id,
            "Message",
            first_message.id as i32,
            "Event",
            event.id,
            "triggers",
            None,
        )
        .unwrap();

    let linked = state
        .ontology_repository
        .get_messages_for_event(user.id, event.id)
        .unwrap();

    assert_eq!(linked.len(), 2);
    assert_eq!(linked[0].id, first_message.id);
    assert_eq!(linked[1].id, second_message.id);
}

#[tokio::test]
#[serial]
async fn update_event_appends_description_and_replaces_timestamps() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    let event = state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Package delivery from shop".to_string(),
            remind_at: Some(now + 60),
            due_at: Some(now + 3600),
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        })
        .unwrap();

    let updated = state
        .ontology_repository
        .update_event(
            user.id,
            event.id,
            Some("Carrier says it will arrive Friday"),
            Some("active"),
            Some(now + 7200),
            Some(now + 14400),
        )
        .unwrap();

    assert!(updated.description.contains("Package delivery from shop"));
    assert!(updated
        .description
        .contains("Update: Carrier says it will arrive Friday"));
    assert_eq!(updated.remind_at, Some(now + 7200));
    assert_eq!(updated.due_at, Some(now + 14400));
}

#[tokio::test]
#[serial]
async fn update_event_preserves_untouched_timestamp_fields() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    let event = state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Renew passport".to_string(),
            remind_at: Some(now + 600),
            due_at: Some(now + 3600),
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        })
        .unwrap();

    let updated = state
        .ontology_repository
        .update_event(
            user.id,
            event.id,
            None,
            Some("active"),
            Some(now + 7200),
            None,
        )
        .unwrap();

    assert_eq!(updated.remind_at, Some(now + 7200));
    assert_eq!(updated.due_at, Some(now + 3600));
}
