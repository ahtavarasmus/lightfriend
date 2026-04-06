//! Matrix client abstraction layer for testability.
//!
//! This module provides trait abstractions over the Matrix SDK client to enable
//! unit testing of bridge functionality without requiring a real Matrix server.

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

// ============================================================================
// Data Transfer Objects
// ============================================================================

/// Basic room information returned from get_joined_rooms
#[derive(Debug, Clone)]
pub struct RoomInfo {
    pub room_id: String,
    pub display_name: String,
}

/// Abstraction of an incoming Matrix message event
#[derive(Debug, Clone, Default)]
pub struct IncomingBridgeEvent {
    pub event_id: String,
    /// Full Matrix user ID (e.g., @user:server.com)
    pub sender: String,
    /// Just the localpart (before the colon)
    pub sender_localpart: String,
    /// Message timestamp in milliseconds since Unix epoch
    pub timestamp_ms: u64,
    /// The message content
    pub content: IncomingMessageContent,
    /// User IDs mentioned in this message
    pub mentions: Option<Vec<String>>,
}

/// Types of incoming message content
#[derive(Debug, Clone, Default)]
pub enum IncomingMessageContent {
    Text {
        body: String,
    },
    Notice {
        body: String,
    },
    Image {
        body: String,
        url: Option<String>,
    },
    Video {
        body: String,
        url: Option<String>,
    },
    Audio {
        body: String,
        url: Option<String>,
    },
    File {
        body: String,
        url: Option<String>,
    },
    Location,
    Emote {
        body: String,
    },
    #[default]
    Other,
}

impl IncomingMessageContent {
    /// Extract the body text from the message content
    pub fn body(&self) -> Option<&str> {
        match self {
            Self::Text { body } | Self::Notice { body } | Self::Emote { body } => Some(body),
            Self::Image { body, .. }
            | Self::Video { body, .. }
            | Self::Audio { body, .. }
            | Self::File { body, .. } => Some(body),
            Self::Location => Some("[Location]"),
            Self::Other => None,
        }
    }

    /// Get a string representation of the message type
    pub fn message_type_str(&self) -> &'static str {
        match self {
            Self::Text { .. } => "text",
            Self::Notice { .. } => "notice",
            Self::Image { .. } => "image",
            Self::Video { .. } => "video",
            Self::Audio { .. } => "audio",
            Self::File { .. } => "file",
            Self::Location => "location",
            Self::Emote { .. } => "emote",
            Self::Other => "other",
        }
    }
}

/// Room member information
#[derive(Debug, Clone)]
pub struct RoomMember {
    pub user_id: String,
    pub localpart: String,
}

// ============================================================================
// Trait Definitions
// ============================================================================

/// Abstraction over Matrix SDK client operations
#[async_trait]
pub trait MatrixClientInterface: Send + Sync {
    /// Get all joined rooms with basic info
    async fn get_joined_rooms(&self) -> Result<Vec<RoomInfo>>;

    /// Get a room by its ID
    async fn get_room(&self, room_id: &str) -> Option<Arc<dyn RoomInterface>>;

    /// Upload media to the homeserver, returns mxc:// URI
    async fn upload_media(&self, mime_type: &str, data: Vec<u8>) -> Result<String>;

    /// Get the user ID of the logged in user
    fn user_id(&self) -> Option<String>;
}

/// Abstraction over Matrix room operations
#[async_trait]
pub trait RoomInterface: Send + Sync {
    /// Get the room ID
    fn room_id(&self) -> String;

    /// Get the display name of the room
    async fn display_name(&self) -> Result<String>;

    /// Fetch messages from the room
    async fn fetch_messages(
        &self,
        limit: u64,
        filter_prefix: Option<&str>,
    ) -> Result<Vec<crate::utils::bridge::BridgeMessage>>;

    /// Send a text message to the room
    async fn send_text(&self, message: &str) -> Result<()>;

    /// Send an image with caption
    async fn send_image(&self, mxc_uri: &str, caption: &str, size: u64) -> Result<()>;

