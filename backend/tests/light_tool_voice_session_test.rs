use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use backend::{
    api::voice_pipeline::{self, WebVoiceTicketError, WEB_VOICE_TICKET_TTL_SECONDS},
    handlers::light_tool_handlers,
    pg_schema::light_tool_devices,
    repositories::light_tool_devices_repository::LightToolDevicesRepository,
    services::{
        light_tool_bootstrap::LightToolBootstrapService, light_tool_identity::hash_device_token,
    },
    test_utils::{create_test_state, create_test_user, set_byot_credentials, TestUserParams},
};
use diesel::prelude::*;
use serde_json::Value;
use tower::ServiceExt;

const INSTALLATION_ID: &str = "550e8400-e29b-41d4-a716-446655440020";

#[tokio::test]
#[serial_test::serial]
async fn voice_session_requires_device_auth_and_a_connected_account() {
    let state = create_test_state();
    let session =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
            .bootstrap(INSTALLATION_ID, None, chrono::Utc::now().timestamp() as i32)
            .unwrap();
    let app = voice_router(state);

    let missing_auth = start_voice_session(&app, None).await;
    assert_eq!(missing_auth.status(), StatusCode::UNAUTHORIZED);

    let anonymous = start_voice_session(&app, Some(&session.device_token)).await;
    assert_eq!(anonymous.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(anonymous).await["error"],
        "connect a Lightfriend account to start a voice call"
    );
}

#[tokio::test]
#[serial_test::serial]
async fn paired_device_receives_a_short_lived_single_use_voice_ticket() {
    std::env::set_var("OPENAI_API_KEY", "test_openai_key");
    let state = create_test_state();
    let session =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
            .bootstrap(INSTALLATION_ID, None, chrono::Utc::now().timestamp() as i32)
            .unwrap();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    set_byot_credentials(&state, user.id);
    let device = LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_active_by_token_hash(&hash_device_token(&session.device_token).unwrap())
        .unwrap()
        .unwrap();
    {
        let mut conn = state.pg_pool.get().unwrap();
        diesel::update(light_tool_devices::table.find(device.id))
            .set(light_tool_devices::user_id.eq(Some(user.id)))
            .execute(&mut conn)
            .unwrap();
    }
    let app = voice_router(state.clone());

    let response = start_voice_session(&app, Some(&session.device_token)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["expires_in_seconds"], WEB_VOICE_TICKET_TTL_SECONDS);
    assert!(body.get("user_id").is_none());
    assert!(body.get("token").is_none());

    let ws_url = body["ws_url"].as_str().unwrap();
    let token = ws_url
        .strip_prefix("/api/voice/web-ws?token=")
        .expect("voice endpoint should return the shared WebSocket URL");
    assert_eq!(
        voice_pipeline::consume_web_voice_ticket(&state, token),
        Ok(user.id)
    );
    assert_eq!(
        voice_pipeline::consume_web_voice_ticket(&state, token),
        Err(WebVoiceTicketError::Invalid)
    );
}

fn voice_router(state: std::sync::Arc<backend::AppState>) -> Router {
    Router::new()
        .route(
            "/api/light-tool/voice/start",
            post(light_tool_handlers::start_voice_session),
        )
        .with_state(state)
}

async fn start_voice_session(app: &Router, device_token: Option<&str>) -> axum::response::Response {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/light-tool/voice/start");
    if let Some(device_token) = device_token {
        builder = builder.header("authorization", format!("Bearer {device_token}"));
    }
    app.clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), 8 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}
