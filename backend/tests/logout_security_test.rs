use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use backend::handlers::auth_handlers;
use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use backend::UserCoreOps;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serial_test::serial;
use sha2::{Digest, Sha256};

fn refresh_token(secret: &str, user_id: i32, nonce: &str) -> String {
    encode(
        &Header::default(),
        &serde_json::json!({
            "sub": user_id,
            "exp": (Utc::now() + Duration::hours(1)).timestamp(),
            "type": "refresh",
            "nonce": nonce,
        }),
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

fn cookie_headers(token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        HeaderValue::from_str(&format!("refresh_token={token}")).unwrap(),
    );
    headers
}

#[tokio::test]
#[serial]
async fn logout_revokes_the_presented_refresh_token() {
    let secret = "logout-security-test-secret";
    std::env::set_var("JWT_REFRESH_KEY", secret);
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let token = refresh_token(secret, user.id, "active");
    state
        .user_core
        .set_refresh_token_hash(user.id, &token_hash(&token))
        .unwrap();

    let response = auth_handlers::logout(State(state.clone()), cookie_headers(&token)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let reloaded = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert!(reloaded.refresh_token_hash.is_none());
}

#[tokio::test]
#[serial]
async fn logout_cannot_revoke_a_different_active_refresh_token() {
    let secret = "logout-security-test-secret";
    std::env::set_var("JWT_REFRESH_KEY", secret);
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let active_token = refresh_token(secret, user.id, "active");
    let stale_token = refresh_token(secret, user.id, "stale");
    let active_hash = token_hash(&active_token);
    state
        .user_core
        .set_refresh_token_hash(user.id, &active_hash)
        .unwrap();

    let response = auth_handlers::logout(State(state.clone()), cookie_headers(&stale_token)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let reloaded = state.user_core.find_by_id(user.id).unwrap().unwrap();
    assert_eq!(
        reloaded.refresh_token_hash.as_deref(),
        Some(active_hash.as_str())
    );
}
