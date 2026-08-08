use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::routing::post;
use axum::Router;
use backend::pg_schema::{agent_credentials, ont_events};
use backend::repositories::agent_integration_repository::{
    AgentIntegrationRepository, CredentialClaim, CredentialIssue, IdempotencyClaim, PairingPoll,
};
use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use diesel::prelude::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

fn hash(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn mint_credential(
    state: &std::sync::Arc<backend::AppState>,
    user_id: i32,
    now: i32,
    raw_token: &str,
) -> backend::models::agent_integration_models::AgentCredential {
    let repository = AgentIntegrationRepository::new(state.pg_pool.clone());
    repository
        .create_pairing(
            &hash("lfpair_device"),
            &hash("ABCDEFGHJKLM"),
            "Codex",
            now,
            now + 600,
        )
        .unwrap();
    assert_eq!(
        repository
            .approve_pairing(user_id, &hash("ABCDEFGHJKLM"), now + 1)
            .unwrap(),
        Some("Codex".to_string())
    );
    assert_eq!(
        repository
            .poll_pairing(&hash("lfpair_device"), now + 2)
            .unwrap(),
        PairingPoll::Approved {
            user_id,
            label: "Codex".to_string(),
        }
    );
    repository
        .consume_pairing(
            &hash("lfpair_device"),
            CredentialIssue {
                user_id,
                token_hash: &hash(raw_token),
                token_prefix: "lfagent_aaaaaaaa",
                label: "Codex",
                issued_at: now + 2,
                expires_at: now + 90 * 86_400,
            },
        )
        .unwrap()
        .unwrap()
}

#[test]
#[serial_test::serial]
fn pairing_tokens_are_one_time_hashed_scoped_and_revocable() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;
    let raw = format!("lfagent_{}", "a".repeat(64));
    let credential = mint_credential(&state, user.id, now, &raw);
    let repository = AgentIntegrationRepository::new(state.pg_pool.clone());
    assert!(repository
        .consume_pairing(
            &hash("lfpair_device"),
            CredentialIssue {
                user_id: user.id,
                token_hash: &hash("unused-second-token"),
                token_prefix: "lfagent_unused",
                label: "Codex",
                issued_at: now + 3,
                expires_at: now + 90 * 86_400,
            },
        )
        .unwrap()
        .is_none());

    let mut conn = state.pg_pool.get().unwrap();
    let stored_hash = agent_credentials::table
        .find(credential.id)
        .select(agent_credentials::token_hash)
        .first::<String>(&mut conn)
        .unwrap();
    assert_eq!(stored_hash, hash(&raw));
    assert_ne!(stored_hash, raw);
    assert_eq!(credential.scopes, "reminders,reply_watch_email");

    assert!(matches!(
        repository.claim_credential(&hash(&raw), "reminders", now + 3),
        Ok(CredentialClaim::Accepted(_))
    ));
    let fresh = repository
        .reserve_idempotency(credential.id, "reminder", &hash("request-1"), now + 3)
        .unwrap();
    let IdempotencyClaim::Fresh(row_id) = fresh else {
        panic!("expected a fresh idempotency reservation");
    };
    repository.complete_idempotency(row_id, "accepted").unwrap();
    assert_eq!(
        repository
            .reserve_idempotency(credential.id, "reminder", &hash("request-1"), now + 4)
            .unwrap(),
        IdempotencyClaim::Replayed("accepted".to_string())
    );

    assert!(repository
        .revoke_by_token_hash(&hash(&raw), now + 5)
        .unwrap());
    assert!(repository
        .authenticate_credential(&hash(&raw), "reminders", now + 6)
        .unwrap()
        .is_none());
}

#[tokio::test]
#[serial_test::serial]
async fn reminder_action_returns_only_status_and_rejects_query_tokens() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;
    let raw = format!("lfagent_{}", "b".repeat(64));
    mint_credential(&state, user.id, now, &raw);
    let app = Router::new()
        .route(
            "/api/agent/actions/reminders",
            post(backend::handlers::agent_integration_handlers::create_reminder),
        )
        .with_state(state.clone());
    let body = json!({
        "message": "Call the dentist",
        "at": (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339(),
    });
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/actions/reminders")
                .header("authorization", format!("Bearer {raw}"))
                .header("idempotency-key", "request-2")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    let response_json: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(response_json, json!({"status": "accepted"}));

    let mut conn = state.pg_pool.get().unwrap();
    assert_eq!(
        ont_events::table
            .filter(ont_events::user_id.eq(user.id))
            .filter(ont_events::description.eq("Call the dentist"))
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap(),
        1
    );

    let leaked = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/agent/actions/reminders?token={raw}"))
                .header("authorization", format!("Bearer {raw}"))
                .header("idempotency-key", "request-3")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(leaked.status(), StatusCode::BAD_REQUEST);

    let missing_idempotency = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/actions/reminders")
                .header("authorization", format!("Bearer {raw}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_idempotency.status(), StatusCode::BAD_REQUEST);
}
