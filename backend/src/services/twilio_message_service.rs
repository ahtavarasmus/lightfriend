//! Twilio message service containing business logic for SMS operations.
//!
//! This service handles credential resolution, message sending, and database logging,
//! separating business logic from the underlying Twilio API calls.

use std::env;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use diesel::prelude::*;
use thiserror::Error;
use tokio::time::sleep;

use crate::api::twilio_client::{
    MessagePrice, SendMessageOptions, TwilioClient, TwilioClientError, TwilioCredentials,
};
use crate::models::user_models::{NewMessageStatusLog, User};
use crate::repositories::user_core::UserCore;
use crate::schema::message_status_log;
use crate::DbPool;

/// Errors that can occur during message operations.
#[derive(Debug, Error)]
pub enum TwilioMessageError {
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),

    #[error("User has no Twilio credentials configured")]
    NoCredentials,

    #[error("Twilio API error: {0}")]
    TwilioApi(#[from] TwilioClientError),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Failed to determine sending number for user")]
    NoSendingNumber,

    #[error("Other error: {0}")]
    Other(String),
}

/// Configuration for how to send a message.
#[derive(Debug, Clone)]
pub struct SendConfig {
    /// The destination phone number.
    pub to: String,
    /// The message body.
    pub body: String,
    /// Optional media URL to attach.
    pub media_url: Option<String>,
}

/// Result of sending a message through the service.
#[derive(Debug, Clone)]
pub struct MessageSendResult {
    /// The Twilio message SID.
    pub message_sid: String,
    /// The "From" number used (if applicable).
    pub from_number: Option<String>,
}

/// Service for handling Twilio message operations.
///
/// This service abstracts the complexity of:
/// - Resolving credentials (BYOT vs global, country-specific)
/// - Determining the correct "From" number
/// - Database logging for message status tracking
pub struct TwilioMessageService<T: TwilioClient> {
    twilio_client: Arc<T>,
    db_pool: DbPool,
}

impl<T: TwilioClient> TwilioMessageService<T> {
    /// Create a new TwilioMessageService.
    pub fn new(twilio_client: Arc<T>, db_pool: DbPool) -> Self {
        Self {
            twilio_client,
            db_pool,
        }
    }

    /// Resolve Twilio credentials for a user.
    ///
    /// Credential resolution logic:
    /// 1. BYOT users always use their own credentials
    /// 2. Users in local-number or notification-only countries use global credentials
    /// 3. Other users must have their own credentials
    pub fn resolve_credentials(
        &self,
        user: &User,
        user_core: &UserCore,
    ) -> Result<TwilioCredentials, TwilioMessageError> {
        // BYOT users with their own credentials always use their own account
        if user_core.is_byot_user(user.id) {
            let (account_sid, auth_token) = user_core
                .get_twilio_credentials(user.id)
                .map_err(|e| TwilioMessageError::Database(e.to_string()))?;
            return Ok(TwilioCredentials::new(account_sid, auth_token));
        }

        // Check if user is in a supported country that uses global credentials
        let is_local = crate::utils::country::is_local_number_country(&user.phone_number);
        let is_notification_only =
            crate::utils::country::is_notification_only_country(&user.phone_number);

        if is_local || is_notification_only {
            // Use global Twilio credentials
            let account_sid = env::var("TWILIO_ACCOUNT_SID")
                .map_err(|_| TwilioMessageError::MissingEnvVar("TWILIO_ACCOUNT_SID".into()))?;
            let auth_token = env::var("TWILIO_AUTH_TOKEN")
                .map_err(|_| TwilioMessageError::MissingEnvVar("TWILIO_AUTH_TOKEN".into()))?;
            return Ok(TwilioCredentials::new(account_sid, auth_token));
        }

        // Non-supported country must have their own credentials
        let (account_sid, auth_token) = user_core
            .get_twilio_credentials(user.id)
            .map_err(|_| TwilioMessageError::NoCredentials)?;
        Ok(TwilioCredentials::new(account_sid, auth_token))
    }

