use backend::models::ontology_models::NewOntEvent;
use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use backend::UserCoreOps;
use serial_test::serial;

const GRACE: i32 = 2 * 3600;

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

    let (first_message, _is_new) = state
        .ontology_repository
        .insert_message(&backend::models::ontology_models::NewOntMessage {
            user_id: user.id,
            room_id: "room-1".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Alice".to_string(),
            sender_key: None,
            content: "Original package confirmation".to_string(),
            person_id: None,
            created_at: now - 200,
            matrix_event_id: None,
        })
        .unwrap();
    let (second_message, _is_new) = state
        .ontology_repository
        .insert_message(&backend::models::ontology_models::NewOntMessage {
            user_id: user.id,
            room_id: "room-1".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Alice".to_string(),
            sender_key: None,
            content: "Latest delivery update".to_string(),
            person_id: None,
            created_at: now - 50,
            matrix_event_id: None,
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

#[tokio::test]
#[serial]
async fn friend_notification_returns_events_inside_grace_window() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    let event = state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Call mom".to_string(),
            remind_at: Some(now - 1800),
            due_at: Some(now + 1800), // 30 min away, inside 2h grace
            status: "active".to_string(),
            created_at: now - 3600,
            updated_at: now - 3600,
        })
        .unwrap();

    let due = state
        .ontology_repository
        .get_events_due_for_friend_notification(now, GRACE)
        .unwrap();

    assert!(
        due.iter().any(|e| e.id == event.id),
        "event 30min from deadline should appear in friend-notify window"
    );
}

#[tokio::test]
#[serial]
async fn friend_notification_skips_events_outside_grace_window() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    let event = state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Far-future task".to_string(),
            remind_at: Some(now + 3600),
            due_at: Some(now + GRACE + 600), // just past grace edge
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        })
        .unwrap();

    let due = state
        .ontology_repository
        .get_events_due_for_friend_notification(now, GRACE)
        .unwrap();

    assert!(
        due.iter().all(|e| e.id != event.id),
        "event outside grace window should not appear"
    );
}

#[tokio::test]
#[serial]
async fn friend_notification_skips_events_past_due_at() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    let event = state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Already overdue".to_string(),
            remind_at: Some(now - 3600),
            due_at: Some(now - 60), // already past deadline
            status: "active".to_string(),
            created_at: now - 7200,
            updated_at: now - 7200,
        })
        .unwrap();

    let due = state
        .ontology_repository
        .get_events_due_for_friend_notification(now, GRACE)
        .unwrap();

    assert!(
        due.iter().all(|e| e.id != event.id),
        "friend nudge is collaborative, not punitive: don't fire after deadline"
    );
}

#[tokio::test]
#[serial]
async fn friend_notification_skips_events_already_notified() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    let event = state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Already pinged friend".to_string(),
            remind_at: Some(now - 1800),
            due_at: Some(now + 1800),
            status: "active".to_string(),
            created_at: now - 3600,
            updated_at: now - 3600,
        })
        .unwrap();

    state
        .ontology_repository
        .mark_friend_notified(user.id, event.id, now - 300)
        .unwrap();

    let due = state
        .ontology_repository
        .get_events_due_for_friend_notification(now, GRACE)
        .unwrap();

    assert!(
        due.iter().all(|e| e.id != event.id),
        "event with friend_notified_at set should not re-trigger"
    );
}

#[tokio::test]
#[serial]
async fn friend_notification_skips_events_without_due_at() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    let event = state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Open-ended task".to_string(),
            remind_at: Some(now - 60),
            due_at: None,
            status: "active".to_string(),
            created_at: now - 3600,
            updated_at: now - 3600,
        })
        .unwrap();

    let due = state
        .ontology_repository
        .get_events_due_for_friend_notification(now, GRACE)
        .unwrap();

    assert!(
        due.iter().all(|e| e.id != event.id),
        "no deadline = no friend nudge"
    );
}

#[tokio::test]
#[serial]
async fn friend_notification_includes_status_notified() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    let event = state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "User got reminded but hasn't done it".to_string(),
            remind_at: Some(now - 3600),
            due_at: Some(now + 1800),
            status: "notified".to_string(),
            created_at: now - 7200,
            updated_at: now - 7200,
        })
        .unwrap();

    let due = state
        .ontology_repository
        .get_events_due_for_friend_notification(now, GRACE)
        .unwrap();

    assert!(
        due.iter().any(|e| e.id == event.id),
        "'notified' events still in grace window should trigger friend nudge"
    );
}

#[tokio::test]
#[serial]
async fn mark_friend_notified_sets_timestamp() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    let event = state
        .ontology_repository
        .create_event(&NewOntEvent {
            user_id: user.id,
            description: "Test".to_string(),
            remind_at: Some(now - 1800),
            due_at: Some(now + 1800),
            status: "active".to_string(),
            created_at: now - 3600,
            updated_at: now - 3600,
        })
        .unwrap();

    assert!(
        state
            .ontology_repository
            .get_event(user.id, event.id)
            .unwrap()
            .friend_notified_at
            .is_none(),
        "should start unset"
    );

    state
        .ontology_repository
        .mark_friend_notified(user.id, event.id, now)
        .unwrap();

    let reloaded = state
        .ontology_repository
        .get_event(user.id, event.id)
        .unwrap();
    assert_eq!(reloaded.friend_notified_at, Some(now));
}

#[tokio::test]
#[serial]
async fn accountability_defaults_to_off_for_new_users() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    assert!(!user.accountability_enabled);
    assert!(user.accountability_friend_phone.is_none());
    assert!(user.accountability_friend_name.is_none());
}

#[tokio::test]
#[serial]
async fn accountability_settings_round_trip() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    state
        .user_core
        .update_accountability_friend_phone(user.id, Some("+358401234567"))
        .unwrap();
    state
        .user_core
        .update_accountability_friend_name(user.id, Some("Alex"))
        .unwrap();
    state
        .user_core
        .update_accountability_enabled(user.id, true)
        .unwrap();

    let reloaded = state
        .user_core
        .find_by_id(user.id)
        .unwrap()
        .expect("user disappeared");
    assert_eq!(
        reloaded.accountability_friend_phone.as_deref(),
        Some("+358401234567")
    );
    assert_eq!(reloaded.accountability_friend_name.as_deref(), Some("Alex"));
    assert!(reloaded.accountability_enabled);

    // Clearing back to None works
    state
        .user_core
        .update_accountability_friend_phone(user.id, None)
        .unwrap();
    state
        .user_core
        .update_accountability_enabled(user.id, false)
        .unwrap();

    let cleared = state
        .user_core
        .find_by_id(user.id)
        .unwrap()
        .expect("user disappeared");
    assert!(cleared.accountability_friend_phone.is_none());
    assert!(!cleared.accountability_enabled);
}
