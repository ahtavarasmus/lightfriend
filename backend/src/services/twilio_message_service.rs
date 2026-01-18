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
use crate::repositories::user_repository::UserRepository;
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
    user_core: Arc<UserCore>,
    user_repository: Arc<UserRepository>,
}

impl<T: TwilioClient> TwilioMessageService<T> {
    /// Create a new TwilioMessageService.
    pub fn new(
        twilio_client: Arc<T>,
        db_pool: DbPool,
        user_core: Arc<UserCore>,
        user_repository: Arc<UserRepository>,
    ) -> Self {
        Self {
            twilio_client,
            db_pool,
            user_core,
            user_repository,
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
    ) -> Result<TwilioCredentials, TwilioMessageError> {
        // BYOT users with their own credentials always use their own account
        if self.user_core.is_byot_user(user.id) {
            let (account_sid, auth_token) = self
                .user_core
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
        let (account_sid, auth_token) = self
            .user_core
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
    ) -> Result<(Option<String>, bool, bool), TwilioMessageError> {
        let preferred = user.preferred_number.as_deref().unwrap_or("");
        let has_byot_credentials = self.user_core.is_byot_user(user.id);
        let is_notification_only =
            crate::utils::country::is_notification_only_country(&user.phone_number);

        // Detect country from phone number using libphonenumber
        let country = crate::utils::country::get_country_code_from_phone(&user.phone_number)
            .or_else(|| Some("Other".to_string()));

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
        } else if let Some(ref c) = country {
            match c.as_str() {
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
                        tracing::info!("Using empty from_number for unsupported country: {}", c);
                    }
                }
            }
        }

