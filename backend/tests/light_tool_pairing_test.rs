use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use backend::{
    handlers::{
        auth_middleware::AuthUser,
        light_tool_handlers::{
            self, create_pairing_offer, get_pairing_status, PairingStatusResponseKind,
        },
    },
    pg_schema::light_tool_pairing_sessions,
    repositories::{
        light_tool_devices_repository::LightToolDevicesRepository,
        light_tool_pairing_repository::{LightToolPairingRepository, PairingConsumption},
    },
    services::{
        light_tool_bootstrap::LightToolBootstrapService,
        light_tool_pairing::{
            LightToolPairingError, LightToolPairingService, LightToolPairingStatus,
            PAIRING_TTL_SECONDS,
        },
    },
    test_utils::{create_test_state, create_test_user, TestUserParams},
};
use diesel::prelude::*;
use serde_json::{json, Value};
use std::sync::{Arc, Barrier};
use tower::ServiceExt;

const INSTALLATION_ID: &str = "550e8400-e29b-41d4-a716-446655440000";
const OTHER_INSTALLATION_ID: &str = "550e8400-e29b-41d4-a716-446655440001";
const NOW: i32 = 1_700_000_000;

#[test]
#[serial_test::serial]
fn pairing_offer_is_hashed_replaced_and_expires() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let bootstrap =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()));
    let device = bootstrap.bootstrap(INSTALLATION_ID, None, NOW).unwrap();
    let device_id = LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_active_by_token_hash(
            &backend::services::light_tool_identity::hash_device_token(&device.device_token)
                .unwrap(),
        )
        .unwrap()
        .unwrap()
        .id;
    let service =
        LightToolPairingService::new(LightToolPairingRepository::new(state.pg_pool.clone()));

    let first = service.create_offer(user.id, NOW).unwrap();
    let raw_token = url::Url::parse(&first.pairing_uri)
        .unwrap()
        .query_pairs()
        .find(|(name, _)| name == "token")
        .unwrap()
        .1
        .to_string();
    let stored_hash = {
        let mut conn = state.pg_pool.get().unwrap();
        light_tool_pairing_sessions::table
            .find(user.id)
            .select(light_tool_pairing_sessions::token_hash)
            .first::<String>(&mut conn)
            .unwrap()
    };
    assert_ne!(stored_hash, raw_token);
    assert!(!stored_hash.contains(&raw_token));

    let replacement = service.create_offer(user.id, NOW + 1).unwrap();
    assert_eq!(
        service
            .consume_uri(&first.pairing_uri, device_id, NOW + 2)
            .unwrap(),
        PairingConsumption::InvalidOrExpired
    );
    assert_eq!(
        service
            .consume_uri(
                &replacement.pairing_uri,
                device_id,
                NOW + 1 + PAIRING_TTL_SECONDS,
            )
            .unwrap(),
        PairingConsumption::InvalidOrExpired
    );
    assert!(matches!(
        service.consume_uri("https://example.com/pair?token=nope", device_id, NOW),
        Err(LightToolPairingError::InvalidOrExpired)
    ));
}

#[test]
#[serial_test::serial]
fn pairing_status_tracks_the_current_session() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let bootstrap =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()));
    bootstrap.bootstrap(INSTALLATION_ID, None, NOW).unwrap();
    let device = LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_by_installation_hash(
            &backend::services::light_tool_identity::hash_installation_id(INSTALLATION_ID).unwrap(),
        )
        .unwrap()
        .unwrap();
    let service =
        LightToolPairingService::new(LightToolPairingRepository::new(state.pg_pool.clone()));

    assert_eq!(
        service.status_for_user(user.id, NOW).unwrap(),
        LightToolPairingStatus::None
    );
    let first = service.create_offer(user.id, NOW).unwrap();
    assert_eq!(
        service.status_for_user(user.id, NOW).unwrap(),
        LightToolPairingStatus::Pending
    );
    assert_eq!(
        service
            .status_for_user(user.id, NOW + PAIRING_TTL_SECONDS)
            .unwrap(),
        LightToolPairingStatus::Expired
    );

    let replacement = service
        .create_offer(user.id, NOW + PAIRING_TTL_SECONDS + 1)
        .unwrap();
    assert_ne!(first.pairing_uri, replacement.pairing_uri);
    service
        .consume_uri(
            &replacement.pairing_uri,
            device.id,
            NOW + PAIRING_TTL_SECONDS + 2,
        )
        .unwrap();
    assert_eq!(
        service
            .status_for_user(user.id, NOW + PAIRING_TTL_SECONDS + 2)
            .unwrap(),
        LightToolPairingStatus::Connected
    );
}

