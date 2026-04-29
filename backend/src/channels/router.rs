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
use crate::repositories::user_repository::UserRepository;

pub struct ChannelRouter {
    channels: HashMap<&'static str, Arc<dyn MessageChannel>>,
    /// Used by the provider-agnostic preprocessing layer to log every
    /// outbound message to the user's chat history. Optional so unit tests
    /// can construct a minimal router without the full DB stack.
    user_repository: Option<Arc<UserRepository>>,
}

impl ChannelRouter {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            user_repository: None,
        }
    }

    /// Construct a router with an attached repository so provider-agnostic
    /// message-history logging works. Used in production wiring; tests
    /// generally use `new()` and skip history logging.
    pub fn with_user_repository(repo: Arc<UserRepository>) -> Self {
        Self {
            channels: HashMap::new(),
            user_repository: Some(repo),
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

    /// Send a message to a user, picking the right channel based on their
    /// phone number's country and which channels are registered. Address
    /// is `user.phone_number` (until `user_channels` lands in a future PR).
    ///
    /// Provider-agnostic preprocessing happens here — every outbound SMS
    /// goes through the same URL/defang sanitizer, the same empty-body
    /// guard, the same dev-mode skip, and the same message-history write,
    /// regardless of which provider ultimately delivers it.
    ///
    /// Routing policy (presence-based, no env reads here — channels register
    /// only if their env config is complete):
    /// - US users: prefer `telnyx`, then `sinch`, then `twilio`
    /// - All other users: `twilio`
    ///
    /// Set `TELNYX_API_KEY` / `SINCH_API_TOKEN` (and friends) to flip US
    /// routing without code changes. Unset to revert.
    pub async fn send_to_user(
        &self,
        user: &User,
        body: &str,
        media: Option<MediaRef>,
    ) -> Result<ChannelMessageId, ChannelError> {
        // 1. Sanitize URLs + re-fang defanged emails/domains so phishing-
        //    pattern false positives don't trip carrier filters.
        let body = crate::utils::sms_sanitizer::apply_sms_url_filter(body);

        // 2. Refuse empty messages. Sending a body-less request to a provider
        //    is malformed (Twilio errors with 21619) and contributes to
        //    abnormal-traffic patterns in anti-abuse heuristics.
        if body.trim().is_empty() && media.is_none() {
            tracing::error!(
                "Refused to send empty SMS to user {} (no body and no media)",
                user.id
            );
            return Err(ChannelError::SendFailed("empty body".into()));
        }

        // 3. Log to message history regardless of provider. This is the
        //    user's chat-history record — not a per-provider concern.
        if let Some(repo) = &self.user_repository {
            let entry = crate::pg_models::NewPgMessageHistory {
                user_id: user.id,
                role: "assistant".to_string(),
                encrypted_content: body.clone(),
                tool_name: None,
                tool_call_id: None,
                tool_calls_json: None,
                created_at: chrono::Utc::now().timestamp() as i32,
                conversation_id: "".to_string(),
            };
            if let Err(e) = repo.create_message_history(&entry) {
                tracing::error!("Failed to store message in history: {}", e);
            }
        }

        // 4. Skip the actual provider call in development. Returns a stub
        //    id so callers see a Successful result.
        if std::env::var("ENVIRONMENT").unwrap_or_default() == "development" {
            tracing::info!("NOT SENDING MESSAGE SINCE ENVIRONMENT IS DEVELOPMENT");
            return Ok(ChannelMessageId("dev_not_sending".to_string()));
        }

        // 5. Dispatch to the chosen channel.
        let channel_id = self.pick_channel_for(user);
        let chan = self
            .channels
            .get(channel_id)
            .ok_or_else(|| ChannelError::NotConfigured(channel_id.to_string()))?;
        chan.send(user, &user.phone_number, &body, media).await
    }

    /// Decide which channel id handles outbound for this user. Public so
    /// callers (e.g. tests, admin diagnostics) can introspect the choice.
    pub fn pick_channel_for(&self, user: &User) -> &'static str {
        let country = crate::utils::country::get_country_code_from_phone(&user.phone_number);
        if country.as_deref() == Some("US") {
            if self.channels.contains_key("telnyx") {
                return "telnyx";
            }
            if self.channels.contains_key("sinch") {
                return "sinch";
            }
        }
        "twilio"
    }
}

impl Default for ChannelRouter {
    fn default() -> Self {
        Self::new()
    }
}