    /// Determine the sending strategy for a user (From number vs Messaging Service).
    ///
    /// Returns (from_number, use_messaging_service, should_update_preferred).
    pub fn determine_sending_strategy(
        &self,
        user: &User,
        user_core: &UserCore,
    ) -> Result<(Option<String>, bool, bool), TwilioMessageError> {
        let preferred = user.preferred_number.as_deref().unwrap_or("");
        let has_byot_credentials = user_core.is_byot_user(user.id);
        let is_notification_only =
            crate::utils::country::is_notification_only_country(&user.phone_number);

        let mut from_number: Option<String> = None;
        let mut use_messaging_service = false;
        let mut update_preferred = false;

        // Notification-only countries without BYOT: check if user selected US or local number
        if is_notification_only && !has_byot_credentials {
            let us_phone = env::var("USA_PHONE").ok();
            if preferred.is_empty() || us_phone.as_deref() == Some(preferred) {
                use_messaging_service = true;
                tracing::info!(
                    "Using US messaging service for notification-only country user {}",
                    user.id
                );
            } else {
                from_number = Some(preferred.to_string());
                tracing::info!(
                    "Using selected local number {} for notification-only user {}",
                    preferred,
                    user.id
                );
            }
        } else if let Some(ref country) = user.phone_number_country {
            match country.as_str() {
                "US" => {
                    use_messaging_service = true;
                }
                "CA" => {
                    if !preferred.is_empty() {
                        from_number = Some(preferred.to_string());
                    } else {
                        update_preferred = true;
                        from_number =
                            Some(env::var("CAN_PHONE").map_err(|_| {
                                TwilioMessageError::MissingEnvVar("CAN_PHONE".into())
                            })?);
                    }
                }
                "FI" => {
                    if !preferred.is_empty() {
                        from_number = Some(preferred.to_string());
                    } else {
                        update_preferred = true;
                        from_number =
                            Some(env::var("FIN_PHONE").map_err(|_| {
                                TwilioMessageError::MissingEnvVar("FIN_PHONE".into())
                            })?);
                    }
                }
                "NL" => {
                    if !preferred.is_empty() {
                        from_number = Some(preferred.to_string());
                    } else {
                        update_preferred = true;
                        from_number =
                            Some(env::var("NL_PHONE").map_err(|_| {
                                TwilioMessageError::MissingEnvVar("NL_PHONE".into())
                            })?);
                    }
                }
                "GB" => {
                    if !preferred.is_empty() {
                        from_number = Some(preferred.to_string());
                    } else {
                        update_preferred = true;
                        from_number =
                            Some(env::var("GB_PHONE").map_err(|_| {
                                TwilioMessageError::MissingEnvVar("GB_PHONE".into())
                            })?);
                    }
                }
                "AU" => {
                    if !preferred.is_empty() {
                        from_number = Some(preferred.to_string());
                    } else {
                        update_preferred = true;
                        from_number =
                            Some(env::var("AUS_PHONE").map_err(|_| {
                                TwilioMessageError::MissingEnvVar("AUS_PHONE".into())
                            })?);
                    }
                }
                _ => {
                    // For other countries with BYOT credentials, use their preferred number
                    if has_byot_credentials && !preferred.is_empty() {
                        from_number = Some(preferred.to_string());
                    } else {
                        tracing::info!(
                            "Using empty from_number for unsupported country: {}",
                            country
                        );
                    }
                }
            }
        }

        Ok((from_number, use_messaging_service, update_preferred))
    }

