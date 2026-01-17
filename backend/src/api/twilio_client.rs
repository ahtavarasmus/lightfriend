//! TwilioClient trait and implementations for Twilio API operations.
//!
//! This module provides an abstraction over the Twilio API for messaging operations,
//! enabling testability through trait-based design.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::fmt;

/// Credentials for authenticating with Twilio API.
#[derive(Debug, Clone)]
pub struct TwilioCredentials {
    pub account_sid: String,
    pub auth_token: String,
}

impl TwilioCredentials {
    /// Create credentials from environment variables.
    pub fn from_env() -> Result<Self, TwilioClientError> {
        Ok(Self {
            account_sid: env::var("TWILIO_ACCOUNT_SID")
                .map_err(|_| TwilioClientError::MissingCredentials("TWILIO_ACCOUNT_SID not set".into()))?,
            auth_token: env::var("TWILIO_AUTH_TOKEN")
                .map_err(|_| TwilioClientError::MissingCredentials("TWILIO_AUTH_TOKEN not set".into()))?,
        })
    }

    /// Create credentials with explicit values.
    pub fn new(account_sid: String, auth_token: String) -> Self {
        Self {
            account_sid,
            auth_token,
        }
    }
}

/// Result of sending a message via Twilio.
#[derive(Debug, Clone)]
pub struct SendMessageResult {
    /// The Twilio message SID.
    pub message_sid: String,
}

/// Pricing information for a message.
#[derive(Debug, Clone)]
pub struct MessagePrice {
    pub price: Option<String>,
    pub price_unit: Option<String>,
}

/// Errors that can occur when interacting with Twilio.
#[derive(Debug)]
pub enum TwilioClientError {
    /// Missing required credentials.
    MissingCredentials(String),
    /// HTTP request failed.
    RequestFailed(String),
    /// Twilio API returned an error.
    ApiError { status: u16, message: String },
    /// Failed to parse response.
    ParseError(String),
    /// Resource not found (e.g., phone number).
    NotFound(String),
    /// Other error.
    Other(String),
}

impl fmt::Display for TwilioClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TwilioClientError::MissingCredentials(msg) => write!(f, "Missing credentials: {}", msg),
            TwilioClientError::RequestFailed(msg) => write!(f, "Request failed: {}", msg),
            TwilioClientError::ApiError { status, message } => {
                write!(f, "Twilio API error ({}): {}", status, message)
            }
            TwilioClientError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            TwilioClientError::NotFound(msg) => write!(f, "Not found: {}", msg),
            TwilioClientError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl Error for TwilioClientError {}

impl From<reqwest::Error> for TwilioClientError {
    fn from(err: reqwest::Error) -> Self {
        TwilioClientError::RequestFailed(err.to_string())
    }
}

/// Options for sending a message.
#[derive(Debug, Clone, Default)]
pub struct SendMessageOptions {
    /// The destination phone number.
    pub to: String,
    /// The message body.
    pub body: String,
    /// Optional media URL to attach.
    pub media_url: Option<String>,
    /// The "From" phone number. Mutually exclusive with messaging_service_sid.
    pub from: Option<String>,
    /// The messaging service SID. Mutually exclusive with from.
    pub messaging_service_sid: Option<String>,
    /// Optional status callback URL.
    pub status_callback_url: Option<String>,
}

/// Trait defining Twilio API operations.
///
/// This trait abstracts the Twilio API for messaging operations,
/// allowing for easy mocking in tests.
#[async_trait]
pub trait TwilioClient: Send + Sync {
    /// Send an SMS/MMS message.
    async fn send_message(
        &self,
        credentials: &TwilioCredentials,
        options: SendMessageOptions,
    ) -> Result<SendMessageResult, TwilioClientError>;

    /// Delete a message by SID.
    async fn delete_message(
        &self,
        credentials: &TwilioCredentials,
        message_sid: &str,
    ) -> Result<(), TwilioClientError>;

