//! Unit tests for TwilioStatusService
//!
//! Tests the business logic for processing Twilio status callbacks,
//! including status updates, price fetching, and message deletion.

use std::sync::Arc;

use backend::api::twilio_client::mock::MockTwilioClient;
use backend::api::twilio_client::{MessagePrice, TwilioCredentials};
use backend::repositories::mock_twilio_status_repository::{
    MockMessage, MockTwilioStatusRepository,
};
use backend::services::twilio_status_service::{
    StatusCallbackInput, TwilioStatusError, TwilioStatusService, TwilioStatusServiceConfig,
};

fn create_test_message(message_sid: &str, status: &str) -> MockMessage {
    MockMessage {
        message_sid: message_sid.to_string(),
        user_id: 1,
        to_number: "+14155551234".to_string(),
        from_number: Some("+14155559999".to_string()),
        status: status.to_string(),
        error_code: None,
        error_message: None,
        price: None,
        price_unit: None,
    }
}

fn create_test_service(
    repo: Arc<MockTwilioStatusRepository>,
    client: Arc<MockTwilioClient>,
) -> TwilioStatusService<MockTwilioStatusRepository, MockTwilioClient> {
    // Use config with no delays for faster tests
    let config = TwilioStatusServiceConfig {
        send_failure_notifications: false, // Disable email in tests
        fetch_price_on_final: true,
        delete_on_final: true,
        price_fetch_delays: vec![0], // No delay for tests
    };
    // Use dummy credentials for tests
    let credentials = TwilioCredentials::new("test_sid".to_string(), "test_token".to_string());
    TwilioStatusService::with_config(repo, client, credentials, config)
}

#[tokio::test]
async fn test_status_update_delivered() {
    let repo = Arc::new(MockTwilioStatusRepository::new());
    let client = Arc::new(MockTwilioClient::new());

    repo.add_message(create_test_message("SM123", "sent"));

    let service = create_test_service(repo.clone(), client.clone());

    let input = StatusCallbackInput {
        message_sid: "SM123".to_string(),
        message_status: "delivered".to_string(),
        error_code: None,
        error_message: None,
        price: Some(0.0075),
        price_unit: Some("USD".to_string()),
    };

    let result = service.process_status_callback(input).await.unwrap();

    assert_eq!(result.rows_updated, 1);
    assert!(result.is_final_status);

    let msg = repo.get_message("SM123").unwrap();
    assert_eq!(msg.status, "delivered");
    assert_eq!(msg.price, Some(0.0075));
}

#[tokio::test]
async fn test_status_update_nonexistent_message() {
    let repo = Arc::new(MockTwilioStatusRepository::new());
    let client = Arc::new(MockTwilioClient::new());

    let service = create_test_service(repo.clone(), client.clone());

    let input = StatusCallbackInput {
        message_sid: "SM_NONEXISTENT".to_string(),
        message_status: "delivered".to_string(),
        error_code: None,
        error_message: None,
        price: None,
        price_unit: None,
    };

    let result = service.process_status_callback(input).await.unwrap();

    assert_eq!(result.rows_updated, 0);
}

#[tokio::test]
async fn test_failure_status_fields() {
    let repo = Arc::new(MockTwilioStatusRepository::new());
    let client = Arc::new(MockTwilioClient::new());

    repo.add_message(create_test_message("SM123", "sent"));

    let service = create_test_service(repo.clone(), client.clone());

    let input = StatusCallbackInput {
        message_sid: "SM123".to_string(),
        message_status: "failed".to_string(),
        error_code: Some("30006".to_string()),
        error_message: Some("Landline or unreachable carrier".to_string()),
        price: None,
        price_unit: None,
    };

    let result = service.process_status_callback(input).await.unwrap();

    assert_eq!(result.rows_updated, 1);
    assert!(result.is_final_status);

    let msg = repo.get_message("SM123").unwrap();
    assert_eq!(msg.status, "failed");
    assert_eq!(msg.error_code, Some("30006".to_string()));
    assert_eq!(
        msg.error_message,
        Some("Landline or unreachable carrier".to_string())
    );
}

