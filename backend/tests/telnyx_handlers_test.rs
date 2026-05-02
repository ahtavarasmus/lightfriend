//! Unit tests for Telnyx webhook handler internals and signature
//! middleware.
//!
//! End-to-end inbound flow requires AppState + DB and lives in integration
//! suites once Telnyx goes live. These tests cover:
//!   - Pure helpers (status mapping, phone normalization)
//!   - Payload parsing (nested data.payload shape Telnyx uses)
//!   - Ed25519 signature middleware: env var gate + positive + negative

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::post,
    Router,
};
use backend::api::telnyx_utils::validate_telnyx_signature;
use backend::handlers::telnyx_handlers::{
    map_telnyx_status, normalize_phone, TelnyxInboundPayload, TelnyxStatusPayload, TelnyxWebhook,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use serial_test::serial;
use tower::ServiceExt;

// =============================================================================
// Pure helpers
// =============================================================================

#[test]
fn maps_telnyx_status_to_twilio_taxonomy() {
    assert_eq!(map_telnyx_status("queued"), "queued");
    assert_eq!(map_telnyx_status("sending"), "sending");
    assert_eq!(map_telnyx_status("delivering"), "sending");
    assert_eq!(map_telnyx_status("sent"), "sent");
    assert_eq!(map_telnyx_status("delivery_unconfirmed"), "sent");
    assert_eq!(map_telnyx_status("delivered"), "delivered");
    assert_eq!(map_telnyx_status("sending_failed"), "failed");
    assert_eq!(map_telnyx_status("delivery_failed"), "failed");
    // Unknown / fallback
    assert_eq!(map_telnyx_status("something_new"), "queued");
    assert_eq!(map_telnyx_status(""), "queued");
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
fn parses_inbound_message_received_event() {
    let json = r#"{
        "data": {
            "event_type": "message.received",
            "id": "abc-event",
            "occurred_at": "2024-01-01T00:00:00Z",
            "payload": {
                "id": "msg-123",
                "type": "SMS",
                "from": {
                    "phone_number": "+15551234567",
                    "carrier": "T-MOBILE"
                },
                "to": [
                    {
                        "phone_number": "+18005551234",
                        "status": "webhook_delivered"
                    }
                ],
                "text": "Hello from US",
                "media": []
            }
        }
    }"#;
    let w: TelnyxWebhook<TelnyxInboundPayload> = serde_json::from_str(json).unwrap();
    assert_eq!(w.data.event_type, "message.received");
    assert_eq!(w.data.payload.id, "msg-123");
    assert_eq!(w.data.payload.from.phone_number, "+15551234567");
    assert_eq!(w.data.payload.to.len(), 1);
    assert_eq!(w.data.payload.to[0].phone_number, "+18005551234");
    assert_eq!(w.data.payload.text.as_deref(), Some("Hello from US"));
    assert_eq!(w.data.payload.media.len(), 0);
}

#[test]
fn parses_inbound_with_mms_media() {
    let json = r#"{
        "data": {
            "event_type": "message.received",
            "payload": {
                "id": "msg-mms",
                "from": {"phone_number": "+15551234567"},
                "to": [{"phone_number": "+18005551234"}],
                "text": "see pic",
                "media": [
                    {"url": "https://example.com/img.jpg", "content_type": "image/jpeg"}
                ]
            }
        }
    }"#;
    let w: TelnyxWebhook<TelnyxInboundPayload> = serde_json::from_str(json).unwrap();
    assert_eq!(w.data.payload.media.len(), 1);
    assert_eq!(w.data.payload.media[0].url, "https://example.com/img.jpg");
    assert_eq!(
        w.data.payload.media[0].content_type.as_deref(),
        Some("image/jpeg")
    );
}

#[test]
fn parses_inbound_without_text_field() {
    let json = r#"{
        "data": {
            "event_type": "message.received",
            "payload": {
                "id": "abc",
                "from": {"phone_number": "+15551234567"},
                "to": [{"phone_number": "+18005551234"}]
            }
        }
    }"#;
    let w: TelnyxWebhook<TelnyxInboundPayload> = serde_json::from_str(json).unwrap();
    assert!(w.data.payload.text.is_none());
    assert_eq!(w.data.payload.media.len(), 0);
}

#[test]
fn parses_status_message_sent_event() {
    let json = r#"{
        "data": {
            "event_type": "message.sent",
            "id": "evt-1",
            "payload": {
                "id": "msg-456",
                "from": {"phone_number": "+18005551234"},
                "to": [
                    {
                        "phone_number": "+15551234567",
                        "status": "sent"
                    }
                ]
            }
        }
    }"#;
    let w: TelnyxWebhook<TelnyxStatusPayload> = serde_json::from_str(json).unwrap();
    assert_eq!(w.data.event_type, "message.sent");
    assert_eq!(w.data.payload.id, "msg-456");
    assert_eq!(w.data.payload.to[0].phone_number, "+15551234567");
    assert_eq!(w.data.payload.to[0].status.as_deref(), Some("sent"));
}