    /// Delete media attached to a message.
    async fn delete_message_media(
        &self,
        credentials: &TwilioCredentials,
        message_sid: &str,
        media_sid: &str,
    ) -> Result<(), TwilioClientError>;

    /// Fetch the price of a sent message.
    async fn fetch_message_price(
        &self,
        credentials: &TwilioCredentials,
        message_sid: &str,
    ) -> Result<Option<MessagePrice>, TwilioClientError>;

    /// Configure the SMS webhook URL for a phone number.
    /// Returns the phone number SID on success.
    async fn configure_webhook(
        &self,
        credentials: &TwilioCredentials,
        phone_number: &str,
        webhook_url: &str,
    ) -> Result<String, TwilioClientError>;
}

/// Real implementation of TwilioClient that makes actual API calls.
pub struct RealTwilioClient {
    http_client: Client,
}

impl RealTwilioClient {
    pub fn new() -> Self {
        Self {
            http_client: Client::new(),
        }
    }
}

impl Default for RealTwilioClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct TwilioMessageResponse {
    sid: String,
}

#[derive(Deserialize)]
struct TwilioMessageDetails {
    price: Option<String>,
    price_unit: Option<String>,
}

#[derive(Deserialize)]
struct PhoneNumbersResponse {
    incoming_phone_numbers: Vec<PhoneNumberInfo>,
}

#[derive(Deserialize)]
struct PhoneNumberInfo {
    sid: String,
}

#[async_trait]
impl TwilioClient for RealTwilioClient {
    async fn send_message(
        &self,
        credentials: &TwilioCredentials,
        options: SendMessageOptions,
    ) -> Result<SendMessageResult, TwilioClientError> {
        let mut form_data: Vec<(&str, String)> = vec![
            ("To", options.to),
            ("Body", options.body),
        ];

        if let Some(from) = options.from {
            form_data.push(("From", from));
        }
        if let Some(sid) = options.messaging_service_sid {
            form_data.push(("MessagingServiceSid", sid));
        }
        if let Some(url) = options.media_url {
            form_data.push(("MediaUrl", url));
        }
        if let Some(callback) = options.status_callback_url {
            form_data.push(("StatusCallback", callback));
        }

        let response = self
            .http_client
            .post(format!(
                "https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json",
                credentials.account_sid
            ))
            .basic_auth(&credentials.account_sid, Some(&credentials.auth_token))
            .form(&form_data)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(TwilioClientError::ApiError {
                status: status.as_u16(),
                message: text,
            });
        }

        let msg_response: TwilioMessageResponse = response
            .json()
            .await
            .map_err(|e| TwilioClientError::ParseError(e.to_string()))?;

