use backend::channels::sinch_channel::SinchChannel;
use backend::channels::traits::{ChannelError, MediaRef, MessageChannel};
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
        preferred_sms_provider: None,
        accountability_friend_phone: None,
        accountability_friend_name: None,
        accountability_enabled: false,
    }
}

#[tokio::test]
async fn sinch_send_posts_correct_shape() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/xms/v1/svcplan-xyz/batches"))
        .and(header("authorization", "Bearer sinch-token"))
        .and(header("content-type", "application/json"))
        .respond_with(|req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap();
            assert_eq!(body["from"], "+18005551234");
            // Sinch wants `to` as an array
            let arr = body["to"].as_array().expect("to is array");
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0], "+12025551234");
            assert_eq!(body["body"], "hello world");
            ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": "sinch-batch-1"}))
        })
        .expect(1)
        .mount(&server)
        .await;

    let chan =
        SinchChannel::with_base_url("sinch-token", "svcplan-xyz", "+18005551234", server.uri());
    let result = chan
        .send(&user(), "+12025551234", "hello world", None)
        .await
        .unwrap();

    assert_eq!(result.as_str(), "sinch-batch-1");
}

#[tokio::test]
async fn sinch_returns_send_failed_on_http_4xx() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/xms/v1/svcplan/batches"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;

    let chan = SinchChannel::with_base_url("bad", "svcplan", "+18005551234", server.uri());
    let err = chan
        .send(&user(), "+12025551234", "hi", None)
        .await
        .unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("401"), "error should mention status: {msg}");
}

#[tokio::test]
async fn sinch_rejects_media() {
    let server = MockServer::start().await;
    // No mock — should never reach the server

    let chan = SinchChannel::with_base_url("k", "s", "+18005551234", server.uri());
    let err = chan
        .send(
            &user(),
            "+12025551234",
            "hi",
            Some(MediaRef::Url("https://example.com/img.jpg".to_string())),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ChannelError::MediaNotSupported));
}
