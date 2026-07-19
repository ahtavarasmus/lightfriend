use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::{get, post},
    Json, Router,
};
use backend::{
    handlers::{light_tool_auth::LightToolDeviceAuth, light_tool_handlers},
    repositories::light_tool_devices_repository::LightToolDevicesRepository,
    services::{
        light_tool_bootstrap::{LightToolBootstrapService, TRIAL_MESSAGE_LIMIT},
        light_tool_identity::generate_device_token,
    },
    test_utils::create_test_state,
};
use serde_json::{json, Value};
use tower::ServiceExt;

const INSTALLATION_ID: &str = "550e8400-e29b-41d4-a716-446655440000";

#[tokio::test]
#[serial_test::serial]
async fn bootstrap_endpoint_matches_the_light_tool_contract() {
    let app = Router::new()
        .route(
            "/api/light-tool/bootstrap",
            post(light_tool_handlers::bootstrap),
        )
        .with_state(create_test_state());

    let created_response = request(&app, INSTALLATION_ID, None).await;
    assert_eq!(created_response.status(), StatusCode::OK);
    let created = response_json(created_response).await;
    let device_token = created["device_token"].as_str().unwrap().to_string();
    assert!(device_token.starts_with("lft_"));
    assert_eq!(created["can_send"], true);
    assert_eq!(created["trial"]["messages_remaining"], TRIAL_MESSAGE_LIMIT);
    assert!(
        chrono::DateTime::parse_from_rfc3339(created["trial"]["expires_at"].as_str().unwrap())
            .is_ok()
    );
    assert!(created["account"].is_null());

    let resumed_response = request(&app, INSTALLATION_ID, Some(&device_token)).await;
    assert_eq!(resumed_response.status(), StatusCode::OK);
    let resumed = response_json(resumed_response).await;
    assert_eq!(resumed["device_token"], device_token);
    assert_eq!(
        resumed["trial"]["expires_at"],
        created["trial"]["expires_at"]
    );

    let repeated_without_token = request(&app, INSTALLATION_ID, None).await;
    assert_eq!(repeated_without_token.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response_json(repeated_without_token).await["error"],
        "invalid device credentials"
    );

    let unknown_token = generate_device_token();
    let unknown_response = request(&app, INSTALLATION_ID, Some(&unknown_token.raw)).await;
    assert_eq!(unknown_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response_json(unknown_response).await["error"],
        "invalid device credentials"
    );

    let invalid_installation = request(&app, "not-a-uuid", None).await;
    assert_eq!(invalid_installation.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(invalid_installation).await["error"],
        "installation_id must be a UUID"
    );
}

#[tokio::test]
#[serial_test::serial]
async fn device_auth_extractor_accepts_only_active_bearer_tokens() {
    let state = create_test_state();
    let service =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()));
    let session = service
        .bootstrap(INSTALLATION_ID, None, 1_700_000_000)
        .unwrap();
    let app = Router::new()
        .route("/protected", get(authenticated_device))
        .with_state(state);

    let valid = protected_request(&app, Some(&session.device_token)).await;
    assert_eq!(valid.status(), StatusCode::OK);
    assert!(response_json(valid).await["device_id"].as_i64().unwrap() > 0);

    let missing = protected_request(&app, None).await;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response_json(missing).await["error"],
        "invalid device credentials"
    );

    let malformed = protected_request(&app, Some("not-a-device-token")).await;
    assert_eq!(malformed.status(), StatusCode::UNAUTHORIZED);

    let unknown_token = generate_device_token();
    let unknown = protected_request(&app, Some(&unknown_token.raw)).await;
    assert_eq!(unknown.status(), StatusCode::UNAUTHORIZED);
}

async fn authenticated_device(auth: LightToolDeviceAuth) -> Json<Value> {
    Json(json!({
        "device_id": auth.device.id,
        "user_id": auth.device.user_id,
    }))
}

async fn request(
    app: &Router,
    installation_id: &str,
    device_token: Option<&str>,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/light-tool/bootstrap")
        .header("content-type", "application/json");
    if let Some(token) = device_token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }

    app.clone()
        .oneshot(
            builder
                .body(Body::from(
                    json!({ "installation_id": installation_id }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn protected_request(app: &Router, device_token: Option<&str>) -> axum::response::Response {
    let mut builder = Request::builder().uri("/protected");
    if let Some(token) = device_token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}