    /// Check if the room is muted
    async fn is_muted(&self) -> bool;

    /// Get the read receipt timestamp for a user
    async fn get_read_receipt_timestamp(&self, user_id: &str) -> Option<i64>;

    /// Get all members of the room
    async fn get_members(&self) -> Result<Vec<RoomMember>>;

    /// Get the last activity timestamp in seconds
    async fn get_last_activity(&self) -> i64;
}

// ============================================================================
// Real Implementation Wrappers
// ============================================================================

use matrix_sdk::notification_settings::RoomNotificationMode;
use matrix_sdk::room::MessagesOptions;
use matrix_sdk::ruma::events::room::message::{MessageType, SyncRoomMessageEvent};
use matrix_sdk::ruma::events::AnySyncTimelineEvent;
use matrix_sdk::RoomMemberships;

/// Wrapper around the real Matrix SDK client
pub struct MatrixClientWrapper {
    client: Arc<matrix_sdk::Client>,
}

impl MatrixClientWrapper {
    pub fn new(client: Arc<matrix_sdk::Client>) -> Self {
        Self { client }
    }

    pub fn inner(&self) -> &Arc<matrix_sdk::Client> {
        &self.client
    }
}

#[async_trait]
impl MatrixClientInterface for MatrixClientWrapper {
    async fn get_joined_rooms(&self) -> Result<Vec<RoomInfo>> {
        let rooms = self.client.joined_rooms();
        let mut result = Vec::new();
        for room in rooms {
            let name = room.display_name().await?.to_string();
            result.push(RoomInfo {
                room_id: room.room_id().to_string(),
                display_name: name,
            });
        }
        Ok(result)
    }

    async fn get_room(&self, room_id: &str) -> Option<Arc<dyn RoomInterface>> {
        let room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id).ok()?;
        let room = self.client.get_room(&room_id)?;
        Some(Arc::new(RoomWrapper::new(room, self.client.clone())))
    }

    async fn upload_media(&self, mime_type: &str, data: Vec<u8>) -> Result<String> {
        let mime: mime_guess::mime::Mime = mime_type
            .parse()
            .unwrap_or(mime_guess::mime::APPLICATION_OCTET_STREAM);
        let response = self.client.media().upload(&mime, data, None).await?;
        Ok(response.content_uri.to_string())
    }

    fn user_id(&self) -> Option<String> {
        self.client.user_id().map(|id| id.to_string())
    }
}

/// Wrapper around a real Matrix SDK room
pub struct RoomWrapper {
    room: matrix_sdk::room::Room,
    client: Arc<matrix_sdk::Client>,
}

impl RoomWrapper {
    pub fn new(room: matrix_sdk::room::Room, client: Arc<matrix_sdk::Client>) -> Self {
        Self { room, client }
    }
}

#[async_trait]
impl RoomInterface for RoomWrapper {
    fn room_id(&self) -> String {
        self.room.room_id().to_string()
    }

    async fn display_name(&self) -> Result<String> {
        Ok(self.room.display_name().await?.to_string())
    }

