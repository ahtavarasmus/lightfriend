use backend::{
    repositories::{
        light_tool_devices_repository::LightToolDevicesRepository,
        light_tool_push_outbox_repository::LightToolPushOutboxRepository,
        light_tool_push_repository::LightToolPushRepository,
    },
    services::{
        light_tool_bootstrap::LightToolBootstrapService, light_tool_identity::hash_installation_id,
        light_tool_push_delivery::CONVERSATION_CHANGED_PAYLOAD,
        light_tool_push_outbox::LightToolPushOutboxWorker,
    },
    test_utils::create_test_state,
    AppState,
};
use std::sync::Arc;
use wiremock::{
    matchers::{body_bytes, method, path},
    Mock, MockServer, ResponseTemplate,
};

const INSTALLATION_ID: &str = "550e8400-e29b-41d4-a716-446655440012";
const NOW: i32 = 1_700_000_000;

#[tokio::test]
#[serial_test::serial]
async fn repeated_changes_coalesce_without_losing_a_new_in_flight_event() {
    let state = create_test_state();
    let device_id = bootstrap_device(&state);
    let repository = LightToolPushOutboxRepository::new(state.pg_pool.clone());

    let first = repository
        .enqueue_conversation_changed(device_id, NOW)
        .unwrap();
    let claimed = repository.claim_due(NOW, NOW + 120, 10).unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].device_id, first.device_id);
    assert_eq!(claimed[0].version, first.version);

    let second = repository
        .enqueue_conversation_changed(device_id, NOW + 1)
        .unwrap();
    assert_eq!(second.version, first.version + 1);
    assert!(!repository.complete(device_id, first.version).unwrap());
    assert_eq!(
        repository.find_for_device(device_id).unwrap().unwrap(),
        second
    );
}

#[tokio::test]
#[serial_test::serial]
async fn delivered_events_are_removed_from_the_outbox() {
    let state = create_test_state();
    let device_id = bootstrap_device(&state);
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/push/device-token"))
        .and(body_bytes(CONVERSATION_CHANGED_PAYLOAD))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    LightToolPushRepository::new(state.pg_pool.clone())
        .upsert(
            device_id,
            &format!("{}/push/device-token", server.uri()),
            NOW,
        )
        .unwrap();
    let outbox = LightToolPushOutboxRepository::new(state.pg_pool.clone());
    outbox.enqueue_conversation_changed(device_id, NOW).unwrap();

    let worker = LightToolPushOutboxWorker::new(state.pg_pool.clone(), ["127.0.0.1"]).unwrap();
    assert_eq!(worker.process_due_once(NOW).await.unwrap(), 1);
    assert!(outbox.find_for_device(device_id).unwrap().is_none());
}

#[tokio::test]
#[serial_test::serial]
async fn transient_failures_are_released_for_a_later_retry() {
    let state = create_test_state();
    let device_id = bootstrap_device(&state);
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/push/unavailable"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    LightToolPushRepository::new(state.pg_pool.clone())
        .upsert(
            device_id,
            &format!("{}/push/unavailable", server.uri()),
            NOW,
        )
        .unwrap();
    let outbox = LightToolPushOutboxRepository::new(state.pg_pool.clone());
    outbox.enqueue_conversation_changed(device_id, NOW).unwrap();

    let worker = LightToolPushOutboxWorker::new(state.pg_pool.clone(), ["127.0.0.1"]).unwrap();
    assert_eq!(worker.process_due_once(NOW).await.unwrap(), 1);

    let pending = outbox.find_for_device(device_id).unwrap().unwrap();
    assert_eq!(pending.attempt_count, 1);
    assert_eq!(pending.lease_until, 0);
    assert!(pending.next_attempt_at > NOW);
    assert_eq!(worker.process_due_once(NOW).await.unwrap(), 0);
}

#[tokio::test]
#[serial_test::serial]
async fn events_without_registered_endpoints_are_completed() {
    let state = create_test_state();
    let device_id = bootstrap_device(&state);
    let outbox = LightToolPushOutboxRepository::new(state.pg_pool.clone());
    outbox.enqueue_conversation_changed(device_id, NOW).unwrap();

    let worker = LightToolPushOutboxWorker::new(state.pg_pool.clone(), ["127.0.0.1"]).unwrap();
    assert_eq!(worker.process_due_once(NOW).await.unwrap(), 1);
    assert!(outbox.find_for_device(device_id).unwrap().is_none());
}

fn bootstrap_device(state: &Arc<AppState>) -> i32 {
    LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
        .bootstrap(INSTALLATION_ID, None, NOW)
        .unwrap();
    LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_by_installation_hash(&hash_installation_id(INSTALLATION_ID).unwrap())
        .unwrap()
        .unwrap()
        .id
}
