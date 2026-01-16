//! Client trait for Twilio API operations.
//!
//! Defines the interface for interacting with Twilio's Messages API,
//! enabling mock implementations for unit testing.

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
