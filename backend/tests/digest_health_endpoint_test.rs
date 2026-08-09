use axum::http::{HeaderMap, HeaderValue};
use backend::handlers::health_handlers::{user_digest_health_requested, UserDigestHealthRow};

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
