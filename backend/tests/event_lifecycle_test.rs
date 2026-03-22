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
            notify_at: Some(now - 120),
            expires_at: Some(now - 60),
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
            notify_at: Some(now - 600),
            expires_at: Some(now - 300),
            status: "completed".to_string(),
            created_at: now - 600,
            updated_at: now - 600,
        })
        .unwrap();

    let purged = state.ontology_repository.purge_old_events(60).unwrap();

    assert_eq!(purged, 1);
    assert!(state.ontology_repository.get_event(user.id, event.id).is_err());
}
