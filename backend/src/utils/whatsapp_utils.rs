use std::sync::Arc;
use anyhow::{anyhow, Result};
use matrix_sdk::{
    Client as MatrixClient,
    room::Room,
    ruma::{
        events::room::message::{RoomMessageEventContent, SyncRoomMessageEvent, MessageType},
        events::{AnyTimelineEvent, AnySyncTimelineEvent},
        OwnedRoomId, OwnedUserId, OwnedDeviceId,
    },
};
use serde::{Deserialize, Serialize};
use crate::{AppState, models::user_models::Bridge};

#[derive(Debug, Serialize, Deserialize)]
pub struct WhatsAppMessage {
    pub sender: String,
    pub sender_display_name: String,
    pub content: String,
    pub timestamp: i64,
    pub message_type: String,
    pub room_name: String,
}

use std::time::Duration;
use tokio::time::sleep;


pub async fn fetch_whatsapp_messages(
    state: &AppState,
    user_id: i32,
    start_time: i64,
    end_time: i64,
) -> Result<Vec<WhatsAppMessage>> {

    let (username, access_token, device_id) = state.user_repository
        .get_matrix_credentials(user_id)?
        .ok_or_else(|| anyhow!("Matrix credentials not found"))?;

    let homeserver_url = std::env::var("MATRIX_HOMESERVER")
        .expect("MATRIX_HOMESERVER not set");

    let parsed_url = url::Url::parse(&homeserver_url)?;
    let domain = parsed_url.host_str().ok_or_else(|| anyhow!("Invalid homeserver URL"))?;
    let full_user_id = format!("@{}:{}", username, domain);

    // Setup SQLite store path
    let store_path = format!(
        "{}/{}",
        std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
            .expect("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"),
        username
    );
    std::fs::create_dir_all(&store_path)?;

    let client = MatrixClient::builder()
        .homeserver_url(homeserver_url)
        .sqlite_store(store_path.clone(), None)
        .build()
        .await?;

    tracing::info!("checking invited rooms: {:#?}", client.invited_rooms());
    for room in client.invited_rooms() {
        tracing::info!("Joining invited room: {}", room.room_id());
        client.join_room_by_id(room.room_id()).await?;
    }
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    tracing::info!("checking joined rooms: {:#?}", client.joined_rooms());
    for room in client.joined_rooms() {
        tracing::info!("✅ Joined room: {}", room.room_id());
    }

    let mut messages = Vec::new();

    for room in client.joined_rooms() {
        let room_id = room.room_id();
        let room_name = room.display_name().await.unwrap_or_else(|_| matrix_sdk::RoomDisplayName::Named(room_id.to_string()));

        let mut from_token = None;

        loop {
            let mut options = matrix_sdk::room::MessagesOptions::backward();
            options.limit = matrix_sdk::ruma::UInt::new(100).unwrap();
            if let Some(token) = from_token.clone() {
                options.from = Some(token);
            }

            let response = room.messages(options).await?;
            let chunk = response.chunk;
            from_token = response.end;
            if chunk.is_empty() {
                break;
            }

            for event in chunk {
                let raw_event = event.raw();
                if let Ok(any_sync_event) = raw_event.deserialize() {
                    if let AnySyncTimelineEvent::MessageLike(
                            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(msg)
                        ) = any_sync_event.clone() {

                        let (sender, timestamp, content) = match msg {
                            matrix_sdk::ruma::events::room::message::SyncRoomMessageEvent::Original(e) => {
                                let timestamp = i64::from(e.origin_server_ts.0) / 1000;

                                (e.sender, timestamp, e.content)
                            }
                            matrix_sdk::ruma::events::room::message::SyncRoomMessageEvent::Redacted(_) => continue,
                        };

                        let (msgtype, body) = match content.msgtype {
                            MessageType::Text(t) => ("text", t.body),
                            MessageType::Notice(n) => ("notice", n.body),
                            MessageType::Image(_) => ("image", "📎 IMAGE".into()),
                            MessageType::Video(_) => ("video", "📎 VIDEO".into()),
                            MessageType::File(_) => ("file", "📎 FILE".into()),
                            MessageType::Audio(_) => ("audio", "📎 AUDIO".into()),
                            MessageType::Location(_) => ("location", "📍 LOCATION".into()),
                            MessageType::Emote(t) => ("emote", t.body),
                            _ => continue,
                        };

                        if timestamp < start_time || timestamp > end_time {
                            continue;
                        }

                        messages.push(WhatsAppMessage {
                            sender: sender.to_string(),
                            sender_display_name: sender.localpart().to_string(),
                            content: body,
                            timestamp,
                            message_type: msgtype.to_string(),
                            room_name: room_name.to_string(),
                        });
                    }
                }
            }

            if from_token.is_none() {
                break;
            }

            // Optional: small delay to respect backpressure
            sleep(Duration::from_millis(100)).await;
        }
    }

    messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(messages)
}