    async fn fetch_messages(
        &self,
        limit: u64,
        filter_prefix: Option<&str>,
    ) -> Result<Vec<crate::utils::bridge::BridgeMessage>> {
        let room_name = self.room.display_name().await?.to_string();
        let mut options = MessagesOptions::backward();
        options.limit = matrix_sdk::ruma::UInt::new(limit).unwrap();

        let response = self.room.messages(options).await?;
        let mut messages = Vec::new();

        for event in response.chunk {
            if let Ok(AnySyncTimelineEvent::MessageLike(
                matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg),
            )) = event.raw().deserialize()
            {
                let (sender, timestamp, content) = match msg {
                    SyncRoomMessageEvent::Original(e) => {
                        (e.sender, i64::from(e.origin_server_ts.0) / 1000, e.content)
                    }
                    _ => continue,
                };

                // Apply sender prefix filter if provided
                if let Some(prefix) = filter_prefix {
                    if !sender.localpart().starts_with(prefix) {
                        continue;
                    }
                }

                let (msgtype, body) = match content.msgtype {
                    MessageType::Text(t) => ("text", t.body),
                    MessageType::Notice(n) => ("notice", n.body),
                    MessageType::Image(i) => (
                        "image",
                        if i.body.is_empty() {
                            "📎 IMAGE".into()
                        } else {
                            i.body
                        },
                    ),
                    MessageType::Video(v) => (
                        "video",
                        if v.body.is_empty() {
                            "📎 VIDEO".into()
                        } else {
                            v.body
                        },
                    ),
                    MessageType::File(f) => (
                        "file",
                        if f.body.is_empty() {
                            "📎 FILE".into()
                        } else {
                            f.body
                        },
                    ),
                    MessageType::Audio(a) => (
                        "audio",
                        if a.body.is_empty() {
                            "📎 AUDIO".into()
                        } else {
                            a.body
                        },
                    ),
                    MessageType::Location(_) => ("location", "📍 LOCATION".into()),
                    MessageType::Emote(t) => ("emote", t.body),
                    _ => continue,
                };

                messages.push(crate::utils::bridge::BridgeMessage {
                    sender: sender.to_string(),
                    sender_display_name: sender.localpart().to_string(),
                    content: body,
                    timestamp,
                    formatted_timestamp: String::new(), // Will be formatted by caller
                    message_type: msgtype.to_string(),
                    room_name: room_name.clone(),
                    media_url: None,
                    room_id: Some(self.room.room_id().to_string()),
                });
            }
        }

        Ok(messages)
    }

    async fn send_text(&self, message: &str) -> Result<()> {
        use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
        self.room
            .send(RoomMessageEventContent::text_plain(message))
            .await?;
        Ok(())
    }

    async fn send_image(&self, mxc_uri: &str, caption: &str, size: u64) -> Result<()> {
        use matrix_sdk::ruma::events::room::message::{
            ImageMessageEventContent, RoomMessageEventContent,
        };
        use matrix_sdk::ruma::events::room::ImageInfo;

        let mxc = matrix_sdk::ruma::OwnedMxcUri::from(mxc_uri.to_string());
        let mut img = ImageMessageEventContent::plain(caption.to_owned(), mxc);
        let mut imageinfo = ImageInfo::new();
        imageinfo.size = Some(matrix_sdk::ruma::UInt::new(size).unwrap_or_default());
        img.info = Some(Box::new(imageinfo));

        let content = RoomMessageEventContent::new(MessageType::Image(img));
        self.room.send(content).await?;
        Ok(())
    }

    async fn is_muted(&self) -> bool {
        self.room.user_defined_notification_mode().await == Some(RoomNotificationMode::Mute)
    }

    async fn get_read_receipt_timestamp(&self, user_id: &str) -> Option<i64> {
        use matrix_sdk::ruma::events::receipt::{ReceiptThread, ReceiptType};
        use matrix_sdk::ruma::{api::client::room::get_room_event, OwnedUserId};

        let own_user_id = OwnedUserId::try_from(user_id).ok()?;

        if let Ok(Some((receipt_event_id, _))) = self
            .room
            .load_user_receipt(ReceiptType::Read, ReceiptThread::Unthreaded, &own_user_id)
            .await
        {
            let request = get_room_event::v3::Request::new(
                self.room.room_id().to_owned(),
                receipt_event_id.clone(),
            );
            if let Ok(response) = self.client.send(request).await {
                if let Ok(any_event) = response.event.deserialize_as::<AnySyncTimelineEvent>() {
                    return Some(i64::from(any_event.origin_server_ts().as_secs()));
                }
            }
        }
        None
    }

    async fn get_members(&self) -> Result<Vec<RoomMember>> {
        let members = self.room.members(RoomMemberships::JOIN).await?;
        Ok(members
            .into_iter()
            .map(|m| RoomMember {
                user_id: m.user_id().to_string(),
                localpart: m.user_id().localpart().to_string(),
            })
            .collect())
    }

    async fn get_last_activity(&self) -> i64 {
        let mut options = MessagesOptions::backward();
        options.limit = matrix_sdk::ruma::UInt::new(1).unwrap();
        match self.room.messages(options).await {
            Ok(response) => response
                .chunk
                .first()
                .and_then(|event| event.raw().deserialize().ok())
                .map(|e: AnySyncTimelineEvent| i64::from(e.origin_server_ts().0) / 1000)
                .unwrap_or(0),
            Err(_) => 0,
        }
    }
}

