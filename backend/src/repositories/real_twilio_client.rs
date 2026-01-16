//! Real implementation of TwilioClient using reqwest.
//!
//! This implementation makes actual HTTP calls to the Twilio API.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

use crate::repositories::twilio_client::{MessagePrice, TwilioClient, TwilioClientError};

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
