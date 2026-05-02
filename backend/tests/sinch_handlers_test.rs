//! Unit tests for Sinch webhook handler internals.
//!
//! Auth middleware integration is exercised through a small Axum Router
//! built per-test (no AppState needed — middleware reads env directly).
//! Status mapping and phone normalization are pure functions, tested
//! directly. End-to-end inbound flow requires AppState + DB and lives
//! in integration suites once Sinch goes live.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::post,
    Router,
};
use backend::api::sinch_utils::validate_sinch_auth;
use backend::handlers::sinch_handlers::{
    is_final_status, map_sinch_status, normalize_phone, SinchDeliveryReport, SinchInboundPayload,
};
use serial_test::serial;
use tower::ServiceExt;

// =============================================================================
// Pure helpers
// =============================================================================

#[test]
fn maps_sinch_status_to_twilio_taxonomy() {
    assert_eq!(map_sinch_status("Queued"), "queued");
    assert_eq!(map_sinch_status("Dispatched"), "sent");
    assert_eq!(map_sinch_status("Delivered"), "delivered");
    assert_eq!(map_sinch_status("Failed"), "failed");
    assert_eq!(map_sinch_status("Rejected"), "failed");
    assert_eq!(map_sinch_status("Aborted"), "undelivered");
    assert_eq!(map_sinch_status("Expired"), "undelivered");
    // Unknown / fallback
    assert_eq!(map_sinch_status("SomethingNew"), "queued");
    assert_eq!(map_sinch_status(""), "queued");
}

#[test]
fn final_status_predicate() {
    assert!(is_final_status("delivered"));
    assert!(is_final_status("failed"));
    assert!(is_final_status("undelivered"));
    assert!(!is_final_status("queued"));
    assert!(!is_final_status("sent"));
    assert!(!is_final_status("sending"));
}

#[test]
fn normalize_phone_adds_plus_prefix() {
    assert_eq!(normalize_phone("15551234567"), "+15551234567");
    assert_eq!(normalize_phone("+15551234567"), "+15551234567");
    assert_eq!(normalize_phone("  15551234567  "), "+15551234567");
    assert_eq!(normalize_phone(" +15551234567 "), "+15551234567");
}

// =============================================================================
// Payload parsing
// =============================================================================

#[test]
fn parses_minimal_inbound_payload() {
    let json = r#"{
        "type": "mo_text",
        "id": "01F2E6WTRTBV3W8B9NPY1H4MY",
        "from": "15551234567",
        "to": "18005551234",
        "body": "Hello"
    }"#;
    let p: SinchInboundPayload = serde_json::from_str(json).unwrap();
    assert_eq!(p.message_type, "mo_text");
    assert_eq!(p.from, "15551234567");
    assert_eq!(p.to, "18005551234");
    assert_eq!(p.body.as_deref(), Some("Hello"));
}

#[test]
fn parses_inbound_payload_with_missing_body() {
    let json = r#"{
        "type": "mo_text",
        "id": "abc",
        "from": "15551234567",
        "to": "18005551234"
    }"#;
    let p: SinchInboundPayload = serde_json::from_str(json).unwrap();
    assert!(p.body.is_none());
}

#[test]
fn parses_per_recipient_dlr_payload() {
    let json = r#"{
        "type": "delivery_report_sms",
        "batch_id": "01F2E6WTRTBV3W8B9NPY1H4MY",
        "client_reference": "ignored",
        "code": 0,
        "status": "Delivered",
        "to": "15551234567",
        "at": "2024-01-01T00:00:00Z"
    }"#;
    let p: SinchDeliveryReport = serde_json::from_str(json).unwrap();
    assert_eq!(p.report_type, "delivery_report_sms");
    assert_eq!(p.batch_id, "01F2E6WTRTBV3W8B9NPY1H4MY");
    assert_eq!(p.status, "Delivered");
    assert_eq!(p.code, Some(0));
    assert_eq!(p.to.as_deref(), Some("15551234567"));
}

#[test]
fn parses_dlr_payload_missing_optional_fields() {
    let json = r#"{
        "type": "delivery_report_sms",
        "batch_id": "abc",
        "status": "Failed"
    }"#;
    let p: SinchDeliveryReport = serde_json::from_str(json).unwrap();
    assert!(p.code.is_none());
    assert!(p.to.is_none());
}

// =============================================================================
// Auth middleware
// =============================================================================

async fn dummy_handler() -> StatusCode {
    StatusCode::OK
}

fn auth_app() -> Router {
    Router::new()
        .route("/test", post(dummy_handler))
        .layer(middleware::from_fn(validate_sinch_auth))
}

#[tokio::test]
#[serial]
async fn auth_rejects_when_secret_unset() {
    std::env::remove_var("SINCH_CALLBACK_SECRET");

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("authorization", "Bearer anything")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn auth_rejects_when_secret_empty() {
    std::env::set_var("SINCH_CALLBACK_SECRET", "");

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("authorization", "Bearer anything")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    std::env::remove_var("SINCH_CALLBACK_SECRET");
}

#[tokio::test]
#[serial]
async fn auth_rejects_missing_header() {
    std::env::set_var("SINCH_CALLBACK_SECRET", "topsecret");

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    std::env::remove_var("SINCH_CALLBACK_SECRET");
}

#[tokio::test]
#[serial]
async fn auth_rejects_wrong_bearer() {
    std::env::set_var("SINCH_CALLBACK_SECRET", "topsecret");

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("authorization", "Bearer wrong")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    std::env::remove_var("SINCH_CALLBACK_SECRET");
}

#[tokio::test]
#[serial]
async fn auth_rejects_non_bearer_scheme() {
    std::env::set_var("SINCH_CALLBACK_SECRET", "topsecret");

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("authorization", "Basic dXNlcjpwYXNz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    std::env::remove_var("SINCH_CALLBACK_SECRET");
}

#[tokio::test]
#[serial]
async fn auth_accepts_correct_bearer() {
    std::env::set_var("SINCH_CALLBACK_SECRET", "topsecret");

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("authorization", "Bearer topsecret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    std::env::remove_var("SINCH_CALLBACK_SECRET");
}

#[tokio::test]
#[serial]
async fn auth_accepts_lowercase_bearer_prefix() {
    std::env::set_var("SINCH_CALLBACK_SECRET", "topsecret");

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("authorization", "bearer topsecret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    std::env::remove_var("SINCH_CALLBACK_SECRET");
}