// ============================================================================
// Event Conversion
// ============================================================================

/// Extract URL from MediaSource enum
fn extract_media_url(source: &matrix_sdk::ruma::events::room::MediaSource) -> Option<String> {
    match source {
        matrix_sdk::ruma::events::room::MediaSource::Plain(url) => Some(url.to_string()),
        matrix_sdk::ruma::events::room::MediaSource::Encrypted(_) => None,
    }
}

impl IncomingBridgeEvent {
    /// Convert from raw Matrix SDK event
    pub fn from_sdk_event(
        event: &matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent,
    ) -> Self {
        let sender = event.sender.to_string();
        let sender_localpart = event.sender.localpart().to_string();

        let content = match &event.content.msgtype {
            MessageType::Text(t) => IncomingMessageContent::Text {
                body: t.body.clone(),
            },
            MessageType::Notice(n) => IncomingMessageContent::Notice {
                body: n.body.clone(),
            },
            MessageType::Image(i) => IncomingMessageContent::Image {
                body: i.body.clone(),
                url: extract_media_url(&i.source),
            },
            MessageType::Video(v) => IncomingMessageContent::Video {
                body: v.body.clone(),
                url: extract_media_url(&v.source),
            },
            MessageType::Audio(a) => IncomingMessageContent::Audio {
                body: a.body.clone(),
                url: extract_media_url(&a.source),
            },
            MessageType::File(f) => IncomingMessageContent::File {
                body: f.body.clone(),
                url: extract_media_url(&f.source),
            },
            MessageType::Location(_) => IncomingMessageContent::Location,
            MessageType::Emote(e) => IncomingMessageContent::Emote {
                body: e.body.clone(),
            },
            _ => IncomingMessageContent::Other,
        };

        let mentions = event
            .content
            .mentions
            .as_ref()
            .map(|m| m.user_ids.iter().map(|u| u.to_string()).collect());

        Self {
            event_id: event.event_id.to_string(),
            sender,
            sender_localpart,
            timestamp_ms: event.origin_server_ts.0.into(),
            content,
            mentions,
        }
    }
}

// ============================================================================
// Mock Implementations (for testing)
// No #[cfg(test)] so they're available to integration tests
// ============================================================================

use std::collections::HashMap;
use tokio::sync::Mutex;

pub struct MockMatrixClient {
    rooms: Arc<Mutex<HashMap<String, Arc<MockRoom>>>>,
    calls: Arc<Mutex<MockMatrixCalls>>,
    user_id: Option<String>,
    upload_result: Arc<Mutex<Result<String, String>>>,
}

#[derive(Default, Clone)]
pub struct MockMatrixCalls {
    pub get_joined_rooms_calls: usize,
    pub upload_media_calls: Vec<(String, usize)>, // (mime_type, data_len)
}

