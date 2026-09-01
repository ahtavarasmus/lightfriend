use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use backend::{
    handlers::{admin_handlers, auth_handlers},
    test_utils::{create_test_state, create_test_user, TestUserParams},
    UserCoreOps,
};
use serial_test::serial;
use tower::ServiceExt;

const TEST_UNSUBSCRIBE_SECRET: &str = "test-unsubscribe-secret";

#[test]
fn unsubscribe_tokens_are_bound_to_normalized_email() {
    let token =
        admin_handlers::generate_unsubscribe_token("User@Example.com", TEST_UNSUBSCRIBE_SECRET);

    assert!(admin_handlers::verify_unsubscribe_token(
        " user@example.COM ",
        &token,
        TEST_UNSUBSCRIBE_SECRET,
    ));
    assert!(!admin_handlers::verify_unsubscribe_token(
        "other@example.com",
        &token,
        TEST_UNSUBSCRIBE_SECRET,
    ));

    let mut tampered = token;
    let replacement = if tampered.starts_with('0') { "1" } else { "0" };
    tampered.replace_range(..1, replacement);
    assert!(!admin_handlers::verify_unsubscribe_token(
        "user@example.com",
        &tampered,
        TEST_UNSUBSCRIBE_SECRET,
    ));
}

#[tokio::test]
#[serial]
async fn unsubscribe_requires_a_valid_token_and_hides_recipient_existence() {
    std::env::set_var("JWT_SECRET_KEY", TEST_UNSUBSCRIBE_SECRET);
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let app = Router::new()
        .route("/api/unsubscribe", get(admin_handlers::unsubscribe))
        .with_state(state.clone());

    let invalid = unsubscribe_request(&app, &user.email, "invalid").await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert!(state.user_core.get_user_settings(user.id).unwrap().notify);

    let token = admin_handlers::generate_unsubscribe_token(&user.email, TEST_UNSUBSCRIBE_SECRET);
    let known = unsubscribe_request(&app, &user.email, &token).await;
    assert_eq!(known.status(), StatusCode::OK);
    let known_body = response_text(known).await;
    assert!(!state.user_core.get_user_settings(user.id).unwrap().notify);

    let unknown_email = "missing@example.com";
    let unknown_token =
        admin_handlers::generate_unsubscribe_token(unknown_email, TEST_UNSUBSCRIBE_SECRET);
    let unknown = unsubscribe_request(&app, unknown_email, &unknown_token).await;
    assert_eq!(unknown.status(), StatusCode::OK);
    assert_eq!(response_text(unknown).await, known_body);
}

#[tokio::test]
#[serial]
async fn phone_verify_request_returns_the_same_response_for_known_and_unknown_numbers() {
    std::env::set_var("ENVIRONMENT", "development");
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let app = Router::new()
        .route(
            "/api/phone-verify/request",
            post(auth_handlers::request_phone_verify),
        )
        .with_state(state.clone());

    let known = phone_verify_request(&app, &user.phone_number).await;
    let unknown_phone = "+14155559999";
    let unknown = phone_verify_request(&app, unknown_phone).await;

    assert_eq!(known.status(), StatusCode::OK);
    assert_eq!(unknown.status(), StatusCode::OK);
    assert_eq!(response_text(known).await, response_text(unknown).await);
    assert!(state.phone_verify_otps.contains_key(&user.phone_number));
    assert!(!state.phone_verify_otps.contains_key(unknown_phone));
}

#[test]
fn session_token_is_consumed_atomically() {
    let tokens = std::sync::Arc::new(dashmap::DashMap::new());
    tokens.insert("single-use-session".to_string(), "secret-token".to_string());
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));

    let workers: Vec<_> = (0..2)
        .map(|_| {
            let tokens = tokens.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                auth_handlers::consume_session_token(&tokens, "single-use-session")
            })
        })
        .collect();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();

    assert_eq!(results.iter().filter(|token| token.is_some()).count(), 1);
    assert_eq!(results.iter().filter(|token| token.is_none()).count(), 1);
}

async fn unsubscribe_request(app: &Router, email: &str, token: &str) -> axum::response::Response {
    let uri = format!(
        "/api/unsubscribe?email={}&token={}",
        urlencoding::encode(email),
        token
    );
    app.clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn phone_verify_request(app: &Router, phone_number: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/phone-verify/request")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"phone_number": phone_number}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn response_text(response: axum::response::Response) -> String {
    let body = to_bytes(response.into_body(), 8 * 1024).await.unwrap();
    String::from_utf8(body.to_vec()).unwrap()
}