#[tokio::test]
async fn test_intermediate_status_not_final() {
    let repo = Arc::new(MockTwilioStatusRepository::new());
    let client = Arc::new(MockTwilioClient::new());

    repo.add_message(create_test_message("SM123", "queued"));

    let service = create_test_service(repo.clone(), client.clone());

    let input = StatusCallbackInput {
        message_sid: "SM123".to_string(),
        message_status: "sending".to_string(),
        error_code: None,
        error_message: None,
        price: None,
        price_unit: None,
    };

    let result = service.process_status_callback(input).await.unwrap();

    assert_eq!(result.rows_updated, 1);
    assert!(!result.is_final_status);

    let msg = repo.get_message("SM123").unwrap();
    assert_eq!(msg.status, "sending");
}

#[tokio::test]
async fn test_final_status_fetches_price() {
    let repo = Arc::new(MockTwilioStatusRepository::new());
    let client = Arc::new(MockTwilioClient::new());

    repo.add_message(create_test_message("SM123", "sent"));
    client.set_price_response(
        "SM123",
        Some(MessagePrice {
            price: Some("0.0075".to_string()),
            price_unit: Some("USD".to_string()),
        }),
    );

    let service = create_test_service(repo.clone(), client.clone());

    let input = StatusCallbackInput {
        message_sid: "SM123".to_string(),
        message_status: "delivered".to_string(),
        error_code: None,
        error_message: None,
        price: None,
        price_unit: None,
    };

    service.process_status_callback(input).await.unwrap();

    // Wait for the spawned task to complete
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Verify price was fetched
    assert!(client.fetch_price_call_count() >= 1);

    // Verify price was updated in repo
    let msg = repo.get_message("SM123").unwrap();
    assert_eq!(msg.price, Some(0.0075));
    assert_eq!(msg.price_unit, Some("USD".to_string()));
}

#[tokio::test]
async fn test_final_status_deletes_message() {
    let repo = Arc::new(MockTwilioStatusRepository::new());
    let client = Arc::new(MockTwilioClient::new());

    repo.add_message(create_test_message("SM123", "sent"));

    let service = create_test_service(repo.clone(), client.clone());

    let input = StatusCallbackInput {
        message_sid: "SM123".to_string(),
        message_status: "delivered".to_string(),
        error_code: None,
        error_message: None,
        price: None,
        price_unit: None,
    };

    service.process_status_callback(input).await.unwrap();

    // Wait for the spawned task to complete
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Verify message was deleted
    assert!(client.was_deleted("SM123"));
}

#[tokio::test]
async fn test_database_error_propagated() {
    let repo = Arc::new(MockTwilioStatusRepository::new());
    let client = Arc::new(MockTwilioClient::new());

    repo.set_fail_on_update(true);

    let service = create_test_service(repo.clone(), client.clone());

    let input = StatusCallbackInput {
        message_sid: "SM123".to_string(),
        message_status: "delivered".to_string(),
        error_code: None,
        error_message: None,
        price: None,
        price_unit: None,
    };

    let result = service.process_status_callback(input).await;

    assert!(matches!(result, Err(TwilioStatusError::Repository(_))));
}

#[tokio::test]
async fn test_undelivered_is_final_status() {
    let repo = Arc::new(MockTwilioStatusRepository::new());
    let client = Arc::new(MockTwilioClient::new());

    repo.add_message(create_test_message("SM123", "sent"));

    let service = create_test_service(repo.clone(), client.clone());

    let input = StatusCallbackInput {
        message_sid: "SM123".to_string(),
        message_status: "undelivered".to_string(),
        error_code: Some("30003".to_string()),
        error_message: Some("Unreachable destination handset".to_string()),
        price: None,
        price_unit: None,
    };

    let result = service.process_status_callback(input).await.unwrap();

    assert!(result.is_final_status);
}

#[tokio::test]
async fn test_sent_is_not_final_status() {
    let repo = Arc::new(MockTwilioStatusRepository::new());
    let client = Arc::new(MockTwilioClient::new());

    repo.add_message(create_test_message("SM123", "sending"));

    let service = create_test_service(repo.clone(), client.clone());

    let input = StatusCallbackInput {
        message_sid: "SM123".to_string(),
        message_status: "sent".to_string(),
        error_code: None,
        error_message: None,
        price: None,
        price_unit: None,
    };

    let result = service.process_status_callback(input).await.unwrap();

    assert!(!result.is_final_status);
}
