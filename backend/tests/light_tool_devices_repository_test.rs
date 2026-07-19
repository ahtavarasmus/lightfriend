use backend::{
    models::light_tool_models::NewLightToolDevice,
    repositories::light_tool_devices_repository::LightToolDevicesRepository,
    test_utils::create_test_pg_pool,
};

#[test]
fn creates_and_finds_anonymous_trial_device() {
    let repository = LightToolDevicesRepository::new(create_test_pg_pool());
    let new_device = NewLightToolDevice {
        installation_id_hash: "installation-hash".to_string(),
        device_token_hash: "token-hash".to_string(),
        trial_started_at: 1_700_000_000,
        trial_expires_at: 1_700_259_200,
        last_seen_at: 1_700_000_000,
        created_at: 1_700_000_000,
        updated_at: 1_700_000_000,
    };

    let created = repository.create(&new_device).unwrap();

    assert_eq!(created.installation_id_hash, "installation-hash");
    assert_eq!(created.device_token_hash, "token-hash");
    assert_eq!(created.user_id, None);
    assert_eq!(created.trial_messages_used, 0);
    assert_eq!(created.revoked_at, None);

    let by_installation = repository
        .find_by_installation_hash("installation-hash")
        .unwrap()
        .unwrap();
    assert_eq!(by_installation.id, created.id);

    let by_token = repository
        .find_active_by_token_hash("token-hash")
        .unwrap()
        .unwrap();
    assert_eq!(by_token.id, created.id);

    assert!(repository
        .find_active_by_token_hash("unknown-token-hash")
        .unwrap()
        .is_none());
}
