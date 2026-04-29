//! Twilio adapter — exposes the existing `TwilioMessageService` as a
//! `MessageChannel` without rewriting it.
//!
//! All current behaviour is preserved: URL filtering, empty-body guard,
//! preferred-number resolution, BYOT credentials, status callbacks, voice
//! fallback. This file is a seam, not a rewrite.
//!
//! The `address` argument is intentionally ignored for now — the existing
//! service reads `user.phone_number` internally. Once `user_channels` lands,
//! we'll route the address through.

use std::sync::Arc;

use async_trait::async_trait;

use crate::api::twilio_client::TwilioClient;
use crate::channels::traits::{ChannelError, ChannelMessageId, MediaRef, MessageChannel};
use crate::models::user_models::User;
use crate::services::twilio_message_service::{TwilioMessageError, TwilioMessageService};

pub struct TwilioChannel<T: TwilioClient> {
    inner: Arc<TwilioMessageService<T>>,
}

impl<T: TwilioClient> TwilioChannel<T> {
    pub fn new(inner: Arc<TwilioMessageService<T>>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<T> MessageChannel for TwilioChannel<T>
where
    T: TwilioClient + 'static,
{
    fn id(&self) -> &'static str {
        "twilio"
    }

    async fn send(
        &self,
        user: &User,
        _address: &str,
        body: &str,
        media: Option<MediaRef>,
    ) -> Result<ChannelMessageId, ChannelError> {
        let media_sid = match media {
            None => None,
            Some(MediaRef::Url(url)) => Some(url),
            Some(MediaRef::Bytes { .. }) => return Err(ChannelError::MediaNotSupported),
        };
        let media_ref = media_sid.as_ref();

        match self.inner.send_sms(body, media_ref, user).await {
            Ok(sid) => Ok(ChannelMessageId(sid)),
            Err(TwilioMessageError::EmptyMessage) => {
                Err(ChannelError::SendFailed("empty body".into()))
            }
            Err(e) => Err(ChannelError::SendFailed(e.to_string())),
        }
    }

    fn supports_media(&self) -> bool {
        true
    }

    fn supports_delete(&self) -> bool {
        true
    }

    async fn delete(&self, user: &User, msg_id: &ChannelMessageId) -> Result<(), ChannelError> {
        self.inner
            .delete_message_with_retry(user, &msg_id.0)
            .await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))
    }
}
