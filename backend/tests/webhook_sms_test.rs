//! Repository-level tests for the webhook-triggered SMS feature.
//!
//! These tests exercise the cap-claim transaction, the idempotency
//! reservation, and revoke semantics directly through the repository —
//! the HTTP handler is a thin wrapper that maps these outcomes to
//! status codes, so covering the repository covers the dangerous logic.

use axum::body::to_bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::Json;
use backend::handlers::auth_middleware::AuthUser;
use backend::handlers::webhook_sms_handlers::{
    create_token, webhook_sms, CreateTokenRequest, WebhookSmsRequest,
};
use backend::models::user_models::NewWebhookToken;
use backend::repositories::webhook_tokens_repository::{ClaimResult, IdempotencyResult};
use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use serial_test::serial;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_unix() -> i32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32
}

fn next_utc_midnight(now: i32) -> i32 {
    let day = 86_400;
    ((now / day) + 1) * day
}

fn hash(raw: &str) -> String {
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    hex::encode(h.finalize())
}

fn owner_auth(user_id: i32) -> AuthUser {
    AuthUser {
        user_id,
        is_admin: false,
    }
}

/// Helper: insert a token with the given cap and return (raw, row).
fn mint(
    state: &std::sync::Arc<backend::AppState>,
    user_id: i32,
    label: &str,
    cap: i32,
) -> (String, backend::models::user_models::WebhookToken) {
    let raw = format!("lf_{}", "a".repeat(32));
    let token_hash = hash(&raw);
    let now = now_unix();
    let row = state
        .webhook_tokens_repository
        .create(&NewWebhookToken {
            user_id,
            token_hash,
            token_prefix: raw.chars().take(8).collect(),
            label: label.to_string(),
            daily_cap: cap,
            daily_sent: 0,
            daily_reset_at: next_utc_midnight(now),
            created_at: now,
        })
        .expect("create webhook token");
    (raw, row)
}

#[tokio::test]
#[serial]
async fn create_and_lookup_token() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let (raw, row) = mint(&state, user.id, "deploy alerts", 50);

    let found = state
        .webhook_tokens_repository
        .find_by_hash(&hash(&raw))
        .unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.id, row.id);
    assert_eq!(found.label, "deploy alerts");
    assert_eq!(found.daily_cap, 50);
    assert_eq!(found.daily_sent, 0);
}

#[tokio::test]
#[serial]
async fn unknown_token_hash_returns_none() {
    let state = create_test_state();
    let _user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let res = state
        .webhook_tokens_repository
        .find_by_hash(&hash("lf_nonexistent"))
        .unwrap();
    assert!(res.is_none());
}

#[tokio::test]
#[serial]
async fn claim_increments_until_cap_then_overcaps() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let (raw, _row) = mint(&state, user.id, "cap test", 3);
    let h = hash(&raw);

    for expected_sent in 1..=3 {
        let res = state.webhook_tokens_repository.claim_send_slot(&h).unwrap();
        match res {
            ClaimResult::Ok { token } => assert_eq!(token.daily_sent, expected_sent),
            other => panic!("expected Ok, got {:?}", other),
        }
    }

    // 4th attempt: cap reached.
    let res = state.webhook_tokens_repository.claim_send_slot(&h).unwrap();
    match res {
        ClaimResult::OverCap {
            daily_cap,
            daily_sent,
        } => {
            assert_eq!(daily_cap, 3);
            assert_eq!(daily_sent, 3);
        }
        other => panic!("expected OverCap, got {:?}", other),
    }
}

#[tokio::test]
#[serial]
async fn revoke_then_claim_returns_revoked() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let (raw, row) = mint(&state, user.id, "to be revoked", 5);

    let ok = state
        .webhook_tokens_repository
        .revoke(user.id, row.id)
        .unwrap();
    assert!(ok);

    let res = state
        .webhook_tokens_repository
        .claim_send_slot(&hash(&raw))
        .unwrap();
    assert!(matches!(res, ClaimResult::Revoked));
}

#[tokio::test]
#[serial]
async fn revoke_is_idempotent_for_same_owner() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let (_raw, row) = mint(&state, user.id, "double revoke", 5);

    let first = state
        .webhook_tokens_repository
        .revoke(user.id, row.id)
        .unwrap();
    let second = state
        .webhook_tokens_repository
        .revoke(user.id, row.id)
        .unwrap();
    assert!(first, "first revoke should succeed");
    assert!(
        second,
        "second revoke should still report success (idempotent)"
    );
}

