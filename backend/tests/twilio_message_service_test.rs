//! Integration tests for Twilio message service.
//!
//! Tests credential resolution, sending strategies, and message sending flows.

use backend::api::twilio_client::mock::MockTwilioClient;
use backend::services::twilio_message_service::{SendConfig, TwilioMessageService};
use backend::test_utils::{
    create_test_state, create_test_user, set_byot_credentials, set_preferred_number,
    setup_test_encryption, TestUserParams,
};
use backend::UserCoreOps;
use serial_test::serial;
use std::sync::Arc;

/// Set up environment variables for testing
fn setup_test_env() {
    // Global Twilio credentials
    std::env::set_var("TWILIO_ACCOUNT_SID", "AC_global_test_sid");
    std::env::set_var("TWILIO_AUTH_TOKEN", "global_test_token");
    std::env::set_var("TWILIO_MESSAGING_SERVICE_SID", "MG_test_messaging_service");

    // Country-specific phone numbers
    std::env::set_var("USA_PHONE", "+18005551234");
    std::env::set_var("CAN_PHONE", "+16135551234");
    std::env::set_var("FIN_PHONE", "+358401001234");
    std::env::set_var("NL_PHONE", "+31201231234");
    std::env::set_var("GB_PHONE", "+442071234567");
    std::env::set_var("AUS_PHONE", "+61291234567");

    // Server URL for callbacks
    std::env::set_var("SERVER_URL", "https://test.example.com");

    // Set to production to test actual sending logic (not "development" which skips)
    std::env::set_var("ENVIRONMENT", "test");
}

/// Create a TwilioMessageService with MockTwilioClient for testing
fn create_test_service(
    state: &Arc<backend::AppState>,
) -> (
    TwilioMessageService<MockTwilioClient>,
    Arc<MockTwilioClient>,
) {
    let mock_client = Arc::new(MockTwilioClient::new());
    let service = TwilioMessageService::new(
        mock_client.clone(),
        state.db_pool.clone(),
        state.user_core.clone(),
        state.user_repository.clone(),
    );
    (service, mock_client)
}

// =========================================================================
// Basic Configuration Tests
// =========================================================================

#[test]
fn test_send_config_creation() {
    let config = SendConfig {
        to: "+1234567890".to_string(),
        body: "Hello, world!".to_string(),
        media_url: None,
    };
    assert_eq!(config.to, "+1234567890");
    assert_eq!(config.body, "Hello, world!");
}

// =========================================================================
// Credential Resolution Tests
// =========================================================================

#[test]
fn test_resolve_credentials_byot_user_uses_own_credentials() {
    setup_test_env();
    setup_test_encryption();
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    // Set BYOT credentials (sets both credentials and plan_type)
    set_byot_credentials(&state, user.id);

    let (service, _mock) = create_test_service(&state);
    let user = state.user_core.find_by_id(user.id).unwrap().unwrap();

    let result = service.resolve_credentials(&user);
    assert!(result.is_ok(), "Should resolve BYOT credentials");

    let creds = result.unwrap();
    // BYOT user should use their own credentials, not global
    assert_eq!(creds.account_sid, "AC_test_sid");
    assert_eq!(creds.auth_token, "test_auth_token");
}

#[test]
fn test_resolve_credentials_local_number_country_uses_global() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, _mock) = create_test_service(&state);

    let result = service.resolve_credentials(&user);
    assert!(
        result.is_ok(),
        "Should resolve global credentials for US user"
    );

    let creds = result.unwrap();
    assert_eq!(creds.account_sid, "AC_global_test_sid");
    assert_eq!(creds.auth_token, "global_test_token");
}

#[test]
fn test_resolve_credentials_notification_only_uses_global() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::germany_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, _mock) = create_test_service(&state);

    let result = service.resolve_credentials(&user);
    assert!(
        result.is_ok(),
        "Should resolve global credentials for Germany user"
    );

    let creds = result.unwrap();
    assert_eq!(creds.account_sid, "AC_global_test_sid");
    assert_eq!(creds.auth_token, "global_test_token");
}