impl Default for MockMatrixClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MockMatrixClient {
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(Mutex::new(HashMap::new())),
            calls: Arc::new(Mutex::new(MockMatrixCalls::default())),
            user_id: None,
            upload_result: Arc::new(Mutex::new(Ok("mxc://test/uploaded".to_string()))),
        }
    }

    pub fn with_rooms(mut self, rooms: Vec<MockRoom>) -> Self {
        let mut map = HashMap::new();
        for room in rooms {
            map.insert(room.room_id.clone(), Arc::new(room));
        }
        self.rooms = Arc::new(Mutex::new(map));
        self
    }

    pub fn with_user_id(mut self, user_id: &str) -> Self {
        self.user_id = Some(user_id.to_string());
        self
    }

    pub fn with_upload_result(self, result: Result<String, String>) -> Self {
        let upload_result = self.upload_result.clone();
        tokio::spawn(async move {
            *upload_result.lock().await = result;
        });
        self
    }

    pub async fn get_calls(&self) -> MockMatrixCalls {
        self.calls.lock().await.clone()
    }

    pub async fn clear_calls(&self) {
        *self.calls.lock().await = MockMatrixCalls::default();
    }

    pub async fn add_room(&self, room: MockRoom) {
        let mut rooms = self.rooms.lock().await;
        rooms.insert(room.room_id.clone(), Arc::new(room));
    }
}

#[async_trait]
impl MatrixClientInterface for MockMatrixClient {
    async fn get_joined_rooms(&self) -> Result<Vec<RoomInfo>> {
        self.calls.lock().await.get_joined_rooms_calls += 1;
        let rooms = self.rooms.lock().await;
        Ok(rooms
            .values()
            .map(|r| RoomInfo {
                room_id: r.room_id.clone(),
                display_name: r.display_name.clone(),
            })
            .collect())
    }

    async fn get_room(&self, room_id: &str) -> Option<Arc<dyn RoomInterface>> {
        let rooms = self.rooms.lock().await;
        rooms
            .get(room_id)
            .map(|r| Arc::clone(r) as Arc<dyn RoomInterface>)
    }

    async fn upload_media(&self, mime_type: &str, data: Vec<u8>) -> Result<String> {
        self.calls
            .lock()
            .await
            .upload_media_calls
            .push((mime_type.to_string(), data.len()));
        let result = self.upload_result.lock().await;
        result
            .clone()
            .map_err(|e| anyhow::anyhow!("Upload failed: {}", e))
    }

    fn user_id(&self) -> Option<String> {
        self.user_id.clone()
    }
}

pub struct MockRoom {
    pub room_id: String,
    pub display_name: String,
    pub messages: Arc<Mutex<Vec<crate::utils::bridge::BridgeMessage>>>,
    pub is_muted: bool,
    pub members: Vec<RoomMember>,
    pub read_receipt_ts: Option<i64>,
    pub sent_messages: Arc<Mutex<Vec<String>>>,
    pub last_activity: i64,
}

impl MockRoom {
    pub fn new(room_id: &str, display_name: &str) -> Self {
        Self {
            room_id: room_id.to_string(),
            display_name: display_name.to_string(),
            messages: Arc::new(Mutex::new(Vec::new())),
            is_muted: false,
            members: Vec::new(),
            read_receipt_ts: None,
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            last_activity: 0,
        }
    }

    pub fn with_messages(mut self, messages: Vec<crate::utils::bridge::BridgeMessage>) -> Self {
        self.messages = Arc::new(Mutex::new(messages));
        self
    }

    pub fn with_muted(mut self, muted: bool) -> Self {
        self.is_muted = muted;
        self
    }

    pub fn with_members(mut self, members: Vec<RoomMember>) -> Self {
        self.members = members;
        self
    }

    pub fn with_last_activity(mut self, ts: i64) -> Self {
        self.last_activity = ts;
        self
    }

    pub async fn get_sent_messages(&self) -> Vec<String> {
        self.sent_messages.lock().await.clone()
    }
}

#[async_trait]
impl RoomInterface for MockRoom {
    fn room_id(&self) -> String {
        self.room_id.clone()
    }

    async fn display_name(&self) -> Result<String> {
        Ok(self.display_name.clone())
    }

    async fn fetch_messages(
        &self,
        limit: u64,
        filter_prefix: Option<&str>,
    ) -> Result<Vec<crate::utils::bridge::BridgeMessage>> {
        let messages = self.messages.lock().await;
        let mut result: Vec<_> = messages
            .iter()
            .filter(|m| {
                if let Some(prefix) = filter_prefix {
                    m.sender_display_name.starts_with(prefix)
                } else {
                    true
                }
            })
            .take(limit as usize)
            .cloned()
            .collect();
        result.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(result)
    }

