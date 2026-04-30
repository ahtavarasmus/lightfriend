//! Channel traits — the seam between core application logic and any specific
//! delivery transport (Twilio SMS, Telnyx SMS, Sinch SMS, Matrix, push, Light
//! Phone, Beepy, …).
//!
//! Design goals:
//! - Keep the trait small. The core application calls `send` and that's it.
//! - Channels are pluggable via the `ChannelRouter`. Adding a new transport
//!   means writing one impl + one webhook adapter, not editing core paths.
//! - Channel-specific quirks (BYOT credentials, status callbacks, delete-
//!   after-send for privacy) stay inside the impl. The trait stays minimal.
//! - Voice is a separate trait. Channels that don't speak voice simply don't
//!   implement it.

use async_trait::async_trait;
use thiserror::Error;

use crate::models::user_models::User;

/// Errors any channel can report when sending a message.
#[derive(Debug, Error)]
pub enum ChannelError {
    #[error("channel not configured: {0}")]
    NotConfigured(String),

    #[error("invalid address for channel: {0}")]
    InvalidAddress(String),

    #[error("media not supported on this channel")]
    MediaNotSupported,

    #[error("send failed: {0}")]
    SendFailed(String),

    #[error("other: {0}")]
    Other(String),
}

/// A reference to media to be sent. The channel impl resolves this however
/// it likes — Twilio looks up its own media SID, Matrix uploads bytes, push
/// channels embed a URL or strip media entirely.
#[derive(Debug, Clone)]
pub enum MediaRef {
    Url(String),
    Bytes { data: Vec<u8>, mime: String },
}

/// Provider-assigned identifier for a sent or received message.
/// Twilio SID, Telnyx message id, Matrix event id, push notification id, …
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelMessageId(pub String);

impl ChannelMessageId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<ChannelMessageId> for String {
    fn from(id: ChannelMessageId) -> Self {
        id.0
    }
}

impl std::fmt::Display for ChannelMessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Inbound message normalised across channels. Each channel's webhook adapter
/// produces this and hands it to `process_inbound`.
#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub channel_id: &'static str,
    pub address: String,
    pub body: String,
    pub media_url: Option<String>,
    pub external_id: Option<String>,
    pub user_id: i32,
}

/// One outbound channel for sending text and (optionally) media.
///
/// Object-safe: register as `Arc<dyn MessageChannel>`. Async via async-trait.
#[async_trait]
pub trait MessageChannel: Send + Sync {
    /// Stable id for this channel. Used to look it up in the router and to
    /// tag rows in the `user_channels` table. Convention: lowercase kebab.
    fn id(&self) -> &'static str;

    /// Send a message to `address` (phone number, MXID, push token, …).
    /// The `user` is included for accounting and personalised credentials
    /// (e.g. BYOT Twilio), not for address resolution.
    async fn send(
        &self,
        user: &User,
        address: &str,
        body: &str,
        media: Option<MediaRef>,
    ) -> Result<ChannelMessageId, ChannelError>;

    fn supports_media(&self) -> bool {
        false
    }

    fn supports_delete(&self) -> bool {
        false
    }

    /// Delete a previously sent message at the provider. Default is a no-op.
    /// Twilio overrides this for privacy-after-delivery.
    async fn delete(&self, _user: &User, _msg_id: &ChannelMessageId) -> Result<(), ChannelError> {
        Ok(())
    }
}

/// Optional voice support. Implement only on channels that can place calls.
#[async_trait]
pub trait VoiceChannel: Send + Sync {
    fn id(&self) -> &'static str;

    async fn place_call(
        &self,
        user: &User,
        address: &str,
        greeting: &str,
    ) -> Result<ChannelMessageId, ChannelError>;
}
