//! Twilio status callback service containing business logic.
//!
//! Separates business logic from webhook handling, enabling unit testing
//! with mock repositories and clients.

use std::sync::Arc;

use thiserror::Error;

use crate::repositories::twilio_client::{TwilioClient, TwilioClientError};
use crate::repositories::twilio_status_repository::{
    StatusUpdate, TwilioStatusRepository, TwilioStatusRepositoryError,
};
use crate::utils::country::get_country_code_from_phone;
use crate::utils::email::send_sms_failure_admin_email;

/// Errors that can occur during status callback processing
#[derive(Debug, Error)]
pub enum TwilioStatusError {
    #[error("Repository error: {0}")]
    Repository(#[from] TwilioStatusRepositoryError),

    #[error("Client error: {0}")]
    Client(#[from] TwilioClientError),

    #[error("Parse error: {0}")]
    Parse(String),
}

/// Input data for status callback processing
#[derive(Debug, Clone)]
pub struct StatusCallbackInput {
    pub message_sid: String,
    pub message_status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub price: Option<f32>,
    pub price_unit: Option<String>,
}

/// Result of processing a status callback
#[derive(Debug)]
pub struct StatusCallbackResult {
    /// Number of database rows updated
    pub rows_updated: usize,
    /// Whether admin notification was triggered
    pub notification_triggered: bool,
    /// Whether this was a final status
    pub is_final_status: bool,
}

/// Configuration for the service
#[derive(Debug, Clone)]
pub struct TwilioStatusServiceConfig {
    /// Whether to send admin email notifications on failure
    pub send_failure_notifications: bool,
    /// Whether to fetch price on final status
    pub fetch_price_on_final: bool,
    /// Whether to delete messages on final status
    pub delete_on_final: bool,
    /// Delays in seconds for price fetch retries
    pub price_fetch_delays: Vec<u64>,
}

impl Default for TwilioStatusServiceConfig {
    fn default() -> Self {
        Self {
            send_failure_notifications: true,
            fetch_price_on_final: true,
            delete_on_final: true,
            price_fetch_delays: vec![3, 5, 7],
        }
    }
}

/// Service for handling Twilio status callbacks.
///
/// Extracts the business logic from the webhook handler,
/// making it testable with mock repositories and clients.
pub struct TwilioStatusService<R: TwilioStatusRepository + 'static, C: TwilioClient + 'static> {
    repository: Arc<R>,
    client: Arc<C>,
    config: TwilioStatusServiceConfig,
}

impl<R: TwilioStatusRepository + 'static, C: TwilioClient + 'static> TwilioStatusService<R, C> {
    /// Create a new service with the given repository and client.
    pub fn new(repository: Arc<R>, client: Arc<C>) -> Self {
        Self {
            repository,
            client,
            config: TwilioStatusServiceConfig::default(),
        }
    }

    /// Create a new service with custom configuration.
    pub fn with_config(
        repository: Arc<R>,
        client: Arc<C>,
        config: TwilioStatusServiceConfig,
    ) -> Self {
        Self {
            repository,
            client,
            config,
        }
    }

    /// Process a status callback from Twilio.
    ///
    /// This handles:
    /// 1. Updating the message status in the database
    /// 2. Sending admin notifications on failure
    /// 3. Fetching price on final status
    /// 4. Deleting the message from Twilio on final status
    pub async fn process_status_callback(
        &self,
        input: StatusCallbackInput,
    ) -> Result<StatusCallbackResult, TwilioStatusError> {
        tracing::info!(
            "Processing status callback: sid={}, status={}, error_code={:?}",
            input.message_sid,
            input.message_status,
            input.error_code
        );

        // Update message status in database
        let update = StatusUpdate {
            status: input.message_status.clone(),
            error_code: input.error_code.clone(),
            error_message: input.error_message.clone(),
            price: input.price,
            price_unit: input.price_unit.clone(),
        };

        let rows_updated = self
            .repository
            .update_message_status(&input.message_sid, &update)?;

        if rows_updated == 0 {
            tracing::warn!(
                "No message_status_log record found for SID {}, status update skipped",
                input.message_sid
            );
        } else {
            tracing::info!(
                "Updated message_status_log for SID {} to status {}",
                input.message_sid,
                input.message_status
            );
        }

        // Send admin notification on failure
        let notification_triggered = self.handle_failure_notification(&input).await;

        // Check if this is a final status
        let is_final_status = matches!(
            input.message_status.as_str(),
            "delivered" | "failed" | "undelivered"
        );

        // Handle final status actions (price fetch and deletion)
        if is_final_status {
            self.handle_final_status(&input.message_sid).await;
        }

        Ok(StatusCallbackResult {
            rows_updated,
            notification_triggered,
            is_final_status,
        })
    }

    /// Handle sending failure notification if message delivery failed.
    async fn handle_failure_notification(&self, input: &StatusCallbackInput) -> bool {
        if !self.config.send_failure_notifications {
            return false;
        }

        let is_failure = input.message_status == "failed" || input.message_status == "undelivered";
        if !is_failure {
            return false;
        }

        // Get user info for the message
        let user_info = match self.repository.get_message_user_info(&input.message_sid) {
            Ok(Some(info)) => info,
            Ok(None) => {
                tracing::warn!(
                    "No user info found for failed message {}",
                    input.message_sid
                );
                return false;
            }
            Err(e) => {
                tracing::error!(
                    "Failed to get user info for message {}: {}",
                    input.message_sid,
                    e
                );
                return false;
            }
        };

        let country = get_country_code_from_phone(&user_info.to_number)
            .unwrap_or_else(|| "Unknown".to_string());
        let from = user_info
            .from_number
            .unwrap_or_else(|| "Unknown".to_string());

        // Clone values for the spawned task
        let user_id = user_info.user_id;
        let to_number = user_info.to_number.clone();
        let error_code = input.error_code.clone();
        let error_message = input.error_message.clone();

        // Spawn email sending to not block the webhook response
        tokio::spawn(async move {
            if let Err(e) = send_sms_failure_admin_email(
                user_id,
                &to_number,
                &from,
                error_code.as_deref(),
                error_message.as_deref(),
                &country,
            )
            .await
            {
                tracing::error!("Failed to send SMS failure admin email: {}", e);
            }
        });

        true
    }

    /// Handle final status actions: fetch price and delete message.
    async fn handle_final_status(&self, message_sid: &str) {
        let message_sid = message_sid.to_string();
        let repository = self.repository.clone();
        let client = self.client.clone();
        let config = self.config.clone();

        // Spawn task to fetch price with retry, then delete message
        tokio::spawn(async move {
            // Fetch price with retry
            if config.fetch_price_on_final {
                let mut price_result = None;

                for (attempt, delay) in config.price_fetch_delays.iter().enumerate() {
                    tokio::time::sleep(std::time::Duration::from_secs(*delay)).await;

                    match client.fetch_message_price(&message_sid).await {
                        Ok(Some(price)) => {
                            price_result = Some(price);
                            break;
                        }
                        Ok(None) => {
                            if attempt < config.price_fetch_delays.len() - 1 {
                                tracing::info!(
                                    "Price fetch attempt {} for {} returned no price, retrying...",
                                    attempt + 1,
                                    message_sid
                                );
                            } else {
                                tracing::warn!(
                                    "Price fetch failed after {} attempts for {}, giving up",
                                    config.price_fetch_delays.len(),
                                    message_sid
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!("Price fetch error for {}: {}", message_sid, e);
                            break;
                        }
                    }
                }

                // Update price in DB if we got it
                if let Some(price) = price_result {
                    if let Err(e) = repository.update_message_price(
                        &message_sid,
                        price.price,
                        &price.price_unit,
                    ) {
                        tracing::error!(
                            "Failed to update price for message {}: {}",
                            message_sid,
                            e
                        );
                    } else {
                        tracing::info!(
                            "Updated price for message {}: {} {}",
                            message_sid,
                            price.price,
                            price.price_unit
                        );
                    }
                }
            }

            // Delete message from Twilio
            if config.delete_on_final {
                if let Err(e) = client.delete_message(&message_sid).await {
                    tracing::error!("Failed to delete message {}: {}", message_sid, e);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::mock_twilio_client::MockTwilioClient;
    use crate::repositories::mock_twilio_status_repository::{
        MockMessage, MockTwilioStatusRepository,
    };
    use crate::repositories::twilio_client::MessagePrice;

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
        TwilioStatusService::with_config(repo, client, config)
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
                price: 0.0075,
                price_unit: "USD".to_string(),
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
}
