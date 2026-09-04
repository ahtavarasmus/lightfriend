use backend::{
    repositories::{
        light_tool_devices_repository::LightToolDevicesRepository,
        light_tool_push_repository::LightToolPushRepository,
    },
    services::{
        light_tool_bootstrap::LightToolBootstrapService,
        light_tool_identity::hash_installation_id,
        light_tool_push_delivery::{
            LightToolPushDeliveryError, LightToolPushDeliveryOutcome, LightToolPushDeliveryService,
            CONVERSATION_CHANGED_PAYLOAD,
        },
    },
    test_utils::create_test_state,
    AppState,
};
use std::sync::Arc;
use wiremock::{
    matchers::{body_bytes, header, method, path},
    Mock, MockServer, ResponseTemplate,
};

const INSTALLATION_ID: &str = "550e8400-e29b-41d4-a716-446655440011";

#[tokio::test]
#[serial_test::serial]
async fn conversation_change_push_contains_no_conversation_data() {
    let state = create_test_state();
    let device_id = bootstrap_device(&state);
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/push/device-token"))
        .and(header("content-type", "application/json"))
        .and(body_bytes(CONVERSATION_CHANGED_PAYLOAD))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    LightToolPushRepository::new(state.pg_pool.clone())
        .upsert(
            device_id,
            &format!("{}/push/device-token", server.uri()),
            1_700_000_001,
        )
        .unwrap();

    let delivery = LightToolPushDeliveryService::new(state.pg_pool.clone(), ["127.0.0.1"])
        .expect("push delivery service");
    let outcome = delivery
        .send_conversation_changed(device_id)
        .await
        .expect("push delivery");

    assert_eq!(outcome, LightToolPushDeliveryOutcome::Delivered);
}

#[tokio::test]
#[serial_test::serial]
async fn expired_endpoint_is_removed_without_touching_other_devices() {
    let state = create_test_state();
    let device_id = bootstrap_device(&state);
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/push/expired"))
        .respond_with(ResponseTemplate::new(410))
        .expect(1)
        .mount(&server)
        .await;
    let repository = LightToolPushRepository::new(state.pg_pool.clone());
    repository
        .upsert(
            device_id,
            &format!("{}/push/expired", server.uri()),
            1_700_000_001,
        )
        .unwrap();

    let delivery = LightToolPushDeliveryService::new(state.pg_pool.clone(), ["127.0.0.1"])
        .expect("push delivery service");
    let outcome = delivery
        .send_conversation_changed(device_id)
        .await
        .expect("expired endpoint handling");

    assert_eq!(outcome, LightToolPushDeliveryOutcome::EndpointExpired);
    assert!(repository.find_for_device(device_id).unwrap().is_none());
}

#[tokio::test]
#[serial_test::serial]
async fn delivery_refuses_hosts_outside_the_allowlist() {
    let state = create_test_state();
    let device_id = bootstrap_device(&state);
    let server = MockServer::start().await;
    LightToolPushRepository::new(state.pg_pool.clone())
        .upsert(
            device_id,
            &format!("{}/push/device-token", server.uri()),
            1_700_000_001,
        )
        .unwrap();

    let delivery = LightToolPushDeliveryService::new(state.pg_pool.clone(), ["push.light.test"])
        .expect("push delivery service");
    let error = delivery
        .send_conversation_changed(device_id)
        .await
        .expect_err("disallowed endpoint should fail");

    assert!(matches!(error, LightToolPushDeliveryError::HostNotAllowed));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
#[serial_test::serial]
async fn delivery_uses_the_configured_https_proxy() {
    let state = create_test_state();
    let device_id = bootstrap_device(&state);
    let proxy = MockServer::start().await;
    let endpoint = "https://push.light.test/push/device-token";
    LightToolPushRepository::new(state.pg_pool.clone())
        .upsert(device_id, endpoint, 1_700_000_001)
        .unwrap();

    let old_https_proxy = std::env::var_os("HTTPS_PROXY");
    let old_https_proxy_lower = std::env::var_os("https_proxy");
    let old_no_proxy = std::env::var_os("NO_PROXY");
    let old_no_proxy_lower = std::env::var_os("no_proxy");
    std::env::set_var("HTTPS_PROXY", proxy.uri());
    std::env::remove_var("https_proxy");
    std::env::remove_var("NO_PROXY");
    std::env::remove_var("no_proxy");

    let delivery = LightToolPushDeliveryService::new(state.pg_pool.clone(), ["push.light.test"])
        .expect("push delivery service");
    let result = delivery.send_conversation_changed(device_id).await;

    restore_env("HTTPS_PROXY", old_https_proxy);
    restore_env("https_proxy", old_https_proxy_lower);
    restore_env("NO_PROXY", old_no_proxy);
    restore_env("no_proxy", old_no_proxy_lower);

    assert!(
        result.is_err(),
        "the mock proxy does not establish TLS tunnels"
    );
    let requests = proxy.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method.as_str(), "CONNECT");
}

fn restore_env(name: &str, value: Option<std::ffi::OsString>) {
    match value {
        Some(value) => std::env::set_var(name, value),
        None => std::env::remove_var(name),
    }
}

fn bootstrap_device(state: &Arc<AppState>) -> i32 {
    LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
        .bootstrap(INSTALLATION_ID, None, 1_700_000_000)
        .unwrap();
    LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_by_installation_hash(&hash_installation_id(INSTALLATION_ID).unwrap())
        .unwrap()
        .unwrap()
        .id
}
