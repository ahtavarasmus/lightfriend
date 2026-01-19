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
            account_sid: env::var("TWILIO_ACCOUNT_SID").map_err(|_| {
                TwilioClientError::MissingCredentials("TWILIO_ACCOUNT_SID not set".into())
            })?,
            auth_token: env::var("TWILIO_AUTH_TOKEN").map_err(|_| {
                TwilioClientError::MissingCredentials("TWILIO_AUTH_TOKEN not set".into())
            })?,
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

/// Messaging pricing information for a country.
#[derive(Debug, Clone, Default)]
pub struct MessagingPricingResult {
    pub outbound_sms_price: Option<f32>,
    pub inbound_sms_price: Option<f32>,
}

/// Voice pricing information for a country.
#[derive(Debug, Clone, Default)]
pub struct VoicePricingResult {
    pub outbound_price_per_min: Option<f32>,
    pub inbound_price_per_min: Option<f32>,
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

    /// Check if phone numbers are available for a country.
    /// Returns true if either Mobile or Local numbers are available.
    async fn check_phone_numbers_available(
        &self,
        credentials: &TwilioCredentials,
        country_code: &str,
    ) -> Result<bool, TwilioClientError>;

    /// Get messaging (SMS) pricing for a country.
    async fn get_messaging_pricing(
        &self,
        credentials: &TwilioCredentials,
        country_code: &str,
    ) -> Result<MessagingPricingResult, TwilioClientError>;

    /// Get voice pricing for a country.
    /// If use_v2 is true, uses the v2 API for origin-based (domestic) pricing.
    async fn get_voice_pricing(
        &self,
        credentials: &TwilioCredentials,
        country_code: &str,
        use_v2: bool,
    ) -> Result<VoicePricingResult, TwilioClientError>;
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

// Response types for availability/pricing APIs

#[derive(Deserialize)]
struct AvailablePhoneNumbersResponse {
    #[serde(default)]
    available_phone_numbers: Vec<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
struct MessagingPricingResponse {
    #[serde(default)]
    inbound_sms_prices: Vec<InboundSmsPrice>,
    #[serde(default)]
    outbound_sms_prices: Vec<OutboundSmsPrice>,
}

#[derive(Deserialize, Debug)]
struct InboundSmsPrice {
    #[serde(default)]
    prices: Vec<PriceItem>,
}

#[derive(Deserialize, Debug)]
struct OutboundSmsPrice {
    #[serde(default)]
    prices: Vec<PriceItem>,
}

#[derive(Deserialize, Debug)]
struct PriceItem {
    current_price: String,
}

#[derive(Deserialize, Debug)]
struct VoicePricingResponse {
    #[serde(default)]
    inbound_call_prices: Vec<InboundCallPrice>,
    #[serde(default)]
    outbound_prefix_prices: Vec<OutboundPrefixPrice>,
}

#[derive(Deserialize, Debug)]
struct InboundCallPrice {
    current_price: String,
}

#[derive(Deserialize, Debug)]
struct OutboundPrefixPrice {
    current_price: String,
}

/// Response from Twilio Voice Pricing v2 API (origin-based pricing)
#[derive(Deserialize, Debug)]
struct OriginBasedVoicePricingResponse {
    #[serde(default)]
    originating_call_prices: Vec<OriginatingCallPrice>,
    #[serde(default)]
    terminating_prefix_prices: Vec<TerminatingPrefixPrice>,
}

#[derive(Deserialize, Debug)]
struct OriginatingCallPrice {
    current_price: String,
}

#[derive(Deserialize, Debug)]
struct TerminatingPrefixPrice {
    current_price: String,
}

#[async_trait]
impl TwilioClient for RealTwilioClient {
    async fn send_message(
        &self,
        credentials: &TwilioCredentials,
        options: SendMessageOptions,
    ) -> Result<SendMessageResult, TwilioClientError> {
        let mut form_data: Vec<(&str, String)> = vec![("To", options.to), ("Body", options.body)];

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

    async fn check_phone_numbers_available(
        &self,
        credentials: &TwilioCredentials,
        country_code: &str,
    ) -> Result<bool, TwilioClientError> {
        let country_upper = country_code.to_uppercase();

        // Check both Mobile and Local numbers
        for number_type in &["Mobile", "Local"] {
            let url = format!(
                "https://api.twilio.com/2010-04-01/Accounts/{}/AvailablePhoneNumbers/{}/{}.json",
                credentials.account_sid, country_upper, number_type
            );

            let response = self
                .http_client
                .get(&url)
                .basic_auth(&credentials.account_sid, Some(&credentials.auth_token))
                .query(&[("PageSize", "1")])
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(data) = resp.json::<AvailablePhoneNumbersResponse>().await {
                        if !data.available_phone_numbers.is_empty() {
                            return Ok(true);
                        }
                    }
                }
                Ok(resp) if resp.status().as_u16() == 404 => {
                    // Country not supported for this number type, continue
                }
                _ => {}
            }
        }