#[tokio::test]
#[serial]
async fn revoke_of_other_users_token_returns_false() {
    let state = create_test_state();
    let owner = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let attacker = create_test_user(
        &state,
        &TestUserParams {
            email: "attacker@example.com".to_string(),
            phone_number: "+14155550199".to_string(),
            credits: 0.0,
            credits_left: 0.0,
            sub_tier: Some("tier 2".to_string()),
        },
    );
    let (_raw, row) = mint(&state, owner.id, "owner token", 5);

    let revoked = state
        .webhook_tokens_repository
        .revoke(attacker.id, row.id)
        .unwrap();
    assert!(!revoked, "attacker must not revoke another user's token");
}

#[tokio::test]
#[serial]
async fn idempotency_fresh_then_inflight_then_replay() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let (_raw, row) = mint(&state, user.id, "idempotency test", 50);

    // First request: fresh.
    let first = state
        .webhook_tokens_repository
        .reserve_idempotency_key(row.id, "key-1")
        .unwrap();
    let row_id = match first {
        IdempotencyResult::Fresh { id } => id,
        other => panic!("expected Fresh, got {:?}", other),
    };

    // Concurrent retry while still in-flight.
    let second = state
        .webhook_tokens_repository
        .reserve_idempotency_key(row.id, "key-1")
        .unwrap();
    assert!(
        matches!(second, IdempotencyResult::InFlight),
        "second call before complete should be InFlight"
    );

    // Complete with a fake SID.
    state
        .webhook_tokens_repository
        .complete_idempotency(row_id, "SMxxxx")
        .unwrap();

    // Third request: replays the cached SID.
    let third = state
        .webhook_tokens_repository
        .reserve_idempotency_key(row.id, "key-1")
        .unwrap();
    match third {
        IdempotencyResult::Replayed { sid } => assert_eq!(sid, "SMxxxx"),
        other => panic!("expected Replayed, got {:?}", other),
    }
}

#[tokio::test]
#[serial]
async fn idempotency_clear_lets_retry_succeed() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let (_raw, row) = mint(&state, user.id, "clear retry", 50);

    let first = state
        .webhook_tokens_repository
        .reserve_idempotency_key(row.id, "key-clear")
        .unwrap();
    let row_id = match first {
        IdempotencyResult::Fresh { id } => id,
        other => panic!("expected Fresh, got {:?}", other),
    };

    // Send failed; release the in-flight row.
    state
        .webhook_tokens_repository
        .clear_idempotency(row_id)
        .unwrap();

    // Retry with the same key: should be Fresh again.
    let second = state
        .webhook_tokens_repository
        .reserve_idempotency_key(row.id, "key-clear")
        .unwrap();
    assert!(
        matches!(second, IdempotencyResult::Fresh { .. }),
        "after clear, retry should be Fresh"
    );
}

#[tokio::test]
#[serial]
async fn idempotency_keys_are_scoped_per_token() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    // Two different tokens for the same user.
    let raw_a = format!("lf_{}", "a".repeat(32));
    let raw_b = format!("lf_{}", "b".repeat(32));
    let now = now_unix();
    let token_a = state
        .webhook_tokens_repository
        .create(&NewWebhookToken {
            user_id: user.id,
            token_hash: hash(&raw_a),
            token_prefix: raw_a.chars().take(8).collect(),
            label: "alpha".to_string(),
            daily_cap: 50,
            daily_sent: 0,
            daily_reset_at: next_utc_midnight(now),
            created_at: now,
        })
        .unwrap();
    let token_b = state
        .webhook_tokens_repository
        .create(&NewWebhookToken {
            user_id: user.id,
            token_hash: hash(&raw_b),
            token_prefix: raw_b.chars().take(8).collect(),
            label: "beta".to_string(),
            daily_cap: 50,
            daily_sent: 0,
            daily_reset_at: next_utc_midnight(now),
            created_at: now,
        })
        .unwrap();

    // Same key, different token IDs: both should be Fresh.
    let res_a = state
        .webhook_tokens_repository
        .reserve_idempotency_key(token_a.id, "shared-key")
        .unwrap();
    let res_b = state
        .webhook_tokens_repository
        .reserve_idempotency_key(token_b.id, "shared-key")
        .unwrap();
    assert!(matches!(res_a, IdempotencyResult::Fresh { .. }));
    assert!(matches!(res_b, IdempotencyResult::Fresh { .. }));
}

