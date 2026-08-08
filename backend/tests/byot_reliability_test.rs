use std::sync::{Arc, Barrier};

use backend::api::twilio_client::mock::MockTwilioClient;
use backend::api::twilio_client::{
    IncomingPhoneNumberConfig, TwilioClientError, TwilioCredentials,
};
use backend::services::byot_setup::{
    configure_and_verify, safe_twilio_error, verify_live_configuration, ByotWebhookEndpoints,
};
use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use backend::{ByotRepository, UserCoreOps};
use serial_test::serial;

const PHONE: &str = "+14155551234";

fn endpoints() -> ByotWebhookEndpoints {
    ByotWebhookEndpoints {
        sms: "https://lightfriend.example/api/sms/server".to_string(),
        voice: "https://lightfriend.example/api/voice/incoming".to_string(),
    }
}

fn owned_number() -> IncomingPhoneNumberConfig {
    IncomingPhoneNumberConfig {
        sid: "PN_test".to_string(),
        phone_number: PHONE.to_string(),
        sms_capable: true,
        voice_capable: true,
        ..Default::default()
    }
}

fn credentials() -> TwilioCredentials {
    TwilioCredentials::new("AC_test".to_string(), "secret".to_string())
}

#[test]
fn provider_errors_are_classified_without_exposing_bodies_or_secrets() {
    let wrong_credentials = TwilioClientError::ApiError {
        status: 401,
        message: "raw body containing auth token secret-token".to_string(),
    };
    let safe = safe_twilio_error(&wrong_credentials);
    assert_eq!(safe.code, "credentials_rejected");
    assert!(!safe.user_message.contains("secret-token"));

    let unowned = safe_twilio_error(&TwilioClientError::NotFound(
        "provider account details".to_string(),
    ));
    assert_eq!(unowned.code, "number_not_owned");
    assert!(!unowned.user_message.contains("provider account details"));
}

#[tokio::test]
async fn requires_live_sms_and_voice_capabilities_before_writing() {
    for (sms_capable, voice_capable, expected_code) in [
        (false, true, "sms_not_supported"),
        (true, false, "voice_not_supported"),
    ] {
        let mut number = owned_number();
        number.sms_capable = sms_capable;
        number.voice_capable = voice_capable;
        let client = MockTwilioClient::new().with_phone_config(Ok(number));
        let error = configure_and_verify(&client, &credentials(), PHONE, &endpoints())
            .await
            .unwrap_err();
        assert_eq!(error.code, expected_code);
        assert_eq!(client.configure_webhook_call_count(), 0);
    }
}

#[tokio::test]
async fn configures_both_exact_callbacks_as_post_and_reads_back() {
    let client = MockTwilioClient::new().with_phone_config(Ok(owned_number()));
    let verified = configure_and_verify(&client, &credentials(), PHONE, &endpoints())
        .await
        .unwrap();
    assert_eq!(verified.sms_url, endpoints().sms);
    assert_eq!(verified.sms_method, "POST");
    assert_eq!(verified.voice_url, endpoints().voice);
    assert_eq!(verified.voice_method, "POST");

    let calls = client.get_calls();
    assert_eq!(calls.fetch_incoming_phone_number_calls.len(), 2);
    assert_eq!(calls.configure_webhook_calls.len(), 1);
    assert_eq!(calls.configure_webhook_calls[0].0, PHONE);
    assert_eq!(calls.configure_webhook_calls[0].1, endpoints().sms);
    assert_eq!(
        calls.configure_webhook_voice_urls[0],
        Some(endpoints().voice)
    );
}

#[tokio::test]
async fn partial_configuration_failure_stays_failed_and_a_retry_can_succeed() {
    let failed = MockTwilioClient::new()
        .with_phone_config(Ok(owned_number()))
        .with_configure_webhook_error("partial provider update".to_string());
    let error = configure_and_verify(&failed, &credentials(), PHONE, &endpoints())
        .await
        .unwrap_err();
    assert_eq!(error.code, "twilio_rejected");

    let retry = MockTwilioClient::new().with_phone_config(Ok(owned_number()));
    assert!(
        configure_and_verify(&retry, &credentials(), PHONE, &endpoints())
            .await
            .is_ok()
    );
}

#[test]
fn externally_overwritten_callback_or_method_is_detected_as_drift() {
    let mut live = owned_number();
    live.sms_url = endpoints().sms;
    live.sms_method = "GET".to_string();
    live.voice_url = "https://attacker.invalid/voice".to_string();
    live.voice_method = "POST".to_string();
    assert_eq!(
        verify_live_configuration(&live, PHONE, &endpoints())
            .unwrap_err()
            .code,
        "webhook_drift"
    );
}

#[test]
#[serial]
fn concurrent_setup_attempts_only_allow_the_latest_to_enable_routing() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let repository = Arc::new(ByotRepository::new(state.pg_pool.clone()));
    let first = repository.start_attempt(user.id, PHONE).unwrap();

    let barrier = Arc::new(Barrier::new(2));
    let repository_for_thread = Arc::clone(&repository);
    let barrier_for_thread = Arc::clone(&barrier);
    let user_id = user.id;
    let second_worker = std::thread::spawn(move || {
        barrier_for_thread.wait();
        repository_for_thread.start_attempt(user_id, PHONE).unwrap()
    });
    barrier.wait();
    let second = second_worker.join().unwrap();

    assert!(!repository
        .activate_if_current(user.id, &first, "PN_old")
        .unwrap());
    assert!(!state.user_core.is_byot_user(user.id));
    assert!(repository
        .activate_if_current(user.id, &second, "PN_current")
        .unwrap());
    assert!(state.user_core.is_byot_user(user.id));
    let persisted = repository.get(user.id).unwrap().unwrap();
    assert_eq!(persisted.phone_sid.as_deref(), Some("PN_current"));
    assert_eq!(persisted.status, "verified");
}

#[test]
#[serial]
fn concurrent_configuration_edit_invalidates_an_inflight_setup_attempt() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let repository = ByotRepository::new(state.pg_pool.clone());
    let attempt = repository.start_attempt(user.id, PHONE).unwrap();

    repository
        .update_phone_and_invalidate(user.id, "+14155550000")
        .unwrap();
    assert!(!repository
        .activate_if_current(user.id, &attempt, "PN_stale")
        .unwrap());
    assert!(!state.user_core.is_byot_user(user.id));
    let persisted = repository.get(user.id).unwrap().unwrap();
    assert_eq!(persisted.phone_number, "+14155550000");
    assert_eq!(persisted.status, "error");
    assert_eq!(
        persisted.error_code.as_deref(),
        Some("configuration_changed")
    );
}
