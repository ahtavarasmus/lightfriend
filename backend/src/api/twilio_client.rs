//! Client trait and implementations for Twilio API operations.
//!
//! This module provides:
//! - `TwilioClient` trait: interface for Twilio API operations
//! - `RealTwilioClient`: production implementation using reqwest
//! - `MockTwilioClient`: test implementation (cfg(test) only)

use async_trait::async_trait;
use thiserror::Error;

/// Error type for Twilio client operations
#[derive(Debug, Error)]
pub enum TwilioClientError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(String),

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("API error: {0}")]
    ApiError(String),
}

/// Price information for a message
#[derive(Debug, Clone)]
pub struct MessagePrice {
    pub price: f32,
    pub price_unit: String,
}

/// Trait for Twilio API client operations.
///
/// This trait abstracts the HTTP calls to Twilio's API,
/// allowing for mock implementations in tests.
#[async_trait]
pub trait TwilioClient: Send + Sync {
    /// Fetch the price of a message from Twilio.
    /// Returns None if the price is not yet available.
    async fn fetch_message_price(
        &self,
        message_sid: &str,
    ) -> Result<Option<MessagePrice>, TwilioClientError>;

    /// Delete a message from Twilio.
    async fn delete_message(&self, message_sid: &str) -> Result<(), TwilioClientError>;
}

// ============================================================================
// Real Implementation
// ============================================================================

use reqwest::Client;
use serde::Deserialize;

/// Response from Twilio Messages API when fetching message details
#[derive(Deserialize, Debug)]
struct TwilioMessageResponse {
    #[serde(default)]
    price: Option<String>,
    #[serde(default)]
    price_unit: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

/// Real Twilio client implementation.
pub struct RealTwilioClient {
    account_sid: String,
    auth_token: String,
    client: Client,
}

impl RealTwilioClient {
    /// Create a new client with the given credentials.
    pub fn new(account_sid: String, auth_token: String) -> Self {
        Self {
            account_sid,
            auth_token,
            client: Client::new(),
        }
    }

    /// Create a client from environment variables.
    pub fn from_env() -> Option<Self> {
        let account_sid = std::env::var("TWILIO_ACCOUNT_SID").ok()?;
        let auth_token = std::env::var("TWILIO_AUTH_TOKEN").ok()?;
        Some(Self::new(account_sid, auth_token))
    }
}

#[async_trait]
impl TwilioClient for RealTwilioClient {
    async fn fetch_message_price(
        &self,
        message_sid: &str,
    ) -> Result<Option<MessagePrice>, TwilioClientError> {
        let url = format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/Messages/{}.json",
            self.account_sid, message_sid
        );

        let response = self
            .client
            .get(&url)
            .basic_auth(&self.account_sid, Some(&self.auth_token))
            .send()
            .await
            .map_err(|e| TwilioClientError::RequestFailed(e.to_string()))?;

        if response.status().is_success() {
            let msg: TwilioMessageResponse = response.json().await.map_err(|e| {
                TwilioClientError::ApiError(format!("Failed to parse response: {}", e))
            })?;

            tracing::info!(
                "Twilio message {} response: status={:?}, price={:?}, price_unit={:?}",
                message_sid,
                msg.status,
                msg.price,
                msg.price_unit
            );

            if let (Some(price_str), Some(price_unit)) = (msg.price, msg.price_unit) {
                if let Ok(price) = price_str.parse::<f32>() {
                    tracing::info!(
                        "Fetched price for message {}: {} {}",
                        message_sid,
                        price,
                        price_unit
                    );
                    return Ok(Some(MessagePrice { price, price_unit }));
                }
            }

            tracing::warn!(
                "Message {} has no price info yet (status: {:?})",
                message_sid,
                msg.status
            );
            Ok(None)
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            tracing::warn!(
                "Message {} not found in Twilio (already deleted?)",
                message_sid
            );
            Ok(None)
        } else {
            Err(TwilioClientError::ApiError(format!(
                "Failed to fetch message {}: status {}",
                message_sid,
                response.status()
            )))
        }
    }

    async fn delete_message(&self, message_sid: &str) -> Result<(), TwilioClientError> {
        let url = format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/Messages/{}.json",
            self.account_sid, message_sid
        );

        let response = self
            .client
            .delete(&url)
            .basic_auth(&self.account_sid, Some(&self.auth_token))
            .send()
            .await
            .map_err(|e| TwilioClientError::RequestFailed(e.to_string()))?;

        if response.status().is_success() {
            tracing::info!("Deleted message {} from Twilio", message_sid);
            Ok(())
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            // Message already deleted - that's fine
            tracing::info!("Message {} already deleted from Twilio", message_sid);
            Ok(())
        } else {
            Err(TwilioClientError::RequestFailed(format!(
                "Failed to delete message {}: status {}",
                message_sid,
                response.status()
            )))
        }
    }
}

// ============================================================================
// Mock Implementation
// ============================================================================

pub mod mock {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Mutex;

    /// Mock Twilio client for testing.
    pub struct MockTwilioClient {
        // Configurable responses
        price_responses: Mutex<std::collections::HashMap<String, Option<MessagePrice>>>,
        // Track which messages have been deleted
        deleted_messages: Mutex<HashSet<String>>,
        // Configurable failure modes
        fail_on_fetch_price: Mutex<bool>,
        fail_on_delete: Mutex<bool>,
        // Track call counts
        fetch_price_calls: Mutex<Vec<String>>,
        delete_calls: Mutex<Vec<String>>,
    }

