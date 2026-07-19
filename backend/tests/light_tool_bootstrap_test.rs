use backend::{
    repositories::light_tool_devices_repository::LightToolDevicesRepository,
    services::{
        light_tool_bootstrap::{
            LightToolBootstrapError, LightToolBootstrapService, TRIAL_DURATION_SECONDS,
            TRIAL_MESSAGE_LIMIT,
        },
        light_tool_identity::{hash_device_token, hash_installation_id},
        light_tool_trial::{LightToolTrialService, TrialMessageClaim},
    },
    test_utils::create_test_pg_pool,
};
use std::sync::{Arc, Barrier};

const INSTALLATION_ID: &str = "550e8400-e29b-41d4-a716-446655440000";
const OTHER_INSTALLATION_ID: &str = "550e8400-e29b-41d4-a716-446655440001";
const NOW: i32 = 1_700_000_000;

#[test]
#[serial_test::serial]
fn creates_and_resumes_the_same_anonymous_trial() {
    let pool = create_test_pg_pool();
    let service = LightToolBootstrapService::new(LightToolDevicesRepository::new(pool.clone()));
    let inspection_repository = LightToolDevicesRepository::new(pool);

    let created = service.bootstrap(INSTALLATION_ID, None, NOW).unwrap();

    assert!(created.can_send);
    assert_eq!(created.trial_messages_remaining, TRIAL_MESSAGE_LIMIT);
    assert_eq!(created.trial_expires_at, NOW + TRIAL_DURATION_SECONDS);

    let installation_hash = hash_installation_id(INSTALLATION_ID).unwrap();
    let stored = inspection_repository
        .find_by_installation_hash(&installation_hash)
        .unwrap()
        .unwrap();
    assert_eq!(
        stored.device_token_hash,
        hash_device_token(&created.device_token).unwrap()
    );

    let resumed = service
        .bootstrap(INSTALLATION_ID, Some(&created.device_token), NOW + 60)
        .unwrap();
    assert_eq!(resumed.device_token, created.device_token);
    assert_eq!(resumed.trial_expires_at, created.trial_expires_at);
    assert_eq!(resumed.trial_messages_remaining, TRIAL_MESSAGE_LIMIT);

    let refreshed = inspection_repository
        .find_by_installation_hash(&installation_hash)
        .unwrap()
        .unwrap();
    assert_eq!(refreshed.last_seen_at, NOW + 60);
}

#[test]
#[serial_test::serial]
fn existing_installations_require_their_device_token() {
    let service =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(create_test_pg_pool()));
    let created = service.bootstrap(INSTALLATION_ID, None, NOW).unwrap();

    assert!(matches!(
        service.bootstrap(INSTALLATION_ID, None, NOW + 1),
        Err(LightToolBootstrapError::DeviceTokenRequired)
    ));
    assert!(matches!(
        service.bootstrap(OTHER_INSTALLATION_ID, Some(&created.device_token), NOW + 1),
        Err(LightToolBootstrapError::InvalidDeviceCredentials)
    ));
}

#[test]
#[serial_test::serial]
fn expired_trials_can_bootstrap_but_cannot_send() {
    let service =
        LightToolBootstrapService::new(LightToolDevicesRepository::new(create_test_pg_pool()));
    let created = service.bootstrap(INSTALLATION_ID, None, NOW).unwrap();

    let expired = service
        .bootstrap(
            INSTALLATION_ID,
            Some(&created.device_token),
            NOW + TRIAL_DURATION_SECONDS,
        )
        .unwrap();

    assert!(!expired.can_send);
    assert_eq!(expired.trial_messages_remaining, TRIAL_MESSAGE_LIMIT);
}

#[test]
#[serial_test::serial]
fn concurrent_trial_claims_cannot_exceed_the_message_limit() {
    let pool = create_test_pg_pool();
    let bootstrap = LightToolBootstrapService::new(LightToolDevicesRepository::new(pool.clone()));
    bootstrap.bootstrap(INSTALLATION_ID, None, NOW).unwrap();

    let installation_hash = hash_installation_id(INSTALLATION_ID).unwrap();
    let inspection_repository = LightToolDevicesRepository::new(pool.clone());
    let device = inspection_repository
        .find_by_installation_hash(&installation_hash)
        .unwrap()
        .unwrap();
    let trial = Arc::new(LightToolTrialService::new(LightToolDevicesRepository::new(
        pool.clone(),
    )));
    let start = Arc::new(Barrier::new(2));

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let trial = trial.clone();
            let start = start.clone();
            std::thread::spawn(move || {
                start.wait();
                (0..TRIAL_MESSAGE_LIMIT)
                    .filter_map(|_| match trial.claim_message(device.id, NOW + 1).unwrap() {
                        TrialMessageClaim::Claimed { messages_remaining } => {
                            Some(messages_remaining)
                        }
                        TrialMessageClaim::Unavailable => None,
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();

    let mut remaining: Vec<i32> = handles
        .into_iter()
        .flat_map(|handle| handle.join().unwrap())
        .collect();
    remaining.sort_unstable();
    assert_eq!(remaining, (0..TRIAL_MESSAGE_LIMIT).collect::<Vec<_>>());

    let stored = inspection_repository
        .find_by_installation_hash(&installation_hash)
        .unwrap()
        .unwrap();
    assert_eq!(stored.trial_messages_used, TRIAL_MESSAGE_LIMIT);
    assert_eq!(
        trial.claim_message(device.id, NOW + 2).unwrap(),
        TrialMessageClaim::Unavailable
    );
}

#[test]
#[serial_test::serial]
fn expired_trials_cannot_claim_a_message() {
    let pool = create_test_pg_pool();
    let bootstrap = LightToolBootstrapService::new(LightToolDevicesRepository::new(pool.clone()));
    bootstrap.bootstrap(INSTALLATION_ID, None, NOW).unwrap();
    let installation_hash = hash_installation_id(INSTALLATION_ID).unwrap();
    let repository = LightToolDevicesRepository::new(pool.clone());
    let device = repository
        .find_by_installation_hash(&installation_hash)
        .unwrap()
        .unwrap();
    let trial = LightToolTrialService::new(LightToolDevicesRepository::new(pool));

    assert_eq!(
        trial
            .claim_message(device.id, NOW + TRIAL_DURATION_SECONDS)
            .unwrap(),
        TrialMessageClaim::Unavailable
    );
    let unchanged = repository
        .find_by_installation_hash(&installation_hash)
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.trial_messages_used, 0);
}