#[test]
fn test_resolve_credentials_finland_uses_global() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::finland_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, _mock) = create_test_service(&state);

    let result = service.resolve_credentials(&user);
    assert!(result.is_ok());

    let creds = result.unwrap();
    assert_eq!(creds.account_sid, "AC_global_test_sid");
}

#[test]
fn test_resolve_credentials_canada_uses_global() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::canada_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, _mock) = create_test_service(&state);

    let result = service.resolve_credentials(&user);
    assert!(result.is_ok());

    let creds = result.unwrap();
    assert_eq!(creds.account_sid, "AC_global_test_sid");
}

// =========================================================================
// Sending Strategy Tests - Local Number Countries
// =========================================================================

#[test]
fn test_strategy_us_user_uses_messaging_service() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, _mock) = create_test_service(&state);

    let (from_number, use_messaging_service, update_preferred) =
        service.determine_sending_strategy(&user).unwrap();

    // US users should use messaging service, not a specific From number
    assert!(
        use_messaging_service,
        "US users should use messaging service"
    );
    assert!(
        from_number.is_none(),
        "US users should not have a From number"
    );
    assert!(
        !update_preferred,
        "US users should not update preferred_number"
    );
}

#[test]
fn test_strategy_ca_user_uses_preferred_number() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::canada_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    // Set a preferred number for the user
    set_preferred_number(&state, user.id, "+16135559999");
    let user = state.user_core.find_by_id(user.id).unwrap().unwrap();

    let (service, _mock) = create_test_service(&state);

    let (from_number, use_messaging_service, update_preferred) =
        service.determine_sending_strategy(&user).unwrap();

    assert!(
        !use_messaging_service,
        "CA users should not use messaging service"
    );
    assert_eq!(
        from_number,
        Some("+16135559999".to_string()),
        "CA users should use their preferred number"
    );
    assert!(
        !update_preferred,
        "Should not update if preferred already set"
    );
}

#[test]
fn test_strategy_ca_user_no_preferred_uses_can_phone_env() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::canada_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, _mock) = create_test_service(&state);

    let (from_number, use_messaging_service, update_preferred) =
        service.determine_sending_strategy(&user).unwrap();

    assert!(
        !use_messaging_service,
        "CA users should not use messaging service"
    );
    assert_eq!(
        from_number,
        Some("+16135551234".to_string()),
        "CA users without preferred should use CAN_PHONE"
    );
    assert!(update_preferred, "Should update preferred_number");
}

// =========================================================================
// Sending Strategy Tests - Notification-Only Countries
// =========================================================================

#[test]
fn test_strategy_noti_country_default_uses_us_messaging_service() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::germany_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, _mock) = create_test_service(&state);

    let (from_number, use_messaging_service, update_preferred) =
        service.determine_sending_strategy(&user).unwrap();

    // Notification-only without preferred_number should use US messaging service
    assert!(
        use_messaging_service,
        "Notification-only without preferred should use messaging service"
    );
    assert!(from_number.is_none());
    assert!(!update_preferred);
}

// =========================================================================
// Send SMS Integration Tests (with MockTwilioClient)
// =========================================================================

#[tokio::test]
#[serial]
async fn test_send_sms_us_user_full_flow() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, mock_client) = create_test_service(&state);

    let result = service.send_sms("Hello from test", None, &user).await;
    assert!(result.is_ok(), "send_sms should succeed for US user");

    // Verify the mock was called with correct parameters
    let calls = mock_client.get_calls();
    assert_eq!(calls.send_message_calls.len(), 1);

    let call = &calls.send_message_calls[0];
    assert_eq!(call.to, user.phone_number);
    assert_eq!(call.body, "Hello from test");
    // US users should use messaging service
    assert!(
        call.messaging_service_sid.is_some(),
        "US should use messaging service"
    );
    assert!(call.from.is_none(), "US should not have From number");
}

#[tokio::test]
#[serial]
async fn test_send_sms_ca_user_full_flow() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::canada_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, mock_client) = create_test_service(&state);

    let result = service.send_sms("Hello Canada", None, &user).await;
    assert!(result.is_ok());

    let calls = mock_client.get_calls();
    assert_eq!(calls.send_message_calls.len(), 1);

    let call = &calls.send_message_calls[0];
    // CA users should use From number (CAN_PHONE), not messaging service
    assert!(
        call.messaging_service_sid.is_none(),
        "CA should not use messaging service"
    );
    assert_eq!(
        call.from,
        Some("+16135551234".to_string()),
        "CA should use CAN_PHONE"
    );
}