        Ok(SendMessageResult {
            message_sid: msg_response.sid,
        })
    }

    async fn delete_message(
        &self,
        credentials: &TwilioCredentials,
        message_sid: &str,
    ) -> Result<(), TwilioClientError> {
        let response = self
            .http_client
            .delete(format!(
                "https://api.twilio.com/2010-04-01/Accounts/{}/Messages/{}.json",
                credentials.account_sid, message_sid
            ))
            .basic_auth(&credentials.account_sid, Some(&credentials.auth_token))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(TwilioClientError::ApiError {
                status: status.as_u16(),
                message: text,
            });
        }

        Ok(())
    }

    async fn delete_message_media(
        &self,
        credentials: &TwilioCredentials,
        message_sid: &str,
        media_sid: &str,
    ) -> Result<(), TwilioClientError> {
        let response = self
            .http_client
            .delete(format!(
                "https://api.twilio.com/2010-04-01/Accounts/{}/Messages/{}/Media/{}.json",
                credentials.account_sid, message_sid, media_sid
            ))
            .basic_auth(&credentials.account_sid, Some(&credentials.auth_token))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(TwilioClientError::ApiError {
                status: status.as_u16(),
                message: text,
            });
        }

        Ok(())
    }

    async fn fetch_message_price(
        &self,
        credentials: &TwilioCredentials,
        message_sid: &str,
    ) -> Result<Option<MessagePrice>, TwilioClientError> {
        let response = self
            .http_client
            .get(format!(
                "https://api.twilio.com/2010-04-01/Accounts/{}/Messages/{}.json",
                credentials.account_sid, message_sid
            ))
            .basic_auth(&credentials.account_sid, Some(&credentials.auth_token))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(TwilioClientError::ApiError {
                status: status.as_u16(),
                message: text,
            });
        }

        let details: TwilioMessageDetails = response
            .json()
            .await
            .map_err(|e| TwilioClientError::ParseError(e.to_string()))?;

        if details.price.is_none() && details.price_unit.is_none() {
            return Ok(None);
        }

        Ok(Some(MessagePrice {
            price: details.price,
            price_unit: details.price_unit,
        }))
    }

    async fn configure_webhook(
        &self,
        credentials: &TwilioCredentials,
        phone_number: &str,
        webhook_url: &str,
    ) -> Result<String, TwilioClientError> {
        // First, find the phone number SID
        let params = [("PhoneNumber", phone_number)];
        let response = self
            .http_client
            .get(format!(
                "https://api.twilio.com/2010-04-01/Accounts/{}/IncomingPhoneNumbers.json",
                credentials.account_sid
            ))
            .basic_auth(&credentials.account_sid, Some(&credentials.auth_token))
            .query(&params)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(TwilioClientError::ApiError {
                status: status.as_u16(),
                message: text,
            });
        }

        let data: PhoneNumbersResponse = response
            .json()
            .await
            .map_err(|e| TwilioClientError::ParseError(e.to_string()))?;

        let phone_sid = data
            .incoming_phone_numbers
            .first()
            .ok_or_else(|| TwilioClientError::NotFound("No matching phone number found".into()))?
            .sid
            .clone();

        // Update the webhook
        let update_params = [("SmsUrl", webhook_url), ("SmsMethod", "POST")];
        let update_response = self
            .http_client
            .post(format!(
                "https://api.twilio.com/2010-04-01/Accounts/{}/IncomingPhoneNumbers/{}.json",
                credentials.account_sid, phone_sid
            ))
            .basic_auth(&credentials.account_sid, Some(&credentials.auth_token))
            .form(&update_params)
            .send()
            .await?;

        let status = update_response.status();
        if !status.is_success() {
            let text = update_response.text().await.unwrap_or_default();
            return Err(TwilioClientError::ApiError {
                status: status.as_u16(),
                message: text,
            });
        }

        Ok(phone_sid)
    }
}