    /// Send a conversation message to a user.
    ///
    /// This is the main entry point for sending messages, handling:
    /// - Credential resolution
    /// - From number selection
    /// - Message history logging
    /// - Status tracking
    pub async fn send_conversation_message(
        &self,
        user: &User,
        user_core: &UserCore,
        config: SendConfig,
    ) -> Result<MessageSendResult, TwilioMessageError> {
        // Log to message history
        self.log_message_history(user.id, &config.body)?;

        // Skip sending in development environment
        let running_environment = env::var("ENVIRONMENT").unwrap_or_default();
        if running_environment == "development" {
            tracing::info!("NOT SENDING MESSAGE SINCE ENVIRONMENT IS DEVELOPMENT");
            return Ok(MessageSendResult {
                message_sid: "dev_not_sending".to_string(),
                from_number: None,
            });
        }

        // Resolve credentials
        let credentials = self.resolve_credentials(user, user_core)?;

        // Determine sending strategy
        let (from_number, use_messaging_service, update_preferred) =
            self.determine_sending_strategy(user, user_core)?;

        // Update preferred number if needed
        if update_preferred {
            if let Some(ref num) = from_number {
                if let Err(e) = user_core.update_preferred_number(user.id, num) {
                    tracing::error!(
                        "Failed to update preferred_number for user {}: {:?}",
                        user.id,
                        e
                    );
                }
            }
        }

        // Build send options
        let messaging_service_sid = if use_messaging_service {
            Some(env::var("TWILIO_MESSAGING_SERVICE_SID").map_err(|_| {
                TwilioMessageError::MissingEnvVar("TWILIO_MESSAGING_SERVICE_SID".into())
            })?)
        } else {
            None
        };

        let status_callback_url = env::var("SERVER_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|url| format!("{}/api/twilio/status-callback", url));

        let options = SendMessageOptions {
            to: config.to,
            body: config.body,
            media_url: config.media_url,
            from: from_number.clone(),
            messaging_service_sid,
            status_callback_url,
        };

        // Warn if no valid From
        if options.from.is_none() && options.messaging_service_sid.is_none() {
            tracing::warn!(
                "No valid From available for user {} and country {:?}",
                user.id,
                user.phone_number_country
            );
        }

        // Send the message
        let result = self
            .twilio_client
            .send_message(&credentials, options)
            .await?;

        tracing::debug!("Successfully sent message with SID: {}", result.message_sid);

        // Log initial status to database
        self.log_message_status(user, &result.message_sid, from_number.as_deref())?;

        Ok(MessageSendResult {
            message_sid: result.message_sid,
            from_number,
        })
    }

    /// Delete a Twilio message with retry logic.
    ///
    /// Waits 60 seconds before the first attempt to avoid 'resource not complete' errors,
    /// then retries up to 3 times with increasing delays.
    pub async fn delete_message_with_retry(
        &self,
        user: &User,
        user_core: &UserCore,
        message_sid: &str,
    ) -> Result<(), TwilioMessageError> {
        tracing::debug!("Deleting incoming message: {}", message_sid);

        let credentials = self.resolve_credentials(user, user_core)?;

        // Wait 1 minute to avoid 'resource not complete' errors
        sleep(Duration::from_secs(60)).await;

        let mut attempts = 0;
        loop {
            match self
                .twilio_client
                .delete_message(&credentials, message_sid)
                .await
            {
                Ok(()) => {
                    tracing::info!("Incoming message deleted: {}", message_sid);
                    return Ok(());
                }
                Err(e) if attempts < 3 => {
                    attempts += 1;
                    let wait_secs = 60 * attempts as u64;
                    tracing::warn!("Retry deletion after {} seconds: {:?}", wait_secs, e);
                    sleep(Duration::from_secs(wait_secs)).await;
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
    }

    /// Delete media attached to a message.
    pub async fn delete_message_media(
        &self,
        user: &User,
        user_core: &UserCore,
        message_sid: &str,
        media_sid: &str,
    ) -> Result<(), TwilioMessageError> {
        let credentials = self.resolve_credentials(user, user_core)?;
        self.twilio_client
            .delete_message_media(&credentials, message_sid, media_sid)
            .await?;
        tracing::debug!("Successfully deleted message media: {}", media_sid);
        Ok(())
    }

    /// Fetch the price of a sent message.
    pub async fn fetch_message_price(
        &self,
        user: &User,
        user_core: &UserCore,
        message_sid: &str,
    ) -> Result<Option<MessagePrice>, TwilioMessageError> {
        let credentials = self.resolve_credentials(user, user_core)?;
        Ok(self
            .twilio_client
            .fetch_message_price(&credentials, message_sid)
            .await?)
    }

    /// Log message to user's message history.
    fn log_message_history(&self, user_id: i32, body: &str) -> Result<(), TwilioMessageError> {
        use crate::models::user_models::NewMessageHistory;
        use crate::schema::message_history;

        let history_entry = NewMessageHistory {
            user_id,
            role: "assistant".to_string(),
            encrypted_content: body.to_string(),
            tool_name: None,
            tool_call_id: None,
            tool_calls_json: None,
            created_at: chrono::Utc::now().timestamp() as i32,
            conversation_id: "".to_string(),
        };

        let mut conn = self
            .db_pool
            .get()
            .map_err(|e| TwilioMessageError::Database(e.to_string()))?;

        diesel::insert_into(message_history::table)
            .values(&history_entry)
            .execute(&mut conn)
            .map_err(|e| {
                tracing::error!("Failed to store message in history: {}", e);
                TwilioMessageError::Database(e.to_string())
            })?;

        Ok(())
    }

    /// Log message status to database for tracking.
    fn log_message_status(
        &self,
        user: &User,
        message_sid: &str,
        from_number: Option<&str>,
    ) -> Result<(), TwilioMessageError> {
        let mut conn = match self.db_pool.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to get DB connection for status logging: {}", e);
                return Ok(()); // Don't fail the send operation
            }
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_status = NewMessageStatusLog {
            message_sid: message_sid.to_string(),
            user_id: user.id,
            direction: "outbound".to_string(),
            to_number: user.phone_number.clone(),
            from_number: from_number.map(|s| s.to_string()),
            status: "queued".to_string(),
            error_code: None,
            error_message: None,
            created_at: now,
            updated_at: now,
            price: None,
            price_unit: None,
        };

        if let Err(e) = diesel::insert_into(message_status_log::table)
            .values(&new_status)
            .execute(&mut conn)
        {
            tracing::error!(
                "Failed to log message status for SID {}: {}",
                message_sid,
                e
            );
        } else {
            tracing::info!("Logged initial message status for SID {}", message_sid);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require a test database setup.
    // These are basic unit tests for the service logic.

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
}
