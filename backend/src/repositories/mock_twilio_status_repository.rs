//! Mock implementation of TwilioStatusRepository for unit testing.
//!
//! Provides an in-memory implementation that can be used in tests
//! without requiring a database connection.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::repositories::twilio_status_repository::{
    MessageUserInfo, StatusUpdate, TwilioStatusRepository, TwilioStatusRepositoryError,
};

/// Mock message data stored in memory
#[derive(Clone, Debug)]
pub struct MockMessage {
    pub message_sid: String,
    pub user_id: i32,
    pub to_number: String,
    pub from_number: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub price: Option<f32>,
    pub price_unit: Option<String>,
}

/// Mock repository for testing TwilioStatusService without a database.
pub struct MockTwilioStatusRepository {
    messages: Mutex<HashMap<String, MockMessage>>,
    // Configurable failure modes
    fail_on_update: Mutex<bool>,
    fail_on_get_user_info: Mutex<bool>,
}

impl Default for MockTwilioStatusRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl MockTwilioStatusRepository {
    /// Create a new empty mock repository.
    pub fn new() -> Self {
        Self {
            messages: Mutex::new(HashMap::new()),
            fail_on_update: Mutex::new(false),
            fail_on_get_user_info: Mutex::new(false),
        }
    }

    /// Add a message to the mock repository.
    pub fn add_message(&self, message: MockMessage) {
        let mut messages = self.messages.lock().unwrap();
        messages.insert(message.message_sid.clone(), message);
    }

    /// Configure to fail on next update_message_status call.
    pub fn set_fail_on_update(&self, fail: bool) {
        *self.fail_on_update.lock().unwrap() = fail;
    }

    /// Configure to fail on get_message_user_info calls.
    pub fn set_fail_on_get_user_info(&self, fail: bool) {
        *self.fail_on_get_user_info.lock().unwrap() = fail;
    }

    /// Get a message from the mock repository.
    pub fn get_message(&self, message_sid: &str) -> Option<MockMessage> {
        let messages = self.messages.lock().unwrap();
        messages.get(message_sid).cloned()
    }

    /// Get the count of messages in the repository.
    pub fn message_count(&self) -> usize {
        self.messages.lock().unwrap().len()
    }

    fn simulate_db_error() -> TwilioStatusRepositoryError {
        TwilioStatusRepositoryError::Database("Simulated database failure".to_string())
    }
}

impl TwilioStatusRepository for MockTwilioStatusRepository {
    fn update_message_status(
        &self,
        message_sid: &str,
        update: &StatusUpdate,
    ) -> Result<usize, TwilioStatusRepositoryError> {
        if *self.fail_on_update.lock().unwrap() {
            return Err(Self::simulate_db_error());
        }

        let mut messages = self.messages.lock().unwrap();
        if let Some(msg) = messages.get_mut(message_sid) {
            msg.status = update.status.clone();
            msg.error_code = update.error_code.clone();
            msg.error_message = update.error_message.clone();
            if let Some(price) = update.price {
                msg.price = Some(price);
            }
            if let Some(ref price_unit) = update.price_unit {
                msg.price_unit = Some(price_unit.clone());
            }
            Ok(1)
        } else {
            Ok(0)
        }
    }

    fn update_message_price(
        &self,
        message_sid: &str,
        price: f32,
        price_unit: &str,
    ) -> Result<(), TwilioStatusRepositoryError> {
        if *self.fail_on_update.lock().unwrap() {
            return Err(Self::simulate_db_error());
        }

        let mut messages = self.messages.lock().unwrap();
        if let Some(msg) = messages.get_mut(message_sid) {
            msg.price = Some(price);
            msg.price_unit = Some(price_unit.to_string());
            Ok(())
        } else {
            Ok(()) // Silently succeed even if not found
        }
    }

    fn get_message_user_info(
        &self,
        message_sid: &str,
    ) -> Result<Option<MessageUserInfo>, TwilioStatusRepositoryError> {
        if *self.fail_on_get_user_info.lock().unwrap() {
            return Err(Self::simulate_db_error());
        }

        let messages = self.messages.lock().unwrap();
        Ok(messages.get(message_sid).map(|msg| MessageUserInfo {
            user_id: msg.user_id,
            to_number: msg.to_number.clone(),
            from_number: msg.from_number.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_message() {
        let repo = MockTwilioStatusRepository::new();

        let message = MockMessage {
            message_sid: "SM123".to_string(),
            user_id: 1,
            to_number: "+14155551234".to_string(),
            from_number: Some("+14155559999".to_string()),
            status: "queued".to_string(),
            error_code: None,
            error_message: None,
            price: None,
            price_unit: None,
        };

        repo.add_message(message);
        assert_eq!(repo.message_count(), 1);

        let retrieved = repo.get_message("SM123").unwrap();
        assert_eq!(retrieved.user_id, 1);
        assert_eq!(retrieved.status, "queued");
    }

    #[test]
    fn test_update_message_status() {
        let repo = MockTwilioStatusRepository::new();

        let message = MockMessage {
            message_sid: "SM123".to_string(),
            user_id: 1,
            to_number: "+14155551234".to_string(),
            from_number: None,
            status: "queued".to_string(),
            error_code: None,
            error_message: None,
            price: None,
            price_unit: None,
        };

        repo.add_message(message);

        let update = StatusUpdate {
            status: "delivered".to_string(),
            error_code: None,
            error_message: None,
            price: Some(0.0075),
            price_unit: Some("USD".to_string()),
        };

        let result = repo.update_message_status("SM123", &update).unwrap();
        assert_eq!(result, 1);

        let updated = repo.get_message("SM123").unwrap();
        assert_eq!(updated.status, "delivered");
        assert_eq!(updated.price, Some(0.0075));
    }

    #[test]
    fn test_update_nonexistent_message() {
        let repo = MockTwilioStatusRepository::new();

        let update = StatusUpdate {
            status: "delivered".to_string(),
            error_code: None,
            error_message: None,
            price: None,
            price_unit: None,
        };

        let result = repo
            .update_message_status("SM_NONEXISTENT", &update)
            .unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_failure_mode() {
        let repo = MockTwilioStatusRepository::new();
        repo.set_fail_on_update(true);

        let update = StatusUpdate {
            status: "delivered".to_string(),
            error_code: None,
            error_message: None,
            price: None,
            price_unit: None,
        };

        let result = repo.update_message_status("SM123", &update);
        assert!(result.is_err());
    }
}
