use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    middleware,
    routing::post,
    Router,
};
use backend::{
    api::{
        twilio_utils::{compute_twilio_signature, validate_twilio_signature},
        voice_pipeline,
    },
    test_utils::{create_test_state, create_test_user, TestUserParams},
};
use serial_test::serial;
use std::collections::BTreeMap;
use tower::ServiceExt;

#[tokio::test]
async fn voice_incoming_unknown_caller_rejects_without_answering() {
    std::env::set_var("SERVER_URL", "https://lightfriend.ai");
    std::env::set_var("TWILIO_AUTH_TOKEN", "test_twilio_auth_token");

    let state = create_test_state();
    let body = "CallSid=CA123&From=%2B8175831087&To=%2B18005551234";

    let mut params = BTreeMap::new();
    params.insert("CallSid".to_string(), "CA123".to_string());
    params.insert("From".to_string(), "+8175831087".to_string());
    params.insert("To".to_string(), "+18005551234".to_string());
    let signature = compute_twilio_signature(
        "https://lightfriend.ai/api/voice/incoming",
        &params,
        "test_twilio_auth_token",
    );

    let app = Router::new()
        .route("/api/voice/incoming", post(voice_pipeline::voice_incoming))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            validate_twilio_signature,
        ))
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/voice/incoming")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("X-Twilio-Signature", signature)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    let twiml = String::from_utf8(body.to_vec()).unwrap();
    assert!(twiml.contains(r#"<Reject reason="rejected" />"#));
    assert!(!twiml.contains("<Say>"));
}

#[tokio::test]
#[serial]
async fn voice_incoming_registered_caller_speaks_before_streaming() {
    std::env::set_var("SERVER_URL", "https://lightfriend.ai");
    std::env::set_var("TWILIO_AUTH_TOKEN", "test_twilio_auth_token");
    std::env::set_var("OPENAI_API_KEY", "test_openai_key");

    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let body = format!(
        "CallSid=CA123&From={}&To=%2B18005551234",
        urlencoding::encode(&user.phone_number)
    );

    let mut params = BTreeMap::new();
    params.insert("CallSid".to_string(), "CA123".to_string());
    params.insert("From".to_string(), user.phone_number);
    params.insert("To".to_string(), "+18005551234".to_string());
    let signature = compute_twilio_signature(
        "https://lightfriend.ai/api/voice/incoming",
        &params,
        "test_twilio_auth_token",
    );

    let app = Router::new()
        .route("/api/voice/incoming", post(voice_pipeline::voice_incoming))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            validate_twilio_signature,
        ))
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/voice/incoming")
                .header("content-type", "application/x-www-form-urlencoded")
                .header("X-Twilio-Signature", signature)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let twiml = String::from_utf8(body.to_vec()).unwrap();
    assert!(twiml.contains("<Say>Connecting Lightfriend.</Say>"));
    assert!(twiml.contains(r#"<Connect>"#));
    assert!(twiml.contains(r#"<Stream url="wss://lightfriend.ai/api/voice/ws""#));
    assert!(twiml.contains(&format!(
        r#"<Parameter name="user_id" value="{}" />"#,
        user.id
    )));
}

#[test]
fn realtime_failure_notice_is_claimed_once() {
    let mut sent = false;
    assert!(voice_pipeline::claim_voice_failure_notice(&mut sent));
    assert!(sent);
    assert!(!voice_pipeline::claim_voice_failure_notice(&mut sent));
}
