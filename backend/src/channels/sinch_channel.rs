//! Sinch SMS REST API channel ("XMS"). Direct REST impl, env-configured.
//! `from_env()` returns `None` if any required var is absent so wiring is
//! "set the env vars and restart" — no code change.
//!
//! Auth: Bearer token. Send: `POST /xms/v1/{service_plan_id}/batches` with
//! JSON. Region (US/EU) selects the API base URL. Status callbacks are
//! configured per-service-plan in the Sinch dashboard, pointing at our
//! `/api/sinch/status` webhook (added in a follow-up).

use async_trait::async_trait;

use crate::channels::traits::{ChannelError, ChannelMessageId, MediaRef, MessageChannel};
use crate::models::user_models::User;

pub struct SinchChannel {
    api_token: String,
    service_plan_id: String,
    base_url: String,
    from_number: String,
    http: reqwest::Client,
}

impl SinchChannel {
    pub fn from_env() -> Option<Self> {
        let api_token = std::env::var("SINCH_API_TOKEN")
            .ok()
            .filter(|s| !s.is_empty())?;
        let service_plan_id = std::env::var("SINCH_SERVICE_PLAN_ID")
            .ok()
            .filter(|s| !s.is_empty())?;
        let from_number = std::env::var("SINCH_US_FROM_NUMBER")
            .ok()
            .filter(|s| !s.is_empty())?;
        let region = std::env::var("SINCH_REGION").unwrap_or_else(|_| "us".to_string());
        let base_url = match region.as_str() {
            "eu" => "https://eu.sms.api.sinch.com",
            _ => "https://us.sms.api.sinch.com",
        }
        .to_string();
        Some(Self {
            api_token,
            service_plan_id,
            base_url,
            from_number,
            http: reqwest::Client::new(),
        })
    }

    /// Construct with an explicit base URL. Used by tests to point at wiremock.
    pub fn with_base_url(
        api_token: impl Into<String>,
        service_plan_id: impl Into<String>,
        from_number: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            api_token: api_token.into(),
            service_plan_id: service_plan_id.into(),
            base_url: base_url.into(),
            from_number: from_number.into(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl MessageChannel for SinchChannel {
    fn id(&self) -> &'static str {
        "sinch"
    }

    async fn send(
        &self,
        _user: &User,
        address: &str,
        body: &str,
        media: Option<MediaRef>,
    ) -> Result<ChannelMessageId, ChannelError> {
        if media.is_some() {
            // Sinch supports MMS via separate batch type; deferred until needed.
            return Err(ChannelError::MediaNotSupported);
        }

        let url = format!("{}/xms/v1/{}/batches", self.base_url, self.service_plan_id);
        let payload = serde_json::json!({
            "from": self.from_number,
            "to": [address],
            "body": body,
        });

        let res = self
            .http
            .post(&url)
            .bearer_auth(&self.api_token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!(
                "sinch http {}: {}",
                status, body
            )));
        }

        #[derive(serde::Deserialize)]
        struct R {
            id: String,
        }

        let parsed: R = res
            .json()
            .await
            .map_err(|e| ChannelError::SendFailed(format!("sinch parse: {}", e)))?;
        Ok(ChannelMessageId(parsed.id))
    }
}
