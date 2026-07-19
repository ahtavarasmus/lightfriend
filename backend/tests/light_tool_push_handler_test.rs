use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::put,
    Router,
};
use backend::{
    handlers::light_tool_handlers,
    pg_schema::light_tool_push_registrations,
    repositories::{
        light_tool_devices_repository::LightToolDevicesRepository,
        light_tool_push_repository::LightToolPushRepository,
    },
    services::light_tool_bootstrap::LightToolBootstrapService,
    test_utils::create_test_state,
};
use diesel::prelude::*;
use serde_json::{json, Value};
use tower::ServiceExt;

const INSTALLATION_ID: &str = "550e8400-e29b-41d4-a716-446655440010";
const ENDPOINT: &str = "https://push.light.example/v1/device/secret-token";
const ROTATED_ENDPOINT: &str = "https://push.light.example/v1/device/rotated-token";

#[tokio::test]
#[serial_test::serial]
async fn push_endpoint_registration_is_authenticated_encrypted_and_rotatable() {
    let state = create_test_state();
    let session =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
            .bootstrap(INSTALLATION_ID, None, 1_700_000_000)
            .unwrap();
    let app = push_router(state.clone());

    let missing_auth = put_endpoint(&app, None, ENDPOINT).await;
    assert_eq!(missing_auth.status(), StatusCode::UNAUTHORIZED);

    let registered = put_endpoint(&app, Some(&session.device_token), ENDPOINT).await;
    assert_eq!(registered.status(), StatusCode::NO_CONTENT);

    let device = LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_by_installation_hash(
            &backend::services::light_tool_identity::hash_installation_id(INSTALLATION_ID).unwrap(),
        )
        .unwrap()
        .unwrap();
    let repository = LightToolPushRepository::new(state.pg_pool.clone());
    let stored = repository.find_for_device(device.id).unwrap().unwrap();
    assert_eq!(stored.endpoint, ENDPOINT);
    assert_ne!(stored.endpoint_hash, ENDPOINT);

    let encrypted_endpoint = {
        let mut conn = state.pg_pool.get().unwrap();
        light_tool_push_registrations::table
            .find(device.id)
            .select(light_tool_push_registrations::encrypted_endpoint)
            .first::<String>(&mut conn)
            .unwrap()
    };
    assert_ne!(encrypted_endpoint, ENDPOINT);

    let rotated = put_endpoint(&app, Some(&session.device_token), ROTATED_ENDPOINT).await;
    assert_eq!(rotated.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        repository
            .find_for_device(device.id)
            .unwrap()
            .unwrap()
            .endpoint,
        ROTATED_ENDPOINT
    );

    let deleted = delete_endpoint(&app, Some(&session.device_token)).await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    assert!(repository.find_for_device(device.id).unwrap().is_none());
    assert_eq!(
        delete_endpoint(&app, Some(&session.device_token))
            .await
            .status(),
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
#[serial_test::serial]
async fn push_endpoint_registration_rejects_unsafe_urls() {
    let state = create_test_state();
    let session =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
            .bootstrap(INSTALLATION_ID, None, 1_700_000_000)
            .unwrap();
    let app = push_router(state);

    for endpoint in [
        "",
        "not-a-url",
        "http://push.light.example/device/token",
        "https://user:password@push.light.example/device/token",
        "https://push.light.example/device/token#fragment",
        "https://localhost/device/token",
        "https://127.0.0.1/device/token",
        "https://10.0.0.1/device/token",
        "https://169.254.169.254/device/token",
        "https://[::1]/device/token",
        "https://[fc00::1]/device/token",
        "https://[fe80::1]/device/token",
    ] {
        let response = put_endpoint(&app, Some(&session.device_token), endpoint).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{endpoint}");
        assert_eq!(
            response_json(response).await["error"],
            "push endpoint is invalid"
        );
    }
}

fn push_router(state: std::sync::Arc<backend::AppState>) -> Router {
    Router::new()
        .route(
            "/api/light-tool/push",
            put(light_tool_handlers::register_push_endpoint)
                .delete(light_tool_handlers::unregister_push_endpoint),
        )
        .with_state(state)
}

async fn put_endpoint(
    app: &Router,
    device_token: Option<&str>,
    endpoint: &str,
) -> axum::response::Response {
    let mut builder = Request::builder()
        .method("PUT")
        .uri("/api/light-tool/push")
        .header("content-type", "application/json");
    if let Some(token) = device_token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(
            builder
                .body(Body::from(json!({ "endpoint": endpoint }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn delete_endpoint(app: &Router, device_token: Option<&str>) -> axum::response::Response {
    let mut builder = Request::builder()
        .method("DELETE")
        .uri("/api/light-tool/push");
    if let Some(token) = device_token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    app.clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}