        Ok((from_number, use_messaging_service, update_preferred))
    }

    /// Send an SMS message to a user - convenience method matching the legacy twilio_utils interface.
    ///
    /// This is the primary method call sites should use. It handles:
    /// - Credential resolution (BYOT vs global)
    /// - Country detection and setting if missing
    /// - From number selection
    /// - Media SID to URL conversion
    /// - Message history logging
    /// - Status tracking
    ///
    /// Returns the message SID on success.
    pub async fn send_sms(
        &self,
        body: &str,
        media_sid: Option<&String>,
        user: &User,
    ) -> Result<String, TwilioMessageError> {
        // Log to message history using repository
        let history_entry = crate::models::user_models::NewMessageHistory {
            user_id: user.id,
            role: "assistant".to_string(),
            encrypted_content: body.to_string(),
            tool_name: None,
            tool_call_id: None,
            tool_calls_json: None,
            created_at: chrono::Utc::now().timestamp() as i32,
            conversation_id: "".to_string(),
        };

        if let Err(e) = self.user_repository.create_message_history(&history_entry) {
            tracing::error!("Failed to store message in history: {}", e);
        }

        // Skip sending in development environment
        let running_environment = env::var("ENVIRONMENT").unwrap_or_default();
        if running_environment == "development" {
            tracing::info!("NOT SENDING MESSAGE SINCE ENVIRONMENT IS DEVELOPMENT");
            return Ok("dev_not_sending".to_string());
        }

        // Resolve credentials first (needed for media URL construction)
        let credentials = self.resolve_credentials(user)?;

        // Convert media_sid to full URL if provided
        let media_url = media_sid.map(|media_id| {
            format!(
                "https://api.twilio.com/2010-04-01/Accounts/{}/Media/{}",
                credentials.account_sid, media_id
            )
        });

        // Determine sending strategy
        let (from_number, use_messaging_service, update_preferred) =
            self.determine_sending_strategy(user)?;

        // Update preferred number if needed
        if update_preferred {
            if let Some(ref num) = from_number {
                if let Err(e) = self.user_core.update_preferred_number(user.id, num) {
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
            to: user.phone_number.clone(),
            body: body.to_string(),
            media_url,
            from: from_number.clone(),
            messaging_service_sid,
            status_callback_url,
        };

        // Warn if no valid From
        if options.from.is_none() && options.messaging_service_sid.is_none() {
            let detected_country =
                crate::utils::country::get_country_code_from_phone(&user.phone_number);
            tracing::warn!(
                "No valid From available for user {} and country {:?}",
                user.id,
                detected_country
            );
        }

        // Send the message
        let result = self
            .twilio_client
            .send_message(&credentials, options)
            .await?;

        let message_sid = result.message_sid;
        tracing::debug!(
            "Successfully sent message{} with SID: {}",
            if media_sid.is_some() {
                " with media"
            } else {
                ""
            },
            message_sid
        );

        // Log initial status to database
        self.log_message_status(user, &message_sid, from_number.as_deref())?;

        Ok(message_sid)
    }

    /// Send a conversation message to a user using SendConfig.
    ///
    /// This method provides more control over message parameters.
    /// For most cases, use `send_sms` instead.
    pub async fn send_conversation_message(
        &self,
        user: &User,
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
        let credentials = self.resolve_credentials(user)?;

        // Determine sending strategy
        let (from_number, use_messaging_service, update_preferred) =
            self.determine_sending_strategy(user)?;

        // Update preferred number if needed
        if update_preferred {
            if let Some(ref num) = from_number {
                if let Err(e) = self.user_core.update_preferred_number(user.id, num) {
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
            let detected_country =
                crate::utils::country::get_country_code_from_phone(&user.phone_number);
            tracing::warn!(
                "No valid From available for user {} and country {:?}",
                user.id,
                detected_country
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
        message_sid: &str,
    ) -> Result<(), TwilioMessageError> {
        tracing::debug!("Deleting incoming message: {}", message_sid);

        let credentials = self.resolve_credentials(user)?;

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
        message_sid: &str,
        media_sid: &str,
    ) -> Result<(), TwilioMessageError> {
        let credentials = self.resolve_credentials(user)?;
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
        message_sid: &str,
    ) -> Result<Option<MessagePrice>, TwilioMessageError> {
        let credentials = self.resolve_credentials(user)?;
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
    use crate::api::twilio_client::mock::MockTwilioClient;
    use crate::test_utils::{
        create_test_state, create_test_user, set_byot_credentials, set_preferred_number,
        setup_test_encryption, TestUserParams,
    };
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
        state: &Arc<crate::AppState>,
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

    #[test]
    fn test_strategy_fi_user_uses_preferred_number() {
        setup_test_env();
        let state = create_test_state();
        let params = TestUserParams::finland_user(10.0, 5.0);
        let user = create_test_user(&state, &params);

        set_preferred_number(&state, user.id, "+358409999999");
        let user = state.user_core.find_by_id(user.id).unwrap().unwrap();

        let (service, _mock) = create_test_service(&state);

        let (from_number, use_messaging_service, update_preferred) =
            service.determine_sending_strategy(&user).unwrap();

        assert!(!use_messaging_service);
        assert_eq!(from_number, Some("+358409999999".to_string()));
        assert!(!update_preferred);
    }

    #[test]
    fn test_strategy_fi_user_no_preferred_uses_fin_phone_env() {
        setup_test_env();
        let state = create_test_state();
        let params = TestUserParams::finland_user(10.0, 5.0);
        let user = create_test_user(&state, &params);

        let (service, _mock) = create_test_service(&state);

        let (from_number, use_messaging_service, update_preferred) =
            service.determine_sending_strategy(&user).unwrap();

        assert!(!use_messaging_service);
        assert_eq!(from_number, Some("+358401001234".to_string()));
        assert!(update_preferred);
    }

    #[test]
    fn test_strategy_nl_user_uses_preferred_number() {
        setup_test_env();
        let state = create_test_state();
        let params = TestUserParams::netherlands_user(10.0, 5.0);
        let user = create_test_user(&state, &params);

        set_preferred_number(&state, user.id, "+31209999999");
        let user = state.user_core.find_by_id(user.id).unwrap().unwrap();

        let (service, _mock) = create_test_service(&state);

        let (from_number, use_messaging_service, update_preferred) =
            service.determine_sending_strategy(&user).unwrap();

        assert!(!use_messaging_service);
        assert_eq!(from_number, Some("+31209999999".to_string()));
        assert!(!update_preferred);
    }

    #[test]
    fn test_strategy_gb_user_uses_preferred_number() {
        setup_test_env();
        let state = create_test_state();
        let params = TestUserParams::uk_user(10.0, 5.0);
        let user = create_test_user(&state, &params);

        set_preferred_number(&state, user.id, "+447911999999");
        let user = state.user_core.find_by_id(user.id).unwrap().unwrap();

        let (service, _mock) = create_test_service(&state);

        let (from_number, use_messaging_service, update_preferred) =
            service.determine_sending_strategy(&user).unwrap();

        assert!(!use_messaging_service);
        assert_eq!(from_number, Some("+447911999999".to_string()));
        assert!(!update_preferred);
    }

    #[test]
    fn test_strategy_au_user_uses_preferred_number() {
        setup_test_env();
        let state = create_test_state();
        let params = TestUserParams::australia_user(10.0, 5.0);
        let user = create_test_user(&state, &params);

        set_preferred_number(&state, user.id, "+61412999999");
        let user = state.user_core.find_by_id(user.id).unwrap().unwrap();

        let (service, _mock) = create_test_service(&state);

        let (from_number, use_messaging_service, update_preferred) =
            service.determine_sending_strategy(&user).unwrap();

        assert!(!use_messaging_service);
        assert_eq!(from_number, Some("+61412999999".to_string()));
        assert!(!update_preferred);
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

    #[test]
    fn test_strategy_noti_country_with_usa_phone_uses_messaging_service() {
        setup_test_env();
        let state = create_test_state();
        let params = TestUserParams::germany_user(10.0, 5.0);
        let user = create_test_user(&state, &params);

        // Set preferred_number to USA_PHONE value
        set_preferred_number(&state, user.id, "+18005551234");
        let user = state.user_core.find_by_id(user.id).unwrap().unwrap();

        let (service, _mock) = create_test_service(&state);

        let (from_number, use_messaging_service, update_preferred) =
            service.determine_sending_strategy(&user).unwrap();

        // Should still use messaging service when preferred == USA_PHONE
        assert!(use_messaging_service);
        assert!(from_number.is_none());
        assert!(!update_preferred);
    }

    #[test]
    fn test_strategy_noti_country_with_selected_fi_number_uses_that_number() {
        setup_test_env();
        let state = create_test_state();
        let params = TestUserParams::germany_user(10.0, 5.0);
        let user = create_test_user(&state, &params);

        // User selected a Finland number
        set_preferred_number(&state, user.id, "+358409876543");
        let user = state.user_core.find_by_id(user.id).unwrap().unwrap();

        let (service, _mock) = create_test_service(&state);

        let (from_number, use_messaging_service, update_preferred) =
            service.determine_sending_strategy(&user).unwrap();

        // Should use the selected local number, not messaging service
        assert!(
            !use_messaging_service,
            "Should use selected number, not messaging service"
        );
        assert_eq!(from_number, Some("+358409876543".to_string()));
        assert!(!update_preferred);
    }

    #[test]
    fn test_strategy_noti_country_with_selected_gb_number_uses_that_number() {
        setup_test_env();
        let state = create_test_state();
        let params = TestUserParams::france_user(10.0, 5.0);
        let user = create_test_user(&state, &params);

        // User selected a GB number
        set_preferred_number(&state, user.id, "+447123456789");
        let user = state.user_core.find_by_id(user.id).unwrap().unwrap();

        let (service, _mock) = create_test_service(&state);

        let (from_number, use_messaging_service, update_preferred) =
            service.determine_sending_strategy(&user).unwrap();

        assert!(!use_messaging_service);
        assert_eq!(from_number, Some("+447123456789".to_string()));
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
    async fn test_send_sms_notification_only_with_local_number() {
        setup_test_env();
        let state = create_test_state();
        let params = TestUserParams::germany_user(10.0, 5.0);
        let user = create_test_user(&state, &params);

        // User selected a Finland number as their preferred
        set_preferred_number(&state, user.id, "+358401112222");
        let user = state.user_core.find_by_id(user.id).unwrap().unwrap();

        let (service, mock_client) = create_test_service(&state);

        let result = service.send_sms("Hallo mit FI nummer!", None, &user).await;
        assert!(result.is_ok());

        let calls = mock_client.get_calls();
        let call = &calls.send_message_calls[0];

        assert!(call.messaging_service_sid.is_none());
        assert_eq!(call.from, Some("+358401112222".to_string()));
    }

    #[tokio::test]
    #[serial]
    async fn test_send_sms_logs_message_history() {
        setup_test_env();
        let state = create_test_state();
        let params = TestUserParams::us_user(10.0, 5.0);
        let user = create_test_user(&state, &params);

        let (service, _mock_client) = create_test_service(&state);

        let result = service
            .send_sms("Test message for history", None, &user)
            .await;
        assert!(result.is_ok());

        // Verify message was logged to history
        use crate::schema::message_history;
        use diesel::prelude::*;

        let mut conn = state.db_pool.get().unwrap();
        let count: i64 = message_history::table
            .filter(message_history::user_id.eq(user.id))
            .count()
            .get_result(&mut conn)
            .unwrap();

        assert!(count >= 1, "Message should be logged to history");
    }

    #[tokio::test]
    #[serial]
    async fn test_send_sms_with_media_url() {
        setup_test_env();
        let state = create_test_state();
        let params = TestUserParams::us_user(10.0, 5.0);
        let user = create_test_user(&state, &params);

        let (service, mock_client) = create_test_service(&state);

        let media_sid = "ME123456789".to_string();
        let result = service
            .send_sms("Check this image", Some(&media_sid), &user)
            .await;
        assert!(result.is_ok());

        let calls = mock_client.get_calls();
        let call = &calls.send_message_calls[0];

        assert!(
            call.media_url.is_some(),
            "Media URL should be present in call"
        );
        // Media URL should be constructed from account_sid and media_sid
        let media_url = call.media_url.as_ref().unwrap();
        assert!(
            media_url.contains("ME123456789"),
            "Media URL should contain media SID"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_send_sms_byot_user_uses_own_credentials() {
        setup_test_env();
        setup_test_encryption();
        let state = create_test_state();
        let params = TestUserParams::us_user(10.0, 5.0);
        let user = create_test_user(&state, &params);

        set_byot_credentials(&state, user.id);
        let user = state.user_core.find_by_id(user.id).unwrap().unwrap();

        let (service, mock_client) = create_test_service(&state);

        let result = service.send_sms("BYOT test", None, &user).await;
        assert!(result.is_ok());

        // BYOT user should still send via the mock, the difference is credentials
        let calls = mock_client.get_calls();
        assert_eq!(calls.send_message_calls.len(), 1);
    }

    #[tokio::test]
    #[serial]
    async fn test_send_sms_handles_twilio_error() {
        setup_test_env();
        let state = create_test_state();
        let params = TestUserParams::us_user(10.0, 5.0);
        let user = create_test_user(&state, &params);

        // Create mock that returns an error
        let mock_client =
            Arc::new(MockTwilioClient::new().with_send_result(Err("Twilio API error".to_string())));
        let service = TwilioMessageService::new(
            mock_client.clone(),
            state.db_pool.clone(),
            state.user_core.clone(),
            state.user_repository.clone(),
        );

        let result = service.send_sms("This will fail", None, &user).await;
        assert!(result.is_err(), "Should return error when Twilio fails");

        let err = result.unwrap_err();
        match err {
            TwilioMessageError::TwilioApi(_) => (),
            _ => panic!("Expected TwilioApi error, got {:?}", err),
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_send_sms_skips_in_development() {
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

        // Reset environment
        std::env::set_var("ENVIRONMENT", "test");
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
            Some(crate::api::twilio_client::MessagePrice {
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
}
