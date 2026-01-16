//! Repository trait for Twilio status callback operations.
//!
//! Defines the minimal interface needed by the TwilioStatusService,
//! enabling mock implementations for unit testing.

use thiserror::Error;

/// Error type for Twilio status repository operations
#[derive(Debug, Error)]
pub enum TwilioStatusRepositoryError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Message not found: {0}")]
    MessageNotFound(String),
}

/// Data for updating message status
#[derive(Debug, Clone)]
pub struct StatusUpdate {
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub price: Option<f32>,
    pub price_unit: Option<String>,
}

/// User info associated with a message
#[derive(Debug, Clone)]
pub struct MessageUserInfo {
    pub user_id: i32,
    pub to_number: String,
    pub from_number: Option<String>,
}

/// Repository trait for Twilio status-related database operations.
///
/// This trait abstracts the database operations needed for processing
/// Twilio status callbacks, allowing for mock implementations in tests.
pub trait TwilioStatusRepository: Send + Sync {
    /// Update the status of a message in the database.
    /// Returns the number of rows affected (0 if message not found).
    fn update_message_status(
        &self,
        message_sid: &str,
        update: &StatusUpdate,
    ) -> Result<usize, TwilioStatusRepositoryError>;

    /// Update just the price fields for a message.
    fn update_message_price(
        &self,
        message_sid: &str,
        price: f32,
        price_unit: &str,
    ) -> Result<(), TwilioStatusRepositoryError>;

    /// Get user info associated with a message.
    fn get_message_user_info(
        &self,
        message_sid: &str,
    ) -> Result<Option<MessageUserInfo>, TwilioStatusRepositoryError>;
}