    async fn send_text(&self, message: &str) -> Result<()> {
        self.sent_messages.lock().await.push(message.to_string());
        Ok(())
    }

    async fn send_image(&self, _mxc_uri: &str, caption: &str, _size: u64) -> Result<()> {
        self.sent_messages
            .lock()
            .await
            .push(format!("[IMAGE] {}", caption));
        Ok(())
    }

    async fn is_muted(&self) -> bool {
        self.is_muted
    }

    async fn get_read_receipt_timestamp(&self, _user_id: &str) -> Option<i64> {
        self.read_receipt_ts
    }

    async fn get_members(&self) -> Result<Vec<RoomMember>> {
        Ok(self.members.clone())
    }

    async fn get_last_activity(&self) -> i64 {
        self.last_activity
    }
}

// ============================================================================
// Pure Decision Functions
// ============================================================================

use std::time::{SystemTime, UNIX_EPOCH};

/// Determine if a message should be processed based on its age
pub fn should_process_message(timestamp_ms: u64, max_age_ms: u64) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    now.saturating_sub(timestamp_ms) <= max_age_ms
}

/// Detect if a message indicates a bridge disconnection
pub fn is_disconnection_message(content: &str) -> bool {
    let disconnection_patterns = [
        "disconnected",
        "connection lost",
        "logged out",
        "authentication failed",
        "login failed",
        "bad_credentials",
        "wa-logged-out",
        "wa-not-logged-in",
        "device_removed",
        "relogin to continue",
        "not logged in",
        "session expired",
    ];

    let lower = content.to_lowercase();
    disconnection_patterns.iter().any(|p| lower.contains(p))
}

/// Check if a message is a health check or status message that should be skipped
pub fn is_health_check_message(content: &str) -> bool {
    let lower = content.to_lowercase();
    lower.contains("already logged in")
        || lower.contains("successfully logged in")
        || lower.contains("queued sync")
        || lower.contains("unknown command")
}

/// Check if a message contains error content that should be skipped
pub fn is_error_message(content: &str) -> bool {
    content.contains("Failed to bridge media")
        || content.contains("media no longer available")
        || content.contains("Decrypting message from WhatsApp failed")
        || content.starts_with("* Failed to")
}

