//! Router that owns the registered channels and decides which one handles a
//! given send. Two send paths:
//!
//! - `reply` — outbound goes back through the channel the user reached us on.
//!   Conversation context is preserved per-channel; users never see replies on
//!   a channel they didn't initiate from.
//! - `notify` — proactive outbound. Goes through the user's chosen single
//!   notification channel (resolved by the caller from `user_channels`).
//!
//! Outbound to a user (`send_to_user`) tries an ordered list of channels and
//! falls through to the next one on send failure. The order is sourced from
//! the `provider_routes` table per country, with `preferred_sms_provider` on
//! `users` overriding the per-country choice as the first attempt. Fallback
//! attempts after the first prepend a short tag so the user can tell the
//! message is still legit Lightfriend reaching out on a different carrier.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::channels::traits::{
    ChannelError, ChannelMessageId, IncomingMessage, MediaRef, MessageChannel,
};
use crate::models::user_models::User;

/// Tag prepended to fallback-provider sends so the recipient can tell the
/// SMS is still Lightfriend even though it comes from a different number.
/// Kept short to preserve as much of the GSM-7 segment budget as possible.
pub const FALLBACK_PREFIX: &str = "[Lightfriend backup] ";

pub struct ChannelRouter {
    channels: HashMap<&'static str, Arc<dyn MessageChannel>>,
    /// Per-country ordered provider ids, mirroring the `provider_routes`
    /// table. Held behind RwLock so the admin endpoint can flip a country's
    /// primary at runtime without restarting. Keys are ISO-3166-1 alpha-2
    /// strings (e.g. "US").
    routes: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl ChannelRouter {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            routes: Arc::new(RwLock::new(HashMap::new())),
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

    /// Replace the in-memory routing order for a country. Called once per
    /// row at startup from `main.rs` after loading `provider_routes`, and
    /// again from the admin endpoint when an operator flips the order.
    /// Pass an empty vec or call `clear_route` to revert to default.
    pub fn set_route(&self, country_code: &str, provider_order: Vec<String>) {
        if let Ok(mut routes) = self.routes.write() {
            routes.insert(country_code.to_uppercase(), provider_order);
        }
    }

    pub fn clear_route(&self, country_code: &str) {
        if let Ok(mut routes) = self.routes.write() {
            routes.remove(&country_code.to_uppercase());
        }
    }

    /// Snapshot of the current in-memory routes — for admin diagnostics.
    pub fn current_routes(&self) -> HashMap<String, Vec<String>> {
        self.routes.read().map(|r| r.clone()).unwrap_or_default()
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

    /// Send a message to a user. Tries providers in priority order
    /// (`pick_channels_for`) and falls through to the next on send failure.
    /// The first attempt sends the body unchanged; subsequent attempts
    /// prepend `FALLBACK_PREFIX` so the user can tell that a backup
    /// carrier is reaching them.
    ///
    /// Provider-agnostic preprocessing happens here — every outbound SMS
    /// goes through the same URL/defang sanitizer, the same empty-body
    /// guard, and the same dev-mode skip regardless of which provider
    /// ultimately delivers it.
    ///
    /// Message-history logging is intentionally NOT owned by the router:
    /// callers have richer context about what to log (e.g. citation-
    /// preserving `history_for_storage` vs. user-facing `clean_response`)
    /// and decide for themselves whether and what to write.
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

        // 4. Walk the provider list, falling through to the next on error.
        let order = self.pick_channels_for(user);
        if order.is_empty() {
            return Err(ChannelError::NotConfigured(
                "no SMS providers registered".into(),
            ));
        }

        let total = order.len();
        let mut last_err: Option<ChannelError> = None;
        for (idx, channel_id) in order.iter().enumerate() {
            let chan = match self.channels.get(channel_id) {
                Some(c) => c,
                None => continue,
            };

            let attempt_body = if idx == 0 {
                body.clone()
            } else {
                format!("{}{}", FALLBACK_PREFIX, body)
            };

            match chan
                .send(user, &user.phone_number, &attempt_body, media.clone())
                .await
            {
                Ok(id) => {
                    if idx > 0 {
                        tracing::warn!(
                            "SMS delivered via fallback provider '{}' for user {} (attempt {}/{})",
                            channel_id,
                            user.id,
                            idx + 1,
                            total
                        );
                    }
                    return Ok(id);
                }
                Err(e) => {
                    tracing::warn!(
                        "SMS provider '{}' failed for user {} (attempt {}/{}): {}",
                        channel_id,
                        user.id,
                        idx + 1,
                        total,
                        e
                    );
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| ChannelError::SendFailed("all providers exhausted".into())))
    }

    /// Decide the ordered list of channel ids to try for this user.
    /// Public so callers (tests, admin diagnostics) can introspect.
    ///
    /// Order of precedence:
    /// 1. `user.preferred_sms_provider` (admin pin) — goes first if the
    ///    requested channel is registered. We still append the country
    ///    route after it so a pinned channel that fails can still hand
    ///    off to the country's fallbacks; the pin no longer disables
    ///    resilience.
    /// 2. The country's `provider_routes` row, filtered to registered
    ///    channels and de-duplicated.
    /// 3. Default: `["twilio"]` if it's registered, else any one
    ///    registered channel.
    pub fn pick_channels_for(&self, user: &User) -> Vec<&'static str> {
        let mut order: Vec<&'static str> = Vec::with_capacity(3);
        let push = |v: &mut Vec<&'static str>,
                    id: &'static str,
                    channels: &HashMap<&'static str, Arc<dyn MessageChannel>>| {
            if !v.contains(&id) && channels.contains_key(id) {
                v.push(id);
            }
        };

        if let Some(pref) = user.preferred_sms_provider.as_deref() {
            if let Some(id) = canonical_channel_id(pref) {
                push(&mut order, id, &self.channels);
            }
        }

        let country = crate::utils::country::get_country_code_from_phone(&user.phone_number)
            .unwrap_or_default()
            .to_uppercase();
        if let Ok(routes) = self.routes.read() {
            if let Some(ids) = routes.get(&country) {
                for id in ids {
                    if let Some(static_id) = canonical_channel_id(id) {
                        push(&mut order, static_id, &self.channels);
                    }
                }
            }
        }

        if order.is_empty() {
            push(&mut order, "twilio", &self.channels);
            if order.is_empty() {
                if let Some(first) = self.channels.keys().next() {
                    order.push(*first);
                }
            }
        }

        order
    }

    /// Legacy single-channel accessor. Returns the first channel from
    /// `pick_channels_for`. Kept for callers that only want the primary
    /// (e.g. accountability nudges via `notify`).
    pub fn pick_channel_for(&self, user: &User) -> &'static str {
        self.pick_channels_for(user)
            .into_iter()
            .next()
            .unwrap_or("twilio")
    }
}

impl Default for ChannelRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a user-provided channel id string to the static id used in the
/// registration table. Returns `None` for unknown ids so we don't poison
/// the order with channels we'll never register.
fn canonical_channel_id(s: &str) -> Option<&'static str> {
    match s {
        "twilio" => Some("twilio"),
        "telnyx" => Some("telnyx"),
        "sinch" => Some("sinch"),
        _ => None,
    }
}
