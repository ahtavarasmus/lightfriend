use std::sync::{Arc, Barrier};

use backend::models::ontology_models::NewOntEvent;
use backend::proactive::utils::{
    format_persisted_local_time, parse_reminder_time_in_zone, reminder_timezone,
};
use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use backend::UserCoreOps;
use serial_test::serial;

fn due_reminder(user_id: i32, now: i32) -> NewOntEvent {
    NewOntEvent {
        user_id,
        description: "Take medication".to_string(),
        remind_at: Some(now - 1),
        due_at: Some(now - 1),
        status: "active".to_string(),
        created_at: now - 60,
        updated_at: now - 60,
    }
}

#[test]
fn target_date_iana_offset_and_dst_edges_are_deterministic() {
    let winter_now = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .unwrap()
        .timestamp() as i32;
    let summer =
        parse_reminder_time_in_zone("2026-07-01T09:00", "Europe/Helsinki", winter_now).unwrap();
    assert_eq!(
        summer as i64,
        chrono::DateTime::parse_from_rfc3339("2026-07-01T09:00:00+03:00")
            .unwrap()
            .timestamp()
    );

    let gap = parse_reminder_time_in_zone("2026-03-08T02:30", "America/New_York", winter_now)
        .unwrap_err();
    assert!(gap.contains("does not exist"));

    let fold = parse_reminder_time_in_zone("2026-11-01T01:30", "America/New_York", winter_now)
        .unwrap_err();
    assert!(fold.contains("occurs twice"));
}

#[test]
#[serial]
fn automatic_timezone_updates_persist_and_confirmation_uses_stored_values() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    state.user_core.update_timezone_auto(user.id, true).unwrap();
    state
        .user_core
        .update_timezone(user.id, "Europe/Helsinki")
        .unwrap();
    let now = chrono::Utc::now().timestamp() as i32;
    assert_eq!(
        reminder_timezone(&state, user.id, now).unwrap(),
        "Europe/Helsinki"
    );

    let at = parse_reminder_time_in_zone(
        "2026-12-15T09:00",
        "Europe/Helsinki",
        chrono::DateTime::parse_from_rfc3339("2026-08-01T00:00:00Z")
            .unwrap()
            .timestamp() as i32,
    )
    .unwrap();
    let event = state
        .ontology_repository
        .create_reminder(&due_reminder(user.id, at + 1), "Europe/Helsinki")
        .unwrap();
    let persisted = state
        .ontology_repository
        .get_event(user.id, event.id)
        .unwrap();
    let rendered = format_persisted_local_time(
        persisted.remind_at.unwrap(),
        persisted.reminder_timezone.as_deref().unwrap(),
    )
    .unwrap();
    assert!(rendered.contains("2026-12-15 09:00"));
    assert!(rendered.contains("Europe/Helsinki UTC+02:00"));
}

#[test]
#[serial]
fn failed_delivery_retries_and_is_not_expired_before_success() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;
    let event = state
        .ontology_repository
        .create_reminder(&due_reminder(user.id, now), "UTC")
        .unwrap();

    let claimed = state
        .ontology_repository
        .claim_due_reminders(now, 120, 10)
        .unwrap();
    assert_eq!(claimed.len(), 1);
    state
        .ontology_repository
        .release_reminder_for_retry(
            user.id,
            event.id,
            claimed[0].reminder_attempts,
            "delivery_failed",
            now,
        )
        .unwrap();
    assert!(state
        .ontology_repository
        .get_expired_events(now + 60)
        .unwrap()
        .iter()
        .all(|candidate| candidate.id != event.id));

    let retry_at = state
        .ontology_repository
        .get_event(user.id, event.id)
        .unwrap()
        .reminder_next_attempt_at
        .unwrap();
    let retried = state
        .ontology_repository
        .claim_due_reminders(retry_at, 120, 10)
        .unwrap();
    assert_eq!(retried.len(), 1);
    state
        .ontology_repository
        .mark_reminder_delivered(user.id, event.id, retry_at)
        .unwrap();
    let delivered = state
        .ontology_repository
        .get_event(user.id, event.id)
        .unwrap();
    assert_eq!(delivered.status, "notified");
    assert_eq!(delivered.reminder_delivered_at, Some(retry_at));
}

#[test]
#[serial]
fn expired_lease_is_recovered_after_restart() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;
    let event = state
        .ontology_repository
        .create_reminder(&due_reminder(user.id, now), "UTC")
        .unwrap();
    assert_eq!(
        state
            .ontology_repository
            .claim_due_reminders(now, 30, 10)
            .unwrap()
            .len(),
        1
    );
    assert!(state
        .ontology_repository
        .claim_due_reminders(now + 29, 30, 10)
        .unwrap()
        .is_empty());
    let recovered = state
        .ontology_repository
        .claim_due_reminders(now + 31, 30, 10)
        .unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].id, event.id);
}

#[test]
#[serial]
fn concurrent_scheduler_instances_cannot_claim_the_same_reminder() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;
    state
        .ontology_repository
        .create_reminder(&due_reminder(user.id, now), "UTC")
        .unwrap();

    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let repository = state.ontology_repository.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            repository.claim_due_reminders(now, 120, 10).unwrap().len()
        }));
    }
    barrier.wait();
    let total: usize = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .sum();
    assert_eq!(total, 1);
}
