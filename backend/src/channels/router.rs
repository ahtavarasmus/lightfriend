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

    /// Send a message to a user, picking the right channel based on their
    /// phone number's country and which channels are registered. Address
    /// is `user.phone_number` (until `user_channels` lands in a future PR).
    ///
    /// Provider-agnostic preprocessing happens here — every outbound SMS
    /// goes through the same URL/defang sanitizer, the same empty-body
    /// guard, and the same dev-mode skip regardless of which provider
    /// ultimately delivers it.
    ///
    /// Message-history logging is intentionally NOT owned by the router:
    /// callers have richer context about what to log (e.g. citation-
    /// preserving `history_for_storage` vs. user-facing `clean_response`)
    /// and decide for themselves whether and what to write. Doing it here
    /// would clobber that distinction and double-log.
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

        // 3. Skip the actual provider call in development. Returns a stub
        //    id so callers see a Successful result.
        if std::env::var("ENVIRONMENT").unwrap_or_default() == "development" {
            tracing::info!("NOT SENDING MESSAGE SINCE ENVIRONMENT IS DEVELOPMENT");
            return Ok(ChannelMessageId("dev_not_sending".to_string()));
        }

        // 4. Dispatch to the chosen channel.
        let channel_id = self.pick_channel_for(user);
        let chan = self
            .channels
            .get(channel_id)
            .ok_or_else(|| ChannelError::NotConfigured(channel_id.to_string()))?;
        chan.send(user, &user.phone_number, &body, media).await
    }

    /// Decide which channel id handles outbound for this user. Public so
    /// callers (e.g. tests, admin diagnostics) can introspect the choice.
    ///
    /// `user.preferred_sms_provider` is an explicit override and wins
    /// over country-based routing — but only if the requested channel
    /// is actually registered. If the operator pinned a user to "sinch"
    /// then later tore down the Sinch credentials, we silently fall
    /// back to country-based selection rather than failing the send.
    pub fn pick_channel_for(&self, user: &User) -> &'static str {
        if let Some(pref) = user.preferred_sms_provider.as_deref() {
            let pref_static: Option<&'static str> = match pref {
                "twilio" => Some("twilio"),
                "telnyx" => Some("telnyx"),
                "sinch" => Some("sinch"),
                _ => None,
            };
            if let Some(id) = pref_static {
                if self.channels.contains_key(id) {
                    return id;
                }
            }
        }
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
