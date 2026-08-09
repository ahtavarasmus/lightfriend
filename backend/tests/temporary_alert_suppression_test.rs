use backend::models::ontology_models::NewOntMessage;
use backend::models::user_models::TemporaryAlertSuppression;
use backend::repositories::temporary_alert_suppressions_repository::{
    decide_suppression, SuppressionDecision, KIND_QUIET, KIND_TOPIC, SCOPE_ALL, SCOPE_CRITICAL,
    SCOPE_DIGEST,
};
use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use backend::tools::alerts::persisted_suppression_confirmation;
use backend::TemporaryAlertSuppressionsRepository;
use serial_test::serial;

fn row(kind: &str, scope: &str, topic: Option<&str>) -> TemporaryAlertSuppression {
    TemporaryAlertSuppression {
        id: 1,
        user_id: 1,
        kind: kind.to_string(),
        scope: scope.to_string(),
        match_text: topic.map(str::to_string),
        timezone: "Europe/Helsinki".to_string(),
        created_at: 1_700_000_000,
        expires_at: 1_800_000_000,
        ended_at: None,
    }
}

#[test]
fn precedence_is_quiet_then_always_show_then_topic() {
    let quiet = row(KIND_QUIET, SCOPE_ALL, None);
    let topic = row(KIND_TOPIC, SCOPE_ALL, Some("Coinbase transactions"));
    assert!(matches!(
        decide_suppression(&[quiet], SCOPE_CRITICAL, "Coinbase transfer", true),
        SuppressionDecision::SuppressQuiet { .. }
    ));
    assert_eq!(
        decide_suppression(
            std::slice::from_ref(&topic),
            SCOPE_CRITICAL,
            "Coinbase transfer",
            true
        ),
        SuppressionDecision::Allow
    );
    assert!(matches!(
        decide_suppression(
            std::slice::from_ref(&topic),
            SCOPE_CRITICAL,
            "Coinbase transfer",
            false
        ),
        SuppressionDecision::SuppressTopic { .. }
    ));
    assert_eq!(
        decide_suppression(&[topic], SCOPE_DIGEST, "Coinbase transfer", false),
        SuppressionDecision::Allow
    );
}

#[test]
#[serial]
fn topic_suppression_is_cross_channel_and_preserves_ingested_messages() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let repository = TemporaryAlertSuppressionsRepository::new(state.pg_pool.clone());
    let now = chrono::Utc::now().timestamp() as i32;
    repository
        .create(
            user.id,
            KIND_TOPIC,
            SCOPE_ALL,
            Some("Coinbase transactions"),
            "Europe/Helsinki",
            now + 7200,
        )
        .unwrap();

    let email = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "email:inbox".to_string(),
            platform: "email".to_string(),
            sender_name: "Coinbase".to_string(),
            sender_key: Some("alerts@coinbase.example".to_string()),
            content: "A transaction completed".to_string(),
            person_id: None,
            created_at: now,
            matrix_event_id: None,
        })
        .unwrap()
        .0;
    let sms = state
        .ontology_repository
        .insert_message(&NewOntMessage {
            user_id: user.id,
            room_id: "sms:coinbase".to_string(),
            platform: "sms".to_string(),
            sender_name: "Coinbase".to_string(),
            sender_key: Some("+15555550100".to_string()),
            content: "Transaction confirmation".to_string(),
            person_id: None,
            created_at: now + 1,
            matrix_event_id: None,
        })
        .unwrap()
        .0;

    assert!(matches!(
        repository
            .decision(
                user.id,
                SCOPE_CRITICAL,
                "email Coinbase A transaction completed",
                false
            )
            .unwrap(),
        SuppressionDecision::SuppressTopic { .. }
    ));
    assert!(matches!(
        repository
            .decision(
                user.id,
                SCOPE_CRITICAL,
                "sms Coinbase Transaction confirmation",
                false
            )
            .unwrap(),
        SuppressionDecision::SuppressTopic { .. }
    ));
    assert_eq!(
        repository
            .decision(
                user.id,
                SCOPE_CRITICAL,
                "email GitHub deployment finished",
                false
            )
            .unwrap(),
        SuppressionDecision::Allow
    );
    let retained = state
        .ontology_repository
        .get_messages_by_ids(&[email.id, sms.id])
        .unwrap();
    assert_eq!(retained.len(), 2);
}

#[test]
#[serial]
fn quiet_mode_scopes_expire_replace_and_end_early() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let repository = TemporaryAlertSuppressionsRepository::new(state.pg_pool.clone());
    let now = chrono::Utc::now().timestamp() as i32;
    repository
        .create(
            user.id,
            KIND_QUIET,
            SCOPE_DIGEST,
            None,
            "Europe/Helsinki",
            now + 3600,
        )
        .unwrap();
    assert_eq!(
        repository
            .decision(user.id, SCOPE_CRITICAL, "urgent", false)
            .unwrap(),
        SuppressionDecision::Allow
    );
    assert!(matches!(
        repository
            .decision(user.id, SCOPE_DIGEST, "daily digest", false)
            .unwrap(),
        SuppressionDecision::SuppressQuiet { .. }
    ));

    let critical = repository
        .create(
            user.id,
            KIND_QUIET,
            SCOPE_CRITICAL,
            None,
            "Europe/Helsinki",
            now + 7200,
        )
        .unwrap();
    let active = repository.active_for_user(user.id, now).unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, critical.id);
    assert!(repository
        .active_for_user(user.id, critical.expires_at + 1)
        .unwrap()
        .is_empty());

    let ended = repository.end_quiet(user.id, Some(SCOPE_CRITICAL)).unwrap();
    assert_eq!(ended.len(), 1);
    assert!(ended[0].ended_at.is_some());
    assert!(repository.active_for_user(user.id, now).unwrap().is_empty());
}

#[test]
#[serial]
fn confirmation_is_rendered_from_the_persisted_scope_expiry_and_zone() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let repository = TemporaryAlertSuppressionsRepository::new(state.pg_pool.clone());
    let expiry = chrono::DateTime::parse_from_rfc3339("2026-12-15T09:00:00+02:00")
        .unwrap()
        .timestamp() as i32;
    let persisted = repository
        .create(
            user.id,
            KIND_TOPIC,
            SCOPE_ALL,
            Some("Coinbase transactions"),
            "Europe/Helsinki",
            expiry,
        )
        .unwrap();
    let confirmation = persisted_suppression_confirmation(&persisted);
    assert!(confirmation.contains("Coinbase transactions"));
    assert!(confirmation.contains("2026-12-15 09:00"));
    assert!(confirmation.contains("Europe/Helsinki UTC+02:00"));
}

#[test]
#[serial]
fn dashboard_cancellation_of_suppression_is_user_scoped() {
    let state = create_test_state();
    let owner = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let other = create_test_user(&state, &TestUserParams::finland_user(10.0, 5.0));
    let repository = TemporaryAlertSuppressionsRepository::new(state.pg_pool.clone());
    let now = chrono::Utc::now().timestamp() as i32;
    let suppression = repository
        .create(
            owner.id,
            KIND_TOPIC,
            SCOPE_ALL,
            Some("Coinbase transactions"),
            "Europe/Helsinki",
            now + 3600,
        )
        .unwrap();

    assert!(!repository.end_for_user(other.id, suppression.id).unwrap());
    assert!(repository.end_for_user(owner.id, suppression.id).unwrap());
    assert!(repository
        .active_for_user(owner.id, now)
        .unwrap()
        .is_empty());
}