    impl Default for MockTwilioClient {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MockTwilioClient {
        /// Create a new mock client.
        pub fn new() -> Self {
            Self {
                price_responses: Mutex::new(std::collections::HashMap::new()),
                deleted_messages: Mutex::new(HashSet::new()),
                fail_on_fetch_price: Mutex::new(false),
                fail_on_delete: Mutex::new(false),
                fetch_price_calls: Mutex::new(Vec::new()),
                delete_calls: Mutex::new(Vec::new()),
            }
        }

        /// Set the price response for a specific message.
        pub fn set_price_response(&self, message_sid: &str, price: Option<MessagePrice>) {
            let mut responses = self.price_responses.lock().unwrap();
            responses.insert(message_sid.to_string(), price);
        }

        /// Set a default price response for any message.
        pub fn set_default_price(&self, price: f32, price_unit: &str) {
            let mut responses = self.price_responses.lock().unwrap();
            responses.insert(
                "*".to_string(),
                Some(MessagePrice {
                    price,
                    price_unit: price_unit.to_string(),
                }),
            );
        }

        /// Configure to fail on fetch_message_price calls.
        pub fn set_fail_on_fetch_price(&self, fail: bool) {
            *self.fail_on_fetch_price.lock().unwrap() = fail;
        }

        /// Configure to fail on delete_message calls.
        pub fn set_fail_on_delete(&self, fail: bool) {
            *self.fail_on_delete.lock().unwrap() = fail;
        }

        /// Check if a message was deleted.
        pub fn was_deleted(&self, message_sid: &str) -> bool {
            self.deleted_messages.lock().unwrap().contains(message_sid)
        }

        /// Get all deleted messages.
        pub fn get_deleted_messages(&self) -> Vec<String> {
            self.deleted_messages
                .lock()
                .unwrap()
                .iter()
                .cloned()
                .collect()
        }

        /// Get the number of fetch_price calls.
        pub fn fetch_price_call_count(&self) -> usize {
            self.fetch_price_calls.lock().unwrap().len()
        }

        /// Get the number of delete calls.
        pub fn delete_call_count(&self) -> usize {
            self.delete_calls.lock().unwrap().len()
        }

        /// Get all message SIDs that had fetch_price called.
        pub fn get_fetch_price_calls(&self) -> Vec<String> {
            self.fetch_price_calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl TwilioClient for MockTwilioClient {
        async fn fetch_message_price(
            &self,
            message_sid: &str,
        ) -> Result<Option<MessagePrice>, TwilioClientError> {
            // Track the call
            self.fetch_price_calls
                .lock()
                .unwrap()
                .push(message_sid.to_string());

            if *self.fail_on_fetch_price.lock().unwrap() {
                return Err(TwilioClientError::RequestFailed(
                    "Simulated fetch failure".to_string(),
                ));
            }

            let responses = self.price_responses.lock().unwrap();

            // Check for specific message response first, then default
            if let Some(price) = responses.get(message_sid) {
                Ok(price.clone())
            } else if let Some(price) = responses.get("*") {
                Ok(price.clone())
            } else {
                Ok(None)
            }
        }

        async fn delete_message(&self, message_sid: &str) -> Result<(), TwilioClientError> {
            // Track the call
            self.delete_calls
                .lock()
                .unwrap()
                .push(message_sid.to_string());

            if *self.fail_on_delete.lock().unwrap() {
                return Err(TwilioClientError::RequestFailed(
                    "Simulated delete failure".to_string(),
                ));
            }

            self.deleted_messages
                .lock()
                .unwrap()
                .insert(message_sid.to_string());
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn test_default_price_response() {
            let client = MockTwilioClient::new();
            client.set_default_price(0.0075, "USD");

            let result = client.fetch_message_price("SM123").await.unwrap();
            assert!(result.is_some());

            let price = result.unwrap();
            assert_eq!(price.price, 0.0075);
            assert_eq!(price.price_unit, "USD");
        }

        #[tokio::test]
        async fn test_specific_message_price() {
            let client = MockTwilioClient::new();
            client.set_price_response(
                "SM456",
                Some(MessagePrice {
                    price: 0.01,
                    price_unit: "EUR".to_string(),
                }),
            );

            let result = client.fetch_message_price("SM456").await.unwrap();
            assert!(result.is_some());

            let price = result.unwrap();
            assert_eq!(price.price, 0.01);
            assert_eq!(price.price_unit, "EUR");
        }

        #[tokio::test]
        async fn test_no_price_available() {
            let client = MockTwilioClient::new();
            client.set_price_response("SM789", None);

            let result = client.fetch_message_price("SM789").await.unwrap();
            assert!(result.is_none());
        }

        #[tokio::test]
        async fn test_delete_tracking() {
            let client = MockTwilioClient::new();

            client.delete_message("SM123").await.unwrap();
            client.delete_message("SM456").await.unwrap();

            assert!(client.was_deleted("SM123"));
            assert!(client.was_deleted("SM456"));
            assert!(!client.was_deleted("SM789"));
            assert_eq!(client.delete_call_count(), 2);
        }

        #[tokio::test]
        async fn test_failure_modes() {
            let client = MockTwilioClient::new();
            client.set_fail_on_fetch_price(true);

            let result = client.fetch_message_price("SM123").await;
            assert!(result.is_err());

            client.set_fail_on_delete(true);
            let result = client.delete_message("SM123").await;
            assert!(result.is_err());
        }
    }
}
