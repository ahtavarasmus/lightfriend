//! Router that owns the registered channels and decides which one handles a
//! given send. Two send paths:
//!
//! - `reply` — outbound goes back through the channel the user reached us on.
//!   Conversation context is preserved per-channel; users never see replies on
//!   a channel they didn't initiate from.
//! - `notify` — proactive outbound. Goes through the user's chosen single
//!   notification channel (resolved by the caller from `user_channels`).
//!
//! The router has no knowledge of which channel is "primary" — it just looks
//! up by `channel_id`. Routing policy lives in the caller (or in the
//! `user_channels` table once we add it).

use std::collections::HashMap;
use std::sync::Arc;

use crate::channels::traits::{
    ChannelError, ChannelMessageId, IncomingMessage, MediaRef, MessageChannel,
};
use crate::models::user_models::User;

pub struct ChannelRouter {
    channels: HashMap<&'static str, Arc<dyn MessageChannel>>,
}

impl ChannelRouter {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    /// Register a channel implementation. Last write wins per `id`.
    pub fn register(&mut self, ch: Arc<dyn MessageChannel>) {
        self.channels.insert(ch.id(), ch);
    }

    /// Look up a channel impl by id.
    pub fn channel(&self, id: &str) -> Option<&Arc<dyn MessageChannel>> {
        self.channels.get(id)
    }

    /// Reply through whichever channel the user reached us on.
    pub async fn reply(
        &self,
        user: &User,
        source: &IncomingMessage,
        body: &str,
        media: Option<MediaRef>,
    ) -> Result<ChannelMessageId, ChannelError> {
        let chan = self
            .channels
            .get(source.channel_id)
            .ok_or_else(|| ChannelError::NotConfigured(source.channel_id.to_string()))?;
        chan.send(user, &source.address, body, media).await
    }

    /// Notify the user via an explicit channel + address. The caller is
    /// responsible for resolving the user's notification preference (the
    /// router intentionally does not read user state).
    pub async fn notify(
        &self,
        user: &User,
        channel_id: &str,
        address: &str,
        body: &str,
    ) -> Result<ChannelMessageId, ChannelError> {
        let chan = self
            .channels
            .get(channel_id)
            .ok_or_else(|| ChannelError::NotConfigured(channel_id.to_string()))?;
        chan.send(user, address, body, None).await
    }
}

impl Default for ChannelRouter {
    fn default() -> Self {
        Self::new()
    }
}