#[tokio::test]
#[serial]
async fn concurrent_idempotency_reservations_have_one_winner_without_db_error() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let (_raw, token) = mint(&state, user.id, "concurrent retry", 50);
    let token_id = token.id;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));

    let (left, right) = tokio::task::spawn_blocking({
        let state = state.clone();
        move || {
            std::thread::scope(|scope| {
                let left_state = state.clone();
                let left_barrier = barrier.clone();
                let left = scope.spawn(move || {
                    left_barrier.wait();
                    left_state
                        .webhook_tokens_repository
                        .reserve_idempotency_key(token_id, "same-concurrent-key")
                });
                let right_state = state.clone();
                let right_barrier = barrier.clone();
                let right = scope.spawn(move || {
                    right_barrier.wait();
                    right_state
                        .webhook_tokens_repository
                        .reserve_idempotency_key(token_id, "same-concurrent-key")
                });
                (left.join().unwrap(), right.join().unwrap())
            })
        }
    })
    .await
    .unwrap();

    let outcomes = [left.unwrap(), right.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, IdempotencyResult::Fresh { .. }))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, IdempotencyResult::InFlight))
            .count(),
        1
    );
}

#[tokio::test]
#[serial]
async fn public_handler_replays_idempotent_send_without_consuming_another_slot() {
    let previous_environment = std::env::var("ENVIRONMENT").ok();
    std::env::set_var("ENVIRONMENT", "development");

    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let created = create_token(
        State(state.clone()),
        owner_auth(user.id),
        Json(CreateTokenRequest {
            label: "build alerts".into(),
            daily_cap: Some(5),
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        created.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    let created_body = to_bytes(created.into_body(), 16 * 1024).await.unwrap();
    let created_json: serde_json::Value = serde_json::from_slice(&created_body).unwrap();
    let raw_token = created_json["token"].as_str().unwrap().to_string();

    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {raw_token}")).unwrap(),
    );
    headers.insert("Idempotency-Key", HeaderValue::from_static("build-123"));

    for _ in 0..2 {
        let response = webhook_sms(
            State(state.clone()),
            headers.clone(),
            Json(WebhookSmsRequest {
                message: "Deploy finished".into(),
            }),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["status"], "accepted");
        assert_eq!(body["sid"], "dev_not_sending");
    }

    let token = state
        .webhook_tokens_repository
        .find_by_hash(&hash(&raw_token))
        .unwrap()
        .unwrap();
    assert_eq!(token.daily_sent, 1, "replay must not consume quota twice");

    match previous_environment {
        Some(value) => std::env::set_var("ENVIRONMENT", value),
        None => std::env::remove_var("ENVIRONMENT"),
    }
}

#[tokio::test]
#[serial]
async fn message_limit_counts_unicode_characters_not_utf8_bytes() {
    let previous_environment = std::env::var("ENVIRONMENT").ok();
    std::env::set_var("ENVIRONMENT", "development");

    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let (raw, _) = mint(&state, user.id, "unicode", 5);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {raw}")).unwrap(),
    );

    let accepted = webhook_sms(
        State(state.clone()),
        headers.clone(),
        Json(WebhookSmsRequest {
            message: "å".repeat(1500),
        }),
    )
    .await;
    assert!(accepted.is_ok());

    let rejected = webhook_sms(
        State(state),
        headers,
        Json(WebhookSmsRequest {
            message: "å".repeat(1501),
        }),
    )
    .await
    .expect_err("1501 characters must be rejected");
    assert_eq!(rejected.0, StatusCode::BAD_REQUEST);

    match previous_environment {
        Some(value) => std::env::set_var("ENVIRONMENT", value),
        None => std::env::remove_var("ENVIRONMENT"),
    }
}

#[tokio::test]
#[serial]
async fn claim_after_window_resets_counter() {
    // Mint a token whose daily_reset_at is already in the past and
    // daily_sent is at cap. The next claim should reset to 0 then
    // succeed with daily_sent=1.
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let raw = format!("lf_{}", "c".repeat(32));
    let token_hash = hash(&raw);
    let _row = state
        .webhook_tokens_repository
        .create(&NewWebhookToken {
            user_id: user.id,
            token_hash: token_hash.clone(),
            token_prefix: raw.chars().take(8).collect(),
            label: "expired window".to_string(),
            daily_cap: 1,
            daily_sent: 1,             // already at cap
            daily_reset_at: 1_000_000, // long ago
            created_at: now_unix(),
        })
        .unwrap();

    let res = state
        .webhook_tokens_repository
        .claim_send_slot(&token_hash)
        .unwrap();
    match res {
        ClaimResult::Ok { token } => {
            assert_eq!(token.daily_sent, 1, "reset to 0 then incremented");
            assert!(
                token.daily_reset_at > now_unix(),
                "daily_reset_at moved into the future"
            );
        }
        other => panic!("expected Ok after window reset, got {:?}", other),
    }
}