#[test]
fn parses_status_message_finalized_delivered() {
    let json = r#"{
        "data": {
            "event_type": "message.finalized",
            "payload": {
                "id": "msg-789",
                "from": {"phone_number": "+18005551234"},
                "to": [
                    {"phone_number": "+15551234567", "status": "delivered"}
                ]
            }
        }
    }"#;
    let w: TelnyxWebhook<TelnyxStatusPayload> = serde_json::from_str(json).unwrap();
    assert_eq!(w.data.event_type, "message.finalized");
    assert_eq!(w.data.payload.to[0].status.as_deref(), Some("delivered"));
}

// =============================================================================
// Ed25519 signature middleware
// =============================================================================

async fn dummy_handler() -> StatusCode {
    StatusCode::OK
}

fn auth_app() -> Router {
    Router::new()
        .route("/test", post(dummy_handler))
        .layer(middleware::from_fn(validate_telnyx_signature))
}

/// Build (signing_key, base64_public_key) for a fresh test keypair.
fn fresh_keypair() -> (SigningKey, String) {
    let mut rng = OsRng;
    let signing_key = SigningKey::generate(&mut rng);
    let pub_b64 = BASE64.encode(signing_key.verifying_key().to_bytes());
    (signing_key, pub_b64)
}

/// Sign `<timestamp>|<body>` with the given key, return base64-encoded sig.
fn sign(signing_key: &SigningKey, timestamp: &str, body: &[u8]) -> String {
    let mut message = Vec::with_capacity(timestamp.len() + 1 + body.len());
    message.extend_from_slice(timestamp.as_bytes());
    message.push(b'|');
    message.extend_from_slice(body);
    let sig = signing_key.sign(&message);
    BASE64.encode(sig.to_bytes())
}

#[tokio::test]
#[serial]
async fn auth_rejects_when_public_key_unset() {
    std::env::remove_var("TELNYX_PUBLIC_KEY");

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("telnyx-signature-ed25519", "anything")
                .header("telnyx-timestamp", "1700000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn auth_rejects_when_public_key_empty() {
    std::env::set_var("TELNYX_PUBLIC_KEY", "");

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("telnyx-signature-ed25519", "anything")
                .header("telnyx-timestamp", "1700000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    std::env::remove_var("TELNYX_PUBLIC_KEY");
}

#[tokio::test]
#[serial]
async fn auth_rejects_missing_signature_header() {
    let (_sk, pub_b64) = fresh_keypair();
    std::env::set_var("TELNYX_PUBLIC_KEY", &pub_b64);

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("telnyx-timestamp", "1700000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    std::env::remove_var("TELNYX_PUBLIC_KEY");
}

#[tokio::test]
#[serial]
async fn auth_rejects_missing_timestamp_header() {
    let (_sk, pub_b64) = fresh_keypair();
    std::env::set_var("TELNYX_PUBLIC_KEY", &pub_b64);

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("telnyx-signature-ed25519", "anything")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    std::env::remove_var("TELNYX_PUBLIC_KEY");
}

#[tokio::test]
#[serial]
async fn auth_rejects_invalid_base64_public_key() {
    std::env::set_var("TELNYX_PUBLIC_KEY", "this-is-not-base64-!!!");

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("telnyx-signature-ed25519", "AAAA")
                .header("telnyx-timestamp", "1700000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);

    std::env::remove_var("TELNYX_PUBLIC_KEY");
}

#[tokio::test]
#[serial]
async fn auth_rejects_signature_from_wrong_key() {
    let (_real_sk, real_pub_b64) = fresh_keypair();
    let (impostor_sk, _) = fresh_keypair();
    std::env::set_var("TELNYX_PUBLIC_KEY", &real_pub_b64);

    let timestamp = "1700000000";
    let body = b"{\"data\":{\"event_type\":\"message.received\"}}";
    let bad_sig = sign(&impostor_sk, timestamp, body);

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("telnyx-signature-ed25519", bad_sig)
                .header("telnyx-timestamp", timestamp)
                .body(Body::from(body.to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    std::env::remove_var("TELNYX_PUBLIC_KEY");
}

#[tokio::test]
#[serial]
async fn auth_rejects_tampered_body() {
    let (sk, pub_b64) = fresh_keypair();
    std::env::set_var("TELNYX_PUBLIC_KEY", &pub_b64);

    let timestamp = "1700000000";
    let original_body = b"{\"original\":true}";
    let sig_for_original = sign(&sk, timestamp, original_body);

    // Send a different body with the original's signature
    let tampered_body = b"{\"tampered\":true}";

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("telnyx-signature-ed25519", sig_for_original)
                .header("telnyx-timestamp", timestamp)
                .body(Body::from(tampered_body.to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    std::env::remove_var("TELNYX_PUBLIC_KEY");
}

#[tokio::test]
#[serial]
async fn auth_accepts_valid_signature() {
    let (sk, pub_b64) = fresh_keypair();
    std::env::set_var("TELNYX_PUBLIC_KEY", &pub_b64);

    let timestamp = "1700000000";
    let body = b"{\"data\":{\"event_type\":\"message.received\"}}";
    let sig = sign(&sk, timestamp, body);

    let res = auth_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/test")
                .header("telnyx-signature-ed25519", sig)
                .header("telnyx-timestamp", timestamp)
                .body(Body::from(body.to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    std::env::remove_var("TELNYX_PUBLIC_KEY");
}
