use backend::{
    proactive::utils::send_notification,
    repositories::{
        light_tool_devices_repository::LightToolDevicesRepository,
        light_tool_notification_repository::LightToolNotificationRepository,
        light_tool_pairing_repository::LightToolPairingRepository,
        light_tool_push_outbox_repository::LightToolPushOutboxRepository,
        light_tool_push_repository::LightToolPushRepository,
        light_tool_runs_repository::LightToolRunsRepository,
    },
    services::{
        light_tool_bootstrap::LightToolBootstrapService, light_tool_identity::hash_installation_id,
        light_tool_pairing::LightToolPairingService,
    },
    test_utils::{create_test_state, create_test_user, TestUserParams},
    AppState, UserCoreOps,
};
use std::sync::Arc;

const INSTALLATION_ID: &str = "550e8400-e29b-41d4-a716-446655440013";
const NOW: i32 = 1_700_000_000;

#[test]
#[serial_test::serial]
fn notification_is_stored_and_push_is_queued_for_each_eligible_device() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let device_id = pair_push_device(&state, user.id);
    let repository = LightToolNotificationRepository::new(state.pg_pool.clone());

    assert_eq!(
        repository
            .enqueue_for_user(user.id, "Meeting moved to 3 PM", NOW)
            .unwrap(),
        vec![device_id]
    );

    let runs = LightToolRunsRepository::new(state.pg_pool.clone())
        .find_recent_for_principal(device_id, Some(user.id), 10)
        .unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].user_message, "");
    assert_eq!(
        runs[0].assistant_message.as_deref(),
        Some("Meeting moved to 3 PM")
    );
    assert_eq!(runs[0].status, "completed");
    assert!(runs[0].client_message_id.starts_with("notification:"));
    assert!(LightToolPushOutboxRepository::new(state.pg_pool.clone())
        .find_for_device(device_id)
        .unwrap()
        .is_some());

    assert!(LightToolRunsRepository::new(state.pg_pool.clone())
        .find_recent_completed_for_principal(device_id, Some(user.id), NOW, 10)
        .unwrap()
        .is_empty());
}

#[test]
#[serial_test::serial]
fn notification_is_not_stored_without_a_registered_push_endpoint() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
        .bootstrap(INSTALLATION_ID, None, NOW)
        .unwrap();
    let device = LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_by_installation_hash(&hash_installation_id(INSTALLATION_ID).unwrap())
        .unwrap()
        .unwrap();
    let pairing =
        LightToolPairingService::new(LightToolPairingRepository::new(state.pg_pool.clone()));
    let offer = pairing.create_offer(user.id, NOW).unwrap();
    pairing
        .consume_uri(&offer.pairing_uri, device.id, NOW + 1)
        .unwrap();

    assert!(LightToolNotificationRepository::new(state.pg_pool.clone())
        .enqueue_for_user(user.id, "No endpoint", NOW + 2)
        .unwrap()
        .is_empty());
    assert!(LightToolRunsRepository::new(state.pg_pool.clone())
        .find_recent_for_principal(device.id, Some(user.id), 10)
        .unwrap()
        .is_empty());
}

#[tokio::test]
#[serial_test::serial]
async fn selected_light_phone_route_queues_alert_instead_of_sms() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let device_id = pair_push_device(&state, user.id);
    state
        .user_core
        .update_notification_type(user.id, Some("light_phone"))
        .unwrap();

    assert!(
        send_notification(
            &state,
            user.id,
            "Time to leave for the train",
            "reminder".to_string(),
            None,
        )
        .await
    );

    let runs = LightToolRunsRepository::new(state.pg_pool.clone())
        .find_recent_for_principal(device_id, Some(user.id), 10)
        .unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(
        runs[0].assistant_message.as_deref(),
        Some("Time to leave for the train")
    );
    assert!(LightToolPushOutboxRepository::new(state.pg_pool.clone())
        .find_for_device(device_id)
        .unwrap()
        .is_some());
    assert!(state
        .user_repository
        .get_conversation_history(user.id)
        .unwrap()
        .iter()
        .any(|message| message.encrypted_content == "Time to leave for the train"));
}

fn pair_push_device(state: &Arc<AppState>, user_id: i32) -> i32 {
    LightToolBootstrapService::new(LightToolDevicesRepository::new(state.pg_pool.clone()))
        .bootstrap(INSTALLATION_ID, None, NOW)
        .unwrap();
    let device = LightToolDevicesRepository::new(state.pg_pool.clone())
        .find_by_installation_hash(&hash_installation_id(INSTALLATION_ID).unwrap())
        .unwrap()
        .unwrap();
    let pairing =
        LightToolPairingService::new(LightToolPairingRepository::new(state.pg_pool.clone()));
    let offer = pairing.create_offer(user_id, NOW).unwrap();
    pairing
        .consume_uri(&offer.pairing_uri, device.id, NOW + 1)
        .unwrap();
    LightToolPushRepository::new(state.pg_pool.clone())
        .upsert(
            device.id,
            "https://push.light.example/v1/device/test-token",
            NOW + 2,
        )
        .unwrap();
    device.id
}
