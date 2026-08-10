use axum::http::{HeaderMap, HeaderValue};
use backend::handlers::health_handlers::{
    user_digest_health_requested, UserDigestHealthRow, USER_DIGEST_HEALTH_QUERY,
};
use backend::repositories::user_repository::LogUsageParams;
use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use backend::UserCoreOps;
use serial_test::serial;

#[test]
fn personal_digest_health_requires_exact_explicit_opt_in() {
    let mut headers = HeaderMap::new();
    assert!(!user_digest_health_requested(&headers));

    headers.insert(
        "X-Include-User-Digest-Health",
        HeaderValue::from_static("false"),
    );
    assert!(!user_digest_health_requested(&headers));

    headers.insert(
        "X-Include-User-Digest-Health",
        HeaderValue::from_static("true"),
    );
    assert!(user_digest_health_requested(&headers));
}

#[test]
fn personal_digest_health_row_serializes_aggregate_fields_only() {
    let row = UserDigestHealthRow {
        user_id: 7,
        digest_enabled: true,
        digest_time: Some("13:00".to_string()),
        timezone: Some("Europe/Helsinki".to_string()),
        active_imap_connections: 1,
        processed_emails_24h: 12,
        ingested_emails_24h: 10,
        latest_email_ingested_at: Some(1_786_265_000),
        pending_later_emails_24h: 4,
        pending_now_emails_24h: 1,
        pending_unclassified_emails_24h: 0,
        seen_emails_24h: 3,
        digest_delivered_emails_24h: 2,
        last_digest_attempt_at: Some(1_786_260_000),
        last_digest_attempt_success: Some(true),
    };

    let value = serde_json::to_value(row).expect("serialize aggregate digest health");
    let object = value.as_object().expect("aggregate object");
    assert_eq!(object.len(), 15);
    assert!(object.contains_key("user_id"));
    assert!(object.contains_key("pending_later_emails_24h"));

    let serialized = value.to_string();
    for forbidden in [
        "sender_name",
        "sender_key",
        "content",
        "summary",
        "phone_number",
        "encrypted_password",
    ] {
        assert!(!serialized.contains(forbidden));
    }
}

#[test]
fn personal_digest_health_query_uses_the_production_processed_email_timestamp() {
    assert!(USER_DIGEST_HEALTH_QUERY.contains("pe.processed_at >= $1"));
    assert!(!USER_DIGEST_HEALTH_QUERY.contains("pe.created_at"));
}

#[test]
#[serial]
fn durable_digest_checkpoint_counts_only_successful_deliveries() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    state
        .user_core
        .update_digest_enabled(user.id, true)
        .unwrap();
    assert!(state.user_repository.digest_enabled(user.id).unwrap());

    let log = |activity_type: &str, success: bool| {
        state
            .user_repository
            .log_usage(LogUsageParams {
                user_id: user.id,
                sid: None,
                activity_type: activity_type.to_string(),
                credits: None,
                time_consumed: None,
                success: Some(success),
                reason: None,
                status: None,
                recharge_threshold_timestamp: None,
                zero_credits_timestamp: None,
            })
            .unwrap();
    };

    log("digest", false);
    log("noti_msg", true);
    assert_eq!(
        state
            .user_repository
            .latest_successful_digest_checkpoint(user.id)
            .unwrap(),
        None
    );

    log("digest_empty", true);
    assert_eq!(
        state
            .user_repository
            .latest_successful_digest_checkpoint(user.id)
            .unwrap(),
        None
    );

    log("digest", true);
    assert!(state
        .user_repository
        .latest_successful_digest_checkpoint(user.id)
        .unwrap()
        .is_some());
}
