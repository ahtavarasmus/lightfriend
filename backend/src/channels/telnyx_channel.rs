//! Telnyx SMS channel — direct REST impl, no shared HTTP client glue beyond
//! `reqwest`. Configured via env vars; if any required var is missing,
//! `from_env` returns `None` and the channel simply isn't registered.
//!
//! Auth: Bearer token. Send: `POST /v2/messages` with JSON. Status callbacks
//! are configured at the messaging-profile level in the Telnyx dashboard,
//! pointing at our `/api/telnyx/status` webhook (added in a follow-up).

use async_trait::async_trait;

use crate::channels::traits::{ChannelError, ChannelMessageId, MediaRef, MessageChannel};
use crate::models::user_models::User;

pub struct TelnyxChannel {
    api_key: String,
    profile_id: String,
    from_number: String,
    base_url: String,
    http: reqwest::Client,
}

const TELNYX_DEFAULT_BASE: &str = "https://api.telnyx.com";

impl TelnyxChannel {
    /// Build from env. Returns `None` if any required var is absent or empty,
    /// so wiring this up is "set the env vars and restart" — no code change.
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("TELNYX_API_KEY")
            .ok()
            .filter(|s| !s.is_empty())?;
        let profile_id = std::env::var("TELNYX_MESSAGING_PROFILE_ID")
            .ok()
            .filter(|s| !s.is_empty())?;
        let from_number = std::env::var("TELNYX_US_FROM_NUMBER")
            .ok()
            .filter(|s| !s.is_empty())?;
        Some(Self {
            api_key,
            profile_id,
            from_number,
            base_url: TELNYX_DEFAULT_BASE.to_string(),
            http: reqwest::Client::new(),
        })
    }

    /// Construct with an explicit base URL. Used by tests to point at wiremock.
    pub fn with_base_url(
        api_key: impl Into<String>,
        profile_id: impl Into<String>,
        from_number: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            profile_id: profile_id.into(),
            from_number: from_number.into(),
            base_url: base_url.into(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl MessageChannel for TelnyxChannel {
    fn id(&self) -> &'static str {
        "telnyx"
    }

    async fn send(
        &self,
        _user: &User,
        address: &str,
        body: &str,
        media: Option<MediaRef>,
    ) -> Result<ChannelMessageId, ChannelError> {
        let mut payload = serde_json::json!({
            "from": self.from_number,
            "to": address,
            "text": body,
            "messaging_profile_id": self.profile_id,
        });

        if let Some(m) = media {
            match m {
                MediaRef::Url(url) => {
                    payload["media_urls"] = serde_json::json!([url]);
                }
                MediaRef::Bytes { .. } => return Err(ChannelError::MediaNotSupported),
            }
        }

        let res = self
            .http
            .post(format!("{}/v2/messages", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!(
                "telnyx http {}: {}",
                status, body
            )));
        }

        #[derive(serde::Deserialize)]
        struct R {
            data: D,
        }
        #[derive(serde::Deserialize)]
        struct D {
            id: String,
        }

        let parsed: R = res
            .json()
            .await
            .map_err(|e| ChannelError::SendFailed(format!("telnyx parse: {}", e)))?;
        Ok(ChannelMessageId(parsed.data.id))
    }

    fn supports_media(&self) -> bool {
        true
    }
}
