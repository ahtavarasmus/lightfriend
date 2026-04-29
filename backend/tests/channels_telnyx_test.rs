use backend::channels::telnyx_channel::TelnyxChannel;
use backend::channels::traits::{MediaRef, MessageChannel};
use backend::models::user_models::User;
use serde_json::Value;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn user() -> User {
    User {
        id: 1,
        email: "test@example.com".to_string(),
        password_hash: String::new(),
        phone_number: "+12025551234".to_string(),
        nickname: None,
        time_to_live: None,
        credits: 0.0,
        preferred_number: None,
        charge_when_under: false,
        charge_back_to: None,
        stripe_customer_id: None,
        stripe_payment_method_id: None,
        stripe_checkout_session_id: None,
        sub_tier: None,
        credits_left: 0.0,
        last_credits_notification: None,
        next_billing_date_timestamp: None,
        magic_token: None,
        refresh_token_hash: None,
        refresh_token_compromised: false,
        magic_token_expires_at: None,
        plan_type: None,
        matrix_e2ee_enabled: false,
    }
}

#[tokio::test]
async fn telnyx_send_posts_correct_shape() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v2/messages"))
        .and(header("authorization", "Bearer test-key"))
        .and(header("content-type", "application/json"))
        .respond_with(|req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap();
            assert_eq!(body["from"], "+18005551234");
            assert_eq!(body["to"], "+12025551234");
            assert_eq!(body["text"], "hello world");
            assert_eq!(body["messaging_profile_id"], "profile-abc");
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"data": {"id": "telnyx-msg-1"}}))
        })
        .expect(1)
        .mount(&server)
        .await;

    let chan =
        TelnyxChannel::with_base_url("test-key", "profile-abc", "+18005551234", server.uri());
    let result = chan
        .send(&user(), "+12025551234", "hello world", None)
        .await
        .unwrap();

    assert_eq!(result.as_str(), "telnyx-msg-1");
}

#[tokio::test]
async fn telnyx_send_includes_media_url_array_when_provided() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v2/messages"))
        .respond_with(|req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap();
            let arr = body["media_urls"].as_array().expect("media_urls is array");
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0], "https://example.com/img.jpg");
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"data": {"id": "telnyx-msg-mms"}}))
        })
        .expect(1)
        .mount(&server)
        .await;

    let chan = TelnyxChannel::with_base_url("k", "p", "+18005551234", server.uri());
    chan.send(
        &user(),
        "+12025551234",
        "see attached",
        Some(MediaRef::Url("https://example.com/img.jpg".to_string())),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn telnyx_returns_send_failed_on_http_4xx() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v2/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
        .mount(&server)
        .await;

    let chan = TelnyxChannel::with_base_url("k", "p", "+18005551234", server.uri());
    let err = chan
        .send(&user(), "+12025551234", "hi", None)
        .await
        .unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("400"),
        "error message should mention status: {msg}"
    );
}

#[tokio::test]
async fn telnyx_rejects_inline_bytes_media() {
    let server = MockServer::start().await;
    // No mock — should never reach the server

    let chan = TelnyxChannel::with_base_url("k", "p", "+18005551234", server.uri());
    let err = chan
        .send(
            &user(),
            "+12025551234",
            "hi",
            Some(MediaRef::Bytes {
                data: vec![1, 2, 3],
                mime: "image/jpeg".to_string(),
            }),
        )
        .await
        .unwrap_err();
    matches!(
        err,
        backend::channels::traits::ChannelError::MediaNotSupported
    );
}
