use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use backend::{
    handlers::light_tool_handlers,
    pg_schema::{light_tool_devices, light_tool_runs},
    repositories::{
        light_tool_devices_repository::LightToolDevicesRepository,
        light_tool_runs_repository::LightToolRunsRepository,
    },
    services::{
        light_tool_bootstrap::{LightToolBootstrapService, TRIAL_DURATION_SECONDS},
        light_tool_identity::hash_device_token,
        light_tool_run_dispatcher::{LightToolResponder, LightToolRunPrincipal},
    },
    test_utils::{create_test_state, create_test_user, TestUserParams},
    utils::encryption::encrypt,
};
use diesel::prelude::*;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tower::ServiceExt;
use uuid::Uuid;

const INSTALLATION_ID: &str = "550e8400-e29b-41d4-a716-446655440000";
const OTHER_INSTALLATION_ID: &str = "550e8400-e29b-41d4-a716-446655440001";
const TEST_ENCRYPTION_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

#[tokio::test]
#[serial_test::serial]
async fn message_endpoint_creates_and_idempotently_replays_a_queued_run() {
    std::env::set_var("ENCRYPTION_KEY", TEST_ENCRYPTION_KEY);
    let state = create_test_state();
    let now = chrono::Utc::now().timestamp() as i32;
    let session =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
            .bootstrap(INSTALLATION_ID, None, now)
            .unwrap();
    let app = light_tool_router(state.clone());
    let client_message_id = Uuid::new_v4().to_string();

    let created = send_request(
        &app,
        Some(&session.device_token),
        &client_message_id,
        "  What is the weather?  ",
    )
    .await;
    assert_eq!(created.status(), StatusCode::ACCEPTED);
    let created_json = response_json(created).await;
    let run_id = created_json["run_id"].as_str().unwrap().to_string();
    assert!(Uuid::parse_str(&run_id).is_ok());

    let replay = send_request(
        &app,
        Some(&session.device_token),
        &client_message_id,
        "Changed retry text",
    )
    .await;
    assert_eq!(replay.status(), StatusCode::ACCEPTED);
    assert_eq!(response_json(replay).await["run_id"], run_id);

    let device = LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_active_by_token_hash(&hash_device_token(&session.device_token).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(device.trial_messages_used, 1);
    let run = LightToolRunsRepository::new(state.pg_pool.clone())
        .find_by_id_for_device(&run_id, device.id)
        .unwrap()
        .unwrap();
    assert_eq!(run.user_message, "What is the weather?");
    assert_eq!(run.status, "queued");

    let status = get_run_request(&app, Some(&session.device_token), &run_id).await;
    assert_eq!(status.status(), StatusCode::OK);
    let status = response_json(status).await;
    assert_eq!(status["run_id"], run_id);
    assert_eq!(status["state"], "queued");
    assert!(status["activity_text"].is_null());
    assert!(status["assistant_message"].is_null());
    assert!(status["error_message"].is_null());
}

#[tokio::test]
#[serial_test::serial]
async fn message_endpoint_requires_authentication_and_valid_content() {
    std::env::set_var("ENCRYPTION_KEY", TEST_ENCRYPTION_KEY);
    let state = create_test_state();
    let now = chrono::Utc::now().timestamp() as i32;
    let session =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
            .bootstrap(INSTALLATION_ID, None, now)
            .unwrap();
    let app = light_tool_router(state);
    let client_message_id = Uuid::new_v4().to_string();

    let missing_auth = send_request(&app, None, &client_message_id, "Hello").await;
    assert_eq!(missing_auth.status(), StatusCode::UNAUTHORIZED);

    let invalid_id = send_request(&app, Some(&session.device_token), "not-a-uuid", "Hello").await;
    assert_eq!(invalid_id.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(invalid_id).await["error"],
        "client_message_id must be a UUID"
    );

    let blank = send_request(&app, Some(&session.device_token), &client_message_id, "   ").await;
    assert_eq!(blank.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response_json(blank).await["error"], "text cannot be blank");

    let oversized = send_request(
        &app,
        Some(&session.device_token),
        &client_message_id,
        &"a".repeat(8_001),
    )
    .await;
    assert_eq!(oversized.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(oversized).await["error"],
        "text must be at most 8000 bytes"
    );
}

#[tokio::test]
#[serial_test::serial]
async fn message_endpoint_rejects_an_expired_trial() {
    std::env::set_var("ENCRYPTION_KEY", TEST_ENCRYPTION_KEY);
    let state = create_test_state();
    let now = chrono::Utc::now().timestamp() as i32;
    let session =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
            .bootstrap(INSTALLATION_ID, None, now - TRIAL_DURATION_SECONDS)
            .unwrap();
    let app = light_tool_router(state.clone());

    let response = send_request(
        &app,
        Some(&session.device_token),
        &Uuid::new_v4().to_string(),
        "Hello",
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(response).await["error"],
        "trial unavailable; connect a Lightfriend account to continue"
    );

    let device = LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_active_by_token_hash(&hash_device_token(&session.device_token).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(device.trial_messages_used, 0);
}

struct ImmediateFakeResponder;

#[async_trait]
impl LightToolResponder for ImmediateFakeResponder {
    async fn respond(
        &self,
        _principal: LightToolRunPrincipal,
        _history: &[backend::services::light_tool_run_dispatcher::LightToolConversationTurn],
        _user_message: &str,
        activity_tx: mpsc::Sender<String>,
    ) -> Result<String, String> {
        activity_tx.send("ANSWERING".to_string()).await.unwrap();
        Ok("Fake asynchronous reply".to_string())
    }
}

struct PrincipalCapturingResponder {
    principal: Arc<Mutex<Option<LightToolRunPrincipal>>>,
}

#[async_trait]
impl LightToolResponder for PrincipalCapturingResponder {
    async fn respond(
        &self,
        principal: LightToolRunPrincipal,
        _history: &[backend::services::light_tool_run_dispatcher::LightToolConversationTurn],
        _user_message: &str,
        _activity_tx: mpsc::Sender<String>,
    ) -> Result<String, String> {
        *self.principal.lock().unwrap() = Some(principal);
        Ok("Connected reply".to_string())
    }
}

#[tokio::test]
#[serial_test::serial]
async fn linked_message_snapshots_account_principal_without_using_trial_quota() {
    std::env::set_var("ENCRYPTION_KEY", TEST_ENCRYPTION_KEY);
    let captured_principal = Arc::new(Mutex::new(None));
    let mut state = create_test_state();
    Arc::get_mut(&mut state).unwrap().light_tool_responder =
        Some(Arc::new(PrincipalCapturingResponder {
            principal: captured_principal.clone(),
        }));
    let now = chrono::Utc::now().timestamp() as i32;
    let session =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
            .bootstrap(INSTALLATION_ID, None, now)
            .unwrap();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let token_hash = hash_device_token(&session.device_token).unwrap();
    let device = LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_active_by_token_hash(&token_hash)
        .unwrap()
        .unwrap();
    {
        let mut conn = state.pg_pool.get().unwrap();
        diesel::update(light_tool_devices::table.find(device.id))
            .set(light_tool_devices::user_id.eq(Some(user.id)))
            .execute(&mut conn)
            .unwrap();
    }
    let app = light_tool_router(state.clone());

    let response = send_request(
        &app,
        Some(&session.device_token),
        &Uuid::new_v4().to_string(),
        "Check my email",
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let run_id = response_json(response).await["run_id"]
        .as_str()
        .unwrap()
        .to_string();

    for _ in 0..100 {
        let run = LightToolRunsRepository::new(state.pg_pool.clone())
            .find_by_id_for_device(&run_id, device.id)
            .unwrap()
            .unwrap();
        if run.status == "completed" {
            assert_eq!(run.account_user_id, Some(user.id));
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    assert_eq!(
        *captured_principal.lock().unwrap(),
        Some(LightToolRunPrincipal::Account {
            device_id: device.id,
            user_id: user.id,
        })
    );
    let device = LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_active_by_token_hash(&token_hash)
        .unwrap()
        .unwrap();
    assert_eq!(device.trial_messages_used, 0);

    let mut other_user_params = TestUserParams::finland_user(10.0, 5.0);
    other_user_params.email = "other-light-tool@example.com".to_string();
    let other_user = create_test_user(&state, &other_user_params);
    {
        let mut conn = state.pg_pool.get().unwrap();
        diesel::update(light_tool_devices::table.find(device.id))
            .set(light_tool_devices::user_id.eq(Some(other_user.id)))
            .execute(&mut conn)
            .unwrap();
    }
    let old_account_run = get_run_request(&app, Some(&session.device_token), &run_id).await;
    assert_eq!(old_account_run.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[serial_test::serial]
async fn fresh_message_dispatches_injected_responder_in_background() {
    std::env::set_var("ENCRYPTION_KEY", TEST_ENCRYPTION_KEY);
    let mut state = create_test_state();
    Arc::get_mut(&mut state).unwrap().light_tool_responder = Some(Arc::new(ImmediateFakeResponder));
    let now = chrono::Utc::now().timestamp() as i32;
    let session =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
            .bootstrap(INSTALLATION_ID, None, now)
            .unwrap();
    let app = light_tool_router(state);

    let response = send_request(
        &app,
        Some(&session.device_token),
        &Uuid::new_v4().to_string(),
        "Hello",
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let run_id = response_json(response).await["run_id"]
        .as_str()
        .unwrap()
        .to_string();

    let mut completed = None;
    for _ in 0..100 {
        let response = get_run_request(&app, Some(&session.device_token), &run_id).await;
        assert_eq!(response.status(), StatusCode::OK);
        let status = response_json(response).await;
        if status["state"] == "completed" {
            completed = Some(status);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let completed = completed.expect("fake responder did not complete");
    assert_eq!(
        completed["assistant_message"]["text"],
        "Fake asynchronous reply"
    );
}

#[tokio::test]
#[serial_test::serial]
async fn run_status_endpoint_maps_progress_completion_and_failure() {
    std::env::set_var("ENCRYPTION_KEY", TEST_ENCRYPTION_KEY);
    let state = create_test_state();
    let now = chrono::Utc::now().timestamp() as i32;
    let session =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
            .bootstrap(INSTALLATION_ID, None, now)
            .unwrap();
    let app = light_tool_router(state.clone());

    let running_id = submit_message(&app, &session.device_token, "Running message").await;
    let completed_id = submit_message(&app, &session.device_token, "Completed message").await;
    let failed_id = submit_message(&app, &session.device_token, "Failed message").await;

    {
        let mut conn = state.pg_pool.get().unwrap();
        diesel::update(light_tool_runs::table.find(&running_id))
            .set((
                light_tool_runs::status.eq("running"),
                light_tool_runs::encrypted_activity_text
                    .eq(Some(encrypt("SEARCHING THE WEB").unwrap())),
                light_tool_runs::updated_at.eq(now + 1),
            ))
            .execute(&mut conn)
            .unwrap();
        diesel::update(light_tool_runs::table.find(&completed_id))
            .set((
                light_tool_runs::status.eq("completed"),
                light_tool_runs::encrypted_assistant_message
                    .eq(Some(encrypt("It will be sunny.").unwrap())),
                light_tool_runs::updated_at.eq(now + 2),
                light_tool_runs::completed_at.eq(Some(now + 2)),
            ))
            .execute(&mut conn)
            .unwrap();
        diesel::update(light_tool_runs::table.find(&failed_id))
            .set((
                light_tool_runs::status.eq("failed"),
                light_tool_runs::encrypted_error_message
                    .eq(Some(encrypt("Search temporarily unavailable").unwrap())),
                light_tool_runs::updated_at.eq(now + 3),
                light_tool_runs::completed_at.eq(Some(now + 3)),
            ))
            .execute(&mut conn)
            .unwrap();
    }

    let running =
        response_json(get_run_request(&app, Some(&session.device_token), &running_id).await).await;
    assert_eq!(running["state"], "running");
    assert_eq!(running["activity_text"], "SEARCHING THE WEB");
    assert!(running["assistant_message"].is_null());

    let completed =
        response_json(get_run_request(&app, Some(&session.device_token), &completed_id).await)
            .await;
    assert_eq!(completed["state"], "completed");
    assert_eq!(completed["assistant_message"]["id"], completed_id);
    assert_eq!(completed["assistant_message"]["text"], "It will be sunny.");
    assert!(chrono::DateTime::parse_from_rfc3339(
        completed["assistant_message"]["created_at"]
            .as_str()
            .unwrap()
    )
    .is_ok());
    assert!(completed["activity_text"].is_null());

    let failed =
        response_json(get_run_request(&app, Some(&session.device_token), &failed_id).await).await;
    assert_eq!(failed["state"], "failed");
    assert_eq!(failed["error_message"], "Search temporarily unavailable");
    assert!(failed["assistant_message"].is_null());
}

#[tokio::test]
#[serial_test::serial]
async fn message_history_returns_current_device_runs_and_resumable_state() {
    std::env::set_var("ENCRYPTION_KEY", TEST_ENCRYPTION_KEY);
    let state = create_test_state();
    let now = chrono::Utc::now().timestamp() as i32;
    let bootstrap =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()));
    let owner = bootstrap.bootstrap(INSTALLATION_ID, None, now).unwrap();
    let other = bootstrap
        .bootstrap(OTHER_INSTALLATION_ID, None, now)
        .unwrap();
    let app = light_tool_router(state.clone());

    let running_id = submit_message(&app, &owner.device_token, "Still working").await;
    let completed_id = submit_message(&app, &owner.device_token, "Finished question").await;
    let failed_id = submit_message(&app, &owner.device_token, "Failed question").await;
    {
        let mut conn = state.pg_pool.get().unwrap();
        diesel::update(light_tool_runs::table.find(&running_id))
            .set((
                light_tool_runs::status.eq("running"),
                light_tool_runs::encrypted_activity_text.eq(Some(encrypt("SEARCHING").unwrap())),
                light_tool_runs::updated_at.eq(now + 1),
            ))
            .execute(&mut conn)
            .unwrap();
        diesel::update(light_tool_runs::table.find(&completed_id))
            .set((
                light_tool_runs::status.eq("completed"),
                light_tool_runs::encrypted_assistant_message
                    .eq(Some(encrypt("Finished answer").unwrap())),
                light_tool_runs::updated_at.eq(now + 2),
                light_tool_runs::completed_at.eq(Some(now + 2)),
            ))
            .execute(&mut conn)
            .unwrap();
        diesel::update(light_tool_runs::table.find(&failed_id))
            .set((
                light_tool_runs::status.eq("failed"),
                light_tool_runs::encrypted_error_message.eq(Some(encrypt("Try again").unwrap())),
                light_tool_runs::updated_at.eq(now + 3),
                light_tool_runs::completed_at.eq(Some(now + 3)),
            ))
            .execute(&mut conn)
            .unwrap();
    }

    let history = response_json(get_history_request(&app, Some(&owner.device_token)).await).await;
    let runs = history["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0]["run_id"], running_id);
    assert_eq!(runs[0]["state"], "running");
    assert_eq!(runs[0]["activity_text"], "SEARCHING");
    assert_eq!(runs[0]["user_message"]["text"], "Still working");
    assert_eq!(runs[1]["run_id"], completed_id);
    assert_eq!(runs[1]["assistant_message"]["text"], "Finished answer");
    assert_eq!(runs[2]["run_id"], failed_id);
    assert_eq!(runs[2]["error_message"], "Try again");

    let other_history =
        response_json(get_history_request(&app, Some(&other.device_token)).await).await;
    assert_eq!(other_history["runs"], json!([]));
    assert_eq!(
        get_history_request(&app, None).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
#[serial_test::serial]
async fn run_status_endpoint_is_authenticated_and_device_scoped() {
    std::env::set_var("ENCRYPTION_KEY", TEST_ENCRYPTION_KEY);
    let state = create_test_state();
    let now = chrono::Utc::now().timestamp() as i32;
    let bootstrap =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()));
    let owner = bootstrap.bootstrap(INSTALLATION_ID, None, now).unwrap();
    let other = bootstrap
        .bootstrap(OTHER_INSTALLATION_ID, None, now)
        .unwrap();
    let app = light_tool_router(state);
    let run_id = submit_message(&app, &owner.device_token, "Private message").await;

    let unauthenticated = get_run_request(&app, None, &run_id).await;
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let other_device = get_run_request(&app, Some(&other.device_token), &run_id).await;
    assert_eq!(other_device.status(), StatusCode::NOT_FOUND);
    assert_eq!(response_json(other_device).await["error"], "run not found");

    let missing =
        get_run_request(&app, Some(&owner.device_token), &Uuid::new_v4().to_string()).await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let malformed = get_run_request(&app, Some(&owner.device_token), "not-a-uuid").await;
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(malformed).await["error"],
        "run_id must be a UUID"
    );
}

fn light_tool_router(state: std::sync::Arc<backend::AppState>) -> Router {
    Router::new()
        .route(
            "/api/light-tool/messages",
            post(light_tool_handlers::send_message).get(light_tool_handlers::get_message_history),
        )
        .route(
            "/api/light-tool/runs/{run_id}",
            get(light_tool_handlers::get_run_status),
        )
        .with_state(state)
}

async fn submit_message(app: &Router, device_token: &str, text: &str) -> String {
    let response = send_request(app, Some(device_token), &Uuid::new_v4().to_string(), text).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    response_json(response).await["run_id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn send_request(
    app: &Router,
    device_token: Option<&str>,
    client_message_id: &str,
    text: &str,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/light-tool/messages")
        .header("content-type", "application/json");
    if let Some(device_token) = device_token {
        builder = builder.header("authorization", format!("Bearer {device_token}"));
    }
    app.clone()
        .oneshot(
            builder
                .body(Body::from(
                    json!({
                        "client_message_id": client_message_id,
                        "text": text,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn get_run_request(
    app: &Router,
    device_token: Option<&str>,
    run_id: &str,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method("GET")
        .uri(format!("/api/light-tool/runs/{run_id}"));
    if let Some(device_token) = device_token {
        builder = builder.header("authorization", format!("Bearer {device_token}"));
    }
    app.clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn get_history_request(app: &Router, device_token: Option<&str>) -> axum::response::Response {
    let mut builder = Request::builder()
        .method("GET")
        .uri("/api/light-tool/messages");
    if let Some(device_token) = device_token {
        builder = builder.header("authorization", format!("Bearer {device_token}"));
    }
    app.clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), 32 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}