/// Mock implementation of TwilioClient for testing.
pub mod mock {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Debug, Clone, Default)]
    pub struct MockCallRecord {
        pub send_message_calls: Vec<SendMessageOptions>,
        pub delete_message_calls: Vec<String>,
        pub delete_media_calls: Vec<(String, String)>,
        pub fetch_price_calls: Vec<String>,
        pub configure_webhook_calls: Vec<(String, String)>,
    }

    pub struct MockTwilioClient {
        pub calls: Mutex<MockCallRecord>,
        pub send_message_result: Mutex<Result<SendMessageResult, String>>,
        pub delete_message_result: Mutex<Result<(), String>>,
        pub fetch_price_result: Mutex<Result<Option<MessagePrice>, String>>,
        /// Per-message price responses for testing.
        pub price_responses: Mutex<HashMap<String, Option<MessagePrice>>>,
    }

    impl MockTwilioClient {
        pub fn new() -> Self {
            Self {
                calls: Mutex::new(MockCallRecord::default()),
                send_message_result: Mutex::new(Ok(SendMessageResult {
                    message_sid: "SM_mock_sid".to_string(),
                })),
                delete_message_result: Mutex::new(Ok(())),
                fetch_price_result: Mutex::new(Ok(None)),
                price_responses: Mutex::new(HashMap::new()),
            }
        }

        pub fn with_send_result(self, result: Result<SendMessageResult, String>) -> Self {
            *self.send_message_result.lock().unwrap() = result;
            self
        }

        pub fn get_calls(&self) -> MockCallRecord {
            self.calls.lock().unwrap().clone()
        }

        /// Set a specific price response for a message SID.
        pub fn set_price_response(&self, message_sid: &str, price: Option<MessagePrice>) {
            self.price_responses
                .lock()
                .unwrap()
                .insert(message_sid.to_string(), price);
        }

        /// Get the number of times fetch_message_price was called.
        pub fn fetch_price_call_count(&self) -> usize {
            self.calls.lock().unwrap().fetch_price_calls.len()
        }

        /// Check if a specific message was deleted.
        pub fn was_deleted(&self, message_sid: &str) -> bool {
            self.calls
                .lock()
                .unwrap()
                .delete_message_calls
                .contains(&message_sid.to_string())
        }
    }

    impl Default for MockTwilioClient {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl TwilioClient for MockTwilioClient {
        async fn send_message(
            &self,
            _credentials: &TwilioCredentials,
            options: SendMessageOptions,
        ) -> Result<SendMessageResult, TwilioClientError> {
            self.calls.lock().unwrap().send_message_calls.push(options);
            self.send_message_result
                .lock()
                .unwrap()
                .clone()
                .map_err(TwilioClientError::Other)
        }

        async fn delete_message(
            &self,
            _credentials: &TwilioCredentials,
            message_sid: &str,
        ) -> Result<(), TwilioClientError> {
            self.calls
                .lock()
                .unwrap()
                .delete_message_calls
                .push(message_sid.to_string());
            self.delete_message_result
                .lock()
                .unwrap()
                .clone()
                .map_err(TwilioClientError::Other)
        }

        async fn delete_message_media(
            &self,
            _credentials: &TwilioCredentials,
            message_sid: &str,
            media_sid: &str,
        ) -> Result<(), TwilioClientError> {
            self.calls
                .lock()
                .unwrap()
                .delete_media_calls
                .push((message_sid.to_string(), media_sid.to_string()));
            Ok(())
        }

        async fn fetch_message_price(
            &self,
            _credentials: &TwilioCredentials,
            message_sid: &str,
        ) -> Result<Option<MessagePrice>, TwilioClientError> {
            self.calls
                .lock()
                .unwrap()
                .fetch_price_calls
                .push(message_sid.to_string());
            // Check for per-message price response first
            if let Some(price) = self.price_responses.lock().unwrap().get(message_sid) {
                return Ok(price.clone());
            }
            // Fall back to default response
            self.fetch_price_result
                .lock()
                .unwrap()
                .clone()
                .map_err(TwilioClientError::Other)
        }

        async fn configure_webhook(
            &self,
            _credentials: &TwilioCredentials,
            phone_number: &str,
            webhook_url: &str,
        ) -> Result<String, TwilioClientError> {
            self.calls
                .lock()
                .unwrap()
                .configure_webhook_calls
                .push((phone_number.to_string(), webhook_url.to_string()));
            Ok("PN_mock_sid".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::mock::MockTwilioClient;

    #[tokio::test]
    async fn test_mock_send_message() {
        let client = MockTwilioClient::new();
        let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

        let options = SendMessageOptions {
            to: "+1234567890".into(),
            body: "Hello".into(),
            from: Some("+0987654321".into()),
            ..Default::default()
        };

        let result = client.send_message(&creds, options.clone()).await;
        assert!(result.is_ok());

        let calls = client.get_calls();
        assert_eq!(calls.send_message_calls.len(), 1);
        assert_eq!(calls.send_message_calls[0].to, "+1234567890");
    }

    #[tokio::test]
    async fn test_mock_with_error() {
        let client = MockTwilioClient::new()
            .with_send_result(Err("Test error".into()));
        let creds = TwilioCredentials::new("test_sid".into(), "test_token".into());

        let options = SendMessageOptions {
            to: "+1234567890".into(),
            body: "Hello".into(),
            ..Default::default()
        };

        let result = client.send_message(&creds, options).await;
        assert!(result.is_err());
    }
}