/// Infer the service type from room name and sender localpart
pub fn infer_service_from_room(room_name: &str, sender_localpart: &str) -> Option<String> {
    let sender_localpart = sender_localpart.trim().to_lowercase();
    let room_name = room_name.to_lowercase();

    if room_name.contains("(wa)")
        || sender_localpart.starts_with("whatsapp_")
        || sender_localpart.starts_with("whatsapp")
    {
        return Some("whatsapp".to_string());
    }
    if room_name.contains("(tg)")
        || sender_localpart.starts_with("telegram_")
        || sender_localpart.starts_with("telegram")
    {
        return Some("telegram".to_string());
    }
    if room_name.contains("signal")
        || sender_localpart.starts_with("signal_")
        || sender_localpart.starts_with("signal")
    {
        return Some("signal".to_string());
    }
    None
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_process_message_accepts_recent() {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 1 second ago should be processed with 30 min window
        assert!(should_process_message(now_ms - 1000, 30 * 60 * 1000));
    }

    #[test]
    fn test_should_process_message_rejects_old() {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 31 minutes ago should NOT be processed with 30 min window
        assert!(!should_process_message(
            now_ms - 31 * 60 * 1000,
            30 * 60 * 1000
        ));
    }

    #[test]
    fn test_is_disconnection_message() {
        assert!(is_disconnection_message("Device has been disconnected"));
        assert!(is_disconnection_message("Login failed: bad_credentials"));
        assert!(is_disconnection_message("wa-logged-out event received"));
        assert!(!is_disconnection_message("Successfully logged in"));
        assert!(!is_disconnection_message("Message delivered"));
    }

    #[test]
    fn test_is_health_check_message() {
        assert!(is_health_check_message("Already logged in"));
        assert!(is_health_check_message(
            "Successfully logged in to WhatsApp"
        ));
        assert!(is_health_check_message("Queued sync"));
        assert!(is_health_check_message("Unknown command: test"));
        assert!(!is_health_check_message("New message from John"));
    }

    #[test]
    fn test_is_error_message() {
        assert!(is_error_message("Failed to bridge media: error"));
        assert!(is_error_message("media no longer available"));
        assert!(is_error_message("Decrypting message from WhatsApp failed"));
        assert!(is_error_message("* Failed to send message"));
        assert!(!is_error_message("Hello, how are you?"));
    }

    #[test]
    fn test_infer_service_from_room_whatsapp() {
        assert_eq!(
            infer_service_from_room("John (WA)", "whatsapp_123"),
            Some("whatsapp".to_string())
        );
        assert_eq!(
            infer_service_from_room("John (wa)", "user"),
            Some("whatsapp".to_string())
        );
    }

    #[test]
    fn test_infer_service_from_room_telegram() {
        assert_eq!(
            infer_service_from_room("John (TG)", "telegram_123"),
            Some("telegram".to_string())
        );
    }

    #[test]
    fn test_infer_service_from_room_signal() {
        assert_eq!(
            infer_service_from_room("Signal Chat", "signal_456"),
            Some("signal".to_string())
        );
    }

    #[test]
    fn test_infer_service_from_room_none() {
        assert_eq!(infer_service_from_room("Random Room", "random_user"), None);
    }

    #[test]
    fn test_incoming_message_content_body() {
        let text = IncomingMessageContent::Text {
            body: "hello".to_string(),
        };
        assert_eq!(text.body(), Some("hello"));

        let location = IncomingMessageContent::Location;
        assert_eq!(location.body(), Some("[Location]"));

        let other = IncomingMessageContent::Other;
        assert_eq!(other.body(), None);
    }

    #[test]
    fn test_incoming_message_content_type_str() {
        assert_eq!(
            IncomingMessageContent::Text {
                body: String::new()
            }
            .message_type_str(),
            "text"
        );
        assert_eq!(
            IncomingMessageContent::Image {
                body: String::new(),
                url: None
            }
            .message_type_str(),
            "image"
        );
        assert_eq!(
            IncomingMessageContent::Location.message_type_str(),
            "location"
        );
    }

    #[tokio::test]
    async fn test_mock_client_get_joined_rooms() {
        let room1 = MockRoom::new("!room1:server", "Room One");
        let room2 = MockRoom::new("!room2:server", "Room Two");

        let client = MockMatrixClient::new().with_rooms(vec![room1, room2]);

        let rooms = client.get_joined_rooms().await.unwrap();
        assert_eq!(rooms.len(), 2);

        let calls = client.get_calls().await;
        assert_eq!(calls.get_joined_rooms_calls, 1);
    }

    #[tokio::test]
    async fn test_mock_client_get_room() {
        let room = MockRoom::new("!room1:server", "Test Room");
        let client = MockMatrixClient::new().with_rooms(vec![room]);

        let found = client.get_room("!room1:server").await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().room_id(), "!room1:server");

        let not_found = client.get_room("!nonexistent:server").await;
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_mock_room_send_text() {
        let room = MockRoom::new("!room1:server", "Test Room");
        room.send_text("Hello, world!").await.unwrap();

        let sent = room.get_sent_messages().await;
        assert_eq!(sent, vec!["Hello, world!"]);
    }

    #[tokio::test]
    async fn test_mock_room_muted() {
        let room = MockRoom::new("!room1:server", "Muted Room").with_muted(true);
        assert!(room.is_muted().await);

        let room2 = MockRoom::new("!room2:server", "Normal Room").with_muted(false);
        assert!(!room2.is_muted().await);
    }
}