        Ok(false)
    }

    async fn get_messaging_pricing(
        &self,
        credentials: &TwilioCredentials,
        country_code: &str,
    ) -> Result<MessagingPricingResult, TwilioClientError> {
        let url = format!(
            "https://pricing.twilio.com/v1/Messaging/Countries/{}",
            country_code.to_uppercase()
        );

        let response = self
            .http_client
            .get(&url)
            .basic_auth(&credentials.account_sid, Some(&credentials.auth_token))
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<MessagingPricingResponse>().await {
                    Ok(pricing) => {
                        let outbound = pricing
                            .outbound_sms_prices
                            .first()
                            .and_then(|p| p.prices.first())
                            .and_then(|p| p.current_price.parse::<f32>().ok());

                        let inbound = pricing
                            .inbound_sms_prices
                            .first()
                            .and_then(|p| p.prices.first())
                            .and_then(|p| p.current_price.parse::<f32>().ok());

                        Ok(MessagingPricingResult {
                            outbound_sms_price: outbound,
                            inbound_sms_price: inbound,
                        })
                    }
                    Err(_) => Ok(MessagingPricingResult::default()),
                }
            }
            Ok(_) | Err(_) => Ok(MessagingPricingResult::default()),
        }
    }

    async fn get_voice_pricing(
        &self,
        credentials: &TwilioCredentials,
        country_code: &str,
        use_v2: bool,
    ) -> Result<VoicePricingResult, TwilioClientError> {
        let country_upper = country_code.to_uppercase();

        if use_v2 {
            // Use v2 API for origin-based (domestic) pricing
            let url = format!(
                "https://pricing.twilio.com/v2/Voice/Countries/{}",
                country_upper
            );

            let response = self
                .http_client
                .get(&url)
                .basic_auth(&credentials.account_sid, Some(&credentials.auth_token))
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<OriginBasedVoicePricingResponse>().await {
                        Ok(pricing) => {
                            // Outbound: terminating_prefix_prices (calling TO the country FROM the country)
                            let outbound = pricing
                                .terminating_prefix_prices
                                .first()
                                .and_then(|p| p.current_price.parse::<f32>().ok());

                            // Inbound: originating_call_prices (receiving calls on a number IN the country)
                            let inbound = pricing
                                .originating_call_prices
                                .first()
                                .and_then(|p| p.current_price.parse::<f32>().ok());

                            Ok(VoicePricingResult {
                                outbound_price_per_min: outbound,
                                inbound_price_per_min: inbound,
                            })
                        }
                        Err(_) => Ok(VoicePricingResult::default()),
                    }
                }
                Ok(_) | Err(_) => Ok(VoicePricingResult::default()),
            }
        } else {
            // Use v1 API for standard pricing
            let url = format!(
                "https://pricing.twilio.com/v1/Voice/Countries/{}",
                country_upper
            );

            let response = self
                .http_client
                .get(&url)
                .basic_auth(&credentials.account_sid, Some(&credentials.auth_token))
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<VoicePricingResponse>().await {
                        Ok(pricing) => {
                            let outbound = pricing
                                .outbound_prefix_prices
                                .first()
                                .and_then(|p| p.current_price.parse::<f32>().ok());

                            let inbound = pricing
                                .inbound_call_prices
                                .first()
                                .and_then(|p| p.current_price.parse::<f32>().ok());

                            Ok(VoicePricingResult {
                                outbound_price_per_min: outbound,
                                inbound_price_per_min: inbound,
                            })
                        }
                        Err(_) => Ok(VoicePricingResult::default()),
                    }
                }
                Ok(_) | Err(_) => Ok(VoicePricingResult::default()),
            }
        }
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
        pub check_phone_numbers_calls: Vec<String>,
        pub get_messaging_pricing_calls: Vec<String>,
        pub get_voice_pricing_calls: Vec<(String, bool)>,
    }

    pub struct MockTwilioClient {
        pub calls: Mutex<MockCallRecord>,
        pub send_message_result: Mutex<Result<SendMessageResult, String>>,
        pub delete_message_result: Mutex<Result<(), String>>,
        pub delete_media_result: Mutex<Result<(), String>>,
        pub fetch_price_result: Mutex<Result<Option<MessagePrice>, String>>,
        pub configure_webhook_result: Mutex<Result<String, String>>,
        /// Per-message price responses for testing.
        pub price_responses: Mutex<HashMap<String, Option<MessagePrice>>>,
        /// Response for check_phone_numbers_available calls.
        pub phone_numbers_available_result: Mutex<bool>,
        /// Per-country messaging pricing responses.
        pub messaging_pricing_responses: Mutex<HashMap<String, MessagingPricingResult>>,
        /// Per-country voice pricing responses.
        pub voice_pricing_responses: Mutex<HashMap<String, VoicePricingResult>>,
    }

    impl MockTwilioClient {
        pub fn new() -> Self {
            Self {
                calls: Mutex::new(MockCallRecord::default()),
                send_message_result: Mutex::new(Ok(SendMessageResult {
                    message_sid: "SM_mock_sid".to_string(),
                })),
                delete_message_result: Mutex::new(Ok(())),
                delete_media_result: Mutex::new(Ok(())),
                fetch_price_result: Mutex::new(Ok(None)),
                configure_webhook_result: Mutex::new(Ok("PN_mock_sid".to_string())),
                price_responses: Mutex::new(HashMap::new()),
                phone_numbers_available_result: Mutex::new(false),
                messaging_pricing_responses: Mutex::new(HashMap::new()),
                voice_pricing_responses: Mutex::new(HashMap::new()),
            }
        }

        pub fn with_send_result(self, result: Result<SendMessageResult, String>) -> Self {
            *self.send_message_result.lock().unwrap() = result;
            self
        }

        /// Configure the mock to return an error for delete_message calls.
        pub fn with_delete_message_error(self, error: String) -> Self {
            *self.delete_message_result.lock().unwrap() = Err(error);
            self
        }

        /// Configure the mock to return an error for delete_message_media calls.
        pub fn with_delete_media_error(self, error: String) -> Self {
            *self.delete_media_result.lock().unwrap() = Err(error);
            self
        }

        /// Configure the mock to return an error for fetch_message_price calls.
        pub fn with_fetch_price_error(self, error: String) -> Self {
            *self.fetch_price_result.lock().unwrap() = Err(error);
            self
        }

        /// Configure the mock to return an error for configure_webhook calls.
        pub fn with_configure_webhook_error(self, error: String) -> Self {
            *self.configure_webhook_result.lock().unwrap() = Err(error);
            self
        }

        /// Configure phone numbers availability for testing.
        pub fn with_phone_numbers_available(self, available: bool) -> Self {
            *self.phone_numbers_available_result.lock().unwrap() = available;
            self
        }

        /// Set messaging pricing for a specific country.
        pub fn with_messaging_pricing(
            self,
            country: &str,
            pricing: MessagingPricingResult,
        ) -> Self {
            self.messaging_pricing_responses
                .lock()
                .unwrap()
                .insert(country.to_string(), pricing);
            self
        }

        /// Set voice pricing for a specific country.
        pub fn with_voice_pricing(self, country: &str, pricing: VoicePricingResult) -> Self {
            self.voice_pricing_responses
                .lock()
                .unwrap()
                .insert(country.to_string(), pricing);
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

        /// Check if media was deleted for a specific message.
        pub fn was_media_deleted(&self, message_sid: &str, media_sid: &str) -> bool {
            self.calls
                .lock()
                .unwrap()
                .delete_media_calls
                .contains(&(message_sid.to_string(), media_sid.to_string()))
        }

        /// Get the number of times send_message was called.
        pub fn send_message_call_count(&self) -> usize {
            self.calls.lock().unwrap().send_message_calls.len()
        }

        /// Get the number of times delete_message was called.
        pub fn delete_message_call_count(&self) -> usize {
            self.calls.lock().unwrap().delete_message_calls.len()
        }

        /// Get the number of times configure_webhook was called.
        pub fn configure_webhook_call_count(&self) -> usize {
            self.calls.lock().unwrap().configure_webhook_calls.len()
        }

        /// Clear all recorded calls (useful for resetting between test phases).
        pub fn clear_calls(&self) {
            *self.calls.lock().unwrap() = MockCallRecord::default();
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
            self.delete_media_result
                .lock()
                .unwrap()
                .clone()
                .map_err(TwilioClientError::Other)
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
            self.configure_webhook_result
                .lock()
                .unwrap()
                .clone()
                .map_err(TwilioClientError::Other)
        }

        async fn check_phone_numbers_available(
            &self,
            _credentials: &TwilioCredentials,
            country_code: &str,
        ) -> Result<bool, TwilioClientError> {
            self.calls
                .lock()
                .unwrap()
                .check_phone_numbers_calls
                .push(country_code.to_string());
            Ok(*self.phone_numbers_available_result.lock().unwrap())
        }

        async fn get_messaging_pricing(
            &self,
            _credentials: &TwilioCredentials,
            country_code: &str,
        ) -> Result<MessagingPricingResult, TwilioClientError> {
            self.calls
                .lock()
                .unwrap()
                .get_messaging_pricing_calls
                .push(country_code.to_string());
            Ok(self
                .messaging_pricing_responses
                .lock()
                .unwrap()
                .get(country_code)
                .cloned()
                .unwrap_or_default())
        }

        async fn get_voice_pricing(
            &self,
            _credentials: &TwilioCredentials,
            country_code: &str,
            use_v2: bool,
        ) -> Result<VoicePricingResult, TwilioClientError> {
            self.calls
                .lock()
                .unwrap()
                .get_voice_pricing_calls
                .push((country_code.to_string(), use_v2));
            Ok(self
                .voice_pricing_responses
                .lock()
                .unwrap()
                .get(country_code)
                .cloned()
                .unwrap_or_default())
        }
    }
}