#[tokio::test]
#[serial]
async fn test_send_sms_fi_user_full_flow() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::finland_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, mock_client) = create_test_service(&state);

    let result = service.send_sms("Moi!", None, &user).await;
    assert!(result.is_ok());

    let calls = mock_client.get_calls();
    let call = &calls.send_message_calls[0];

    assert!(call.messaging_service_sid.is_none());
    assert_eq!(call.from, Some("+358401001234".to_string()));
}

#[tokio::test]
#[serial]
async fn test_send_sms_notification_only_default_flow() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::germany_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, mock_client) = create_test_service(&state);

    let result = service.send_sms("Hallo!", None, &user).await;
    assert!(result.is_ok());

    let calls = mock_client.get_calls();
    let call = &calls.send_message_calls[0];

    // Germany (notification-only) without preferred should use US messaging service
    assert!(
        call.messaging_service_sid.is_some(),
        "Notification-only should use messaging service"
    );
    assert!(call.from.is_none());
}

#[tokio::test]
#[serial]
async fn test_send_sms_skips_in_development() {
    // Save original environment value
    let original_env = std::env::var("ENVIRONMENT").ok();

    // Set to development environment
    std::env::set_var("ENVIRONMENT", "development");

    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, mock_client) = create_test_service(&state);

    let result = service.send_sms("Dev test", None, &user).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "dev_not_sending");

    // Mock should NOT have been called
    let calls = mock_client.get_calls();
    assert_eq!(
        calls.send_message_calls.len(),
        0,
        "Should not call Twilio in development"
    );

    // Restore original environment
    match original_env {
        Some(val) => std::env::set_var("ENVIRONMENT", val),
        None => std::env::remove_var("ENVIRONMENT"),
    }
}

// =========================================================================
// Delete Message Tests
// =========================================================================

#[tokio::test]
#[serial]
async fn test_delete_message_media_success() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, mock_client) = create_test_service(&state);

    let result = service.delete_message_media(&user, "SM123", "ME456").await;
    assert!(result.is_ok());

    let calls = mock_client.get_calls();
    assert_eq!(calls.delete_media_calls.len(), 1);
    assert_eq!(
        calls.delete_media_calls[0],
        ("SM123".to_string(), "ME456".to_string())
    );
}

// =========================================================================
// Fetch Message Price Tests
// =========================================================================

#[tokio::test]
#[serial]
async fn test_fetch_message_price_success() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let mock_client = Arc::new(MockTwilioClient::new());
    mock_client.set_price_response(
        "SM123",
        Some(backend::api::twilio_client::MessagePrice {
            price: Some("-0.0075".to_string()),
            price_unit: Some("USD".to_string()),
        }),
    );

    let service = TwilioMessageService::new(
        mock_client.clone(),
        state.db_pool.clone(),
        state.user_core.clone(),
        state.user_repository.clone(),
    );

    let result = service.fetch_message_price(&user, "SM123").await;
    assert!(result.is_ok());

    let price = result.unwrap();
    assert!(price.is_some());
    let price = price.unwrap();
    assert_eq!(price.price, Some("-0.0075".to_string()));
    assert_eq!(price.price_unit, Some("USD".to_string()));
}

// =========================================================================
// Status Callback URL Tests
// =========================================================================

#[tokio::test]
#[serial]
async fn test_send_sms_includes_status_callback() {
    setup_test_env();
    let state = create_test_state();
    let params = TestUserParams::us_user(10.0, 5.0);
    let user = create_test_user(&state, &params);

    let (service, mock_client) = create_test_service(&state);

    let result = service.send_sms("Test with callback", None, &user).await;
    assert!(result.is_ok());

    let calls = mock_client.get_calls();
    let call = &calls.send_message_calls[0];

    assert!(
        call.status_callback_url.is_some(),
        "Status callback URL should be set"
    );
    assert!(
        call.status_callback_url
            .as_ref()
            .unwrap()
            .contains("/api/twilio/status-callback"),
        "Callback URL should point to status endpoint"
    );
}