#[test]
#[serial_test::serial]
fn one_pairing_offer_cannot_link_two_devices_concurrently() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let bootstrap =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()));
    bootstrap.bootstrap(INSTALLATION_ID, None, NOW).unwrap();
    bootstrap
        .bootstrap(OTHER_INSTALLATION_ID, None, NOW)
        .unwrap();
    let repository = LightToolDevicesRepository::new(state.pg_pool.clone());
    let first_device = repository
        .find_by_installation_hash(
            &backend::services::light_tool_identity::hash_installation_id(INSTALLATION_ID).unwrap(),
        )
        .unwrap()
        .unwrap();
    let second_device = repository
        .find_by_installation_hash(
            &backend::services::light_tool_identity::hash_installation_id(OTHER_INSTALLATION_ID)
                .unwrap(),
        )
        .unwrap()
        .unwrap();
    let service = Arc::new(LightToolPairingService::new(
        LightToolPairingRepository::new(state.pg_pool.clone()),
    ));
    let offer = service.create_offer(user.id, NOW).unwrap();
    let start = Arc::new(Barrier::new(2));

    let handles = [first_device.id, second_device.id]
        .into_iter()
        .map(|device_id| {
            let service = service.clone();
            let start = start.clone();
            let uri = offer.pairing_uri.clone();
            std::thread::spawn(move || {
                start.wait();
                service.consume_uri(&uri, device_id, NOW + 1).unwrap()
            })
        })
        .collect::<Vec<_>>();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(
                result,
                PairingConsumption::Linked {
                    newly_linked: true,
                    ..
                }
            ))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, PairingConsumption::InvalidOrExpired))
            .count(),
        1
    );
}

#[tokio::test]
#[serial_test::serial]
async fn pairing_endpoints_connect_bootstrap_and_disconnect() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let session =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
            .bootstrap(INSTALLATION_ID, None, chrono::Utc::now().timestamp() as i32)
            .unwrap();
    let offer = create_pairing_offer(
        State(state.clone()),
        AuthUser {
            user_id: user.id,
            is_admin: false,
        },
    )
    .await
    .unwrap()
    .0;
    assert!(offer
        .pairing_uri
        .starts_with("lightfriend://pair?token=lfp_"));
    assert!(chrono::DateTime::parse_from_rfc3339(&offer.expires_at).is_ok());
    assert_eq!(
        get_pairing_status(
            State(state.clone()),
            AuthUser {
                user_id: user.id,
                is_admin: false,
            },
        )
        .await
        .unwrap()
        .0
        .status,
        PairingStatusResponseKind::Pending
    );

    let app = Router::new()
        .route(
            "/api/light-tool/pair",
            post(light_tool_handlers::consume_pairing_offer)
                .delete(light_tool_handlers::disconnect_account),
        )
        .route(
            "/api/light-tool/bootstrap",
            post(light_tool_handlers::bootstrap),
        )
        .with_state(state.clone());

    let paired = pair_request(&app, &session.device_token, &offer.pairing_uri).await;
    assert_eq!(paired.status(), StatusCode::OK);
    assert_eq!(response_json(paired).await["account"]["email"], user.email);
    assert_eq!(
        get_pairing_status(
            State(state.clone()),
            AuthUser {
                user_id: user.id,
                is_admin: false,
            },
        )
        .await
        .unwrap()
        .0
        .status,
        PairingStatusResponseKind::Connected
    );

    let replay = pair_request(&app, &session.device_token, &offer.pairing_uri).await;
    assert_eq!(replay.status(), StatusCode::OK);

    let bootstrapped = bootstrap_request(&app, &session.device_token).await;
    assert_eq!(bootstrapped.status(), StatusCode::OK);
    let bootstrapped = response_json(bootstrapped).await;
    assert_eq!(bootstrapped["can_send"], true);
    assert_eq!(bootstrapped["account"]["email"], user.email);

    let disconnected = disconnect_request(&app, &session.device_token).await;
    assert_eq!(disconnected.status(), StatusCode::NO_CONTENT);
    let after_disconnect = bootstrap_request(&app, &session.device_token).await;
    assert!(response_json(after_disconnect).await["account"].is_null());

    let consumed_after_disconnect =
        pair_request(&app, &session.device_token, &offer.pairing_uri).await;
    assert_eq!(consumed_after_disconnect.status(), StatusCode::BAD_REQUEST);
}

async fn pair_request(
    app: &Router,
    device_token: &str,
    pairing_uri: &str,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/light-tool/pair")
                .header("authorization", format!("Bearer {device_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "pairing_uri": pairing_uri }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn disconnect_request(app: &Router, device_token: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/light-tool/pair")
                .header("authorization", format!("Bearer {device_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn bootstrap_request(app: &Router, device_token: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/light-tool/bootstrap")
                .header("authorization", format!("Bearer {device_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "installation_id": INSTALLATION_ID }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}
