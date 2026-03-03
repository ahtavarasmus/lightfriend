//! Matrix Test Server Utilities
//!
//! Provides helpers for running integration tests against a real or mock Matrix server.
//! These utilities support both the Docker-based tests and can be used to create
//! programmatic test setups.

use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::Sha1;
use std::time::Duration;

/// Configuration for connecting to a Matrix test server
#[derive(Clone, Debug)]
pub struct MatrixTestConfig {
    /// Homeserver URL (e.g., "http://localhost:8008")
    pub homeserver_url: String,
    /// Shared secret for admin API registration
    pub shared_secret: String,
    /// Server domain (e.g., "matrix.local")
    pub domain: String,
}

impl Default for MatrixTestConfig {
    fn default() -> Self {
        Self {
            homeserver_url: std::env::var("TEST_MATRIX_HOMESERVER")
                .unwrap_or_else(|_| "http://localhost:8008".to_string()),
            shared_secret: std::env::var("TEST_MATRIX_SHARED_SECRET")
                .unwrap_or_else(|_| "test_shared_secret".to_string()),
            domain: std::env::var("TEST_MATRIX_DOMAIN")
                .unwrap_or_else(|_| "matrix.local".to_string()),
        }
    }
}

impl MatrixTestConfig {
    /// Create config for testing with custom values
    pub fn new(homeserver_url: &str, shared_secret: &str, domain: &str) -> Self {
        Self {
            homeserver_url: homeserver_url.to_string(),
            shared_secret: shared_secret.to_string(),
            domain: domain.to_string(),
        }
    }

    /// Get the bot user ID for a service
    pub fn bot_user_id(&self, service: &str) -> String {
        match service {
            "whatsapp" => format!("@whatsappbot:{}", self.domain),
            "signal" => format!("@signalbot:{}", self.domain),
            "telegram" => format!("@telegrambot:{}", self.domain),
            _ => format!("@{}bot:{}", service, self.domain),
        }
    }
}

/// Credentials returned from user registration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestUserCredentials {
    pub user_id: String,
    pub username: String,
    pub access_token: String,
    pub device_id: String,
    pub password: String,
}

/// Matrix test server client for integration tests
pub struct MatrixTestClient {
    config: MatrixTestConfig,
    http_client: HttpClient,
}

impl MatrixTestClient {
    /// Create a new test client with default config
    pub fn new() -> Self {
        Self::with_config(MatrixTestConfig::default())
    }

    /// Create a new test client with custom config
    pub fn with_config(config: MatrixTestConfig) -> Self {
        Self {
            config,
            http_client: HttpClient::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Check if the Synapse server is healthy
    pub async fn is_healthy(&self) -> bool {
        match self
            .http_client
            .get(format!("{}/health", self.config.homeserver_url))
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// Wait for server to become healthy with timeout
    pub async fn wait_for_healthy(&self, timeout: Duration) -> Result<()> {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if self.is_healthy().await {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Err(anyhow!(
            "Server did not become healthy within {:?}",
            timeout
        ))
    }

    /// Register a new test user via admin API
    pub async fn register_user(&self, username: &str) -> Result<TestUserCredentials> {
        // Get registration nonce
        let nonce_res = self
            .http_client
            .get(format!(
                "{}/_synapse/admin/v1/register",
                self.config.homeserver_url
            ))
            .send()
            .await?
            .json::<Value>()
            .await?;

        let nonce = nonce_res["nonce"]
            .as_str()
            .ok_or_else(|| anyhow!("No nonce in response"))?;

        // Generate password
        let password = format!("testpass_{}", uuid::Uuid::new_v4());

        // Calculate HMAC
        let mac_content = format!("{}\0{}\0{}\0notadmin", nonce, username, password);
        let mut mac = Hmac::<Sha1>::new_from_slice(self.config.shared_secret.as_bytes())?;
        mac.update(mac_content.as_bytes());
        let mac_result = hex::encode(mac.finalize().into_bytes());

        // Register user
        let response = self
            .http_client
            .post(format!(
                "{}/_synapse/admin/v1/register",
                self.config.homeserver_url
            ))
            .json(&json!({
                "nonce": nonce,
                "username": username,
                "password": password,
                "admin": false,
                "mac": mac_result
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Registration failed: {}", error_text));
        }

        let register_res: Value = response.json().await?;

        Ok(TestUserCredentials {
            user_id: format!("@{}:{}", username, self.config.domain),
            username: username.to_string(),
            access_token: register_res["access_token"]
                .as_str()
                .ok_or_else(|| anyhow!("No access_token"))?
                .to_string(),
            device_id: register_res["device_id"]
                .as_str()
                .ok_or_else(|| anyhow!("No device_id"))?
                .to_string(),
            password,
        })
    }

    /// Register a user with a unique random suffix
    pub async fn register_unique_user(&self, prefix: &str) -> Result<TestUserCredentials> {
        let username = format!("{}_{}", prefix, &uuid::Uuid::new_v4().to_string()[..8]);
        self.register_user(&username).await
    }

    /// Create a room
    pub async fn create_room(&self, access_token: &str, name: Option<&str>) -> Result<String> {
        let mut body = json!({
            "preset": "private_chat",
            "is_direct": true
        });

        if let Some(room_name) = name {
            body["name"] = json!(room_name);
        }

        let response = self
            .http_client
            .post(format!(
                "{}/_matrix/client/v3/createRoom",
                self.config.homeserver_url
            ))
            .header("Authorization", format!("Bearer {}", access_token))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Room creation failed: {}", error_text));
        }

        let room_res: Value = response.json().await?;
        room_res["room_id"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("No room_id in response"))
    }

    /// Invite a user to a room
    pub async fn invite_to_room(
        &self,
        access_token: &str,
        room_id: &str,
        user_id: &str,
    ) -> Result<()> {
        let response = self
            .http_client
            .post(format!(
                "{}/_matrix/client/v3/rooms/{}/invite",
                self.config.homeserver_url,
                urlencoding::encode(room_id)
            ))
            .header("Authorization", format!("Bearer {}", access_token))
            .json(&json!({ "user_id": user_id }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Invite failed: {}", error_text));
        }

        Ok(())
    }

    /// Send a text message to a room
    pub async fn send_message(
        &self,
        access_token: &str,
        room_id: &str,
        message: &str,
    ) -> Result<String> {
        let txn_id = uuid::Uuid::new_v4().to_string();

        let response = self
            .http_client
            .put(format!(
                "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
                self.config.homeserver_url,
                urlencoding::encode(room_id),
                txn_id
            ))
            .header("Authorization", format!("Bearer {}", access_token))
            .json(&json!({
                "msgtype": "m.text",
                "body": message
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Send message failed: {}", error_text));
        }

        let send_res: Value = response.json().await?;
        send_res["event_id"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("No event_id in response"))
    }

    /// Get room members
    pub async fn get_room_members(&self, access_token: &str, room_id: &str) -> Result<Vec<String>> {
        let response = self
            .http_client
            .get(format!(
                "{}/_matrix/client/v3/rooms/{}/members",
                self.config.homeserver_url,
                urlencoding::encode(room_id)
            ))
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Get members failed: {}", error_text));
        }

        let members_res: Value = response.json().await?;
        let members: Vec<String> = members_res["chunk"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|m| {
                if m["content"]["membership"] == "join" {
                    m["state_key"].as_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();

        Ok(members)
    }

    /// Get recent messages from a room
    pub async fn get_room_messages(
        &self,
        access_token: &str,
        room_id: &str,
        limit: u32,
    ) -> Result<Vec<Value>> {
        let response = self
            .http_client
            .get(format!(
                "{}/_matrix/client/v3/rooms/{}/messages",
                self.config.homeserver_url,
                urlencoding::encode(room_id)
            ))
            .header("Authorization", format!("Bearer {}", access_token))
            .query(&[("dir", "b"), ("limit", &limit.to_string())])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Get messages failed: {}", error_text));
        }

        let messages_res: Value = response.json().await?;
        Ok(messages_res["chunk"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    /// Wait for a specific user to join a room
    pub async fn wait_for_member(
        &self,
        access_token: &str,
        room_id: &str,
        user_id: &str,
        timeout: Duration,
    ) -> Result<bool> {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            match self.get_room_members(access_token, room_id).await {
                Ok(members) => {
                    if members.iter().any(|m| m == user_id) {
                        return Ok(true);
                    }
                }
                Err(e) => {
                    tracing::warn!("Error checking members: {}", e);
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Ok(false)
    }

    /// Wait for a message matching a predicate
    pub async fn wait_for_message<F>(
        &self,
        access_token: &str,
        room_id: &str,
        predicate: F,
        timeout: Duration,
    ) -> Result<Option<Value>>
    where
        F: Fn(&Value) -> bool,
    {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            match self.get_room_messages(access_token, room_id, 20).await {
                Ok(messages) => {
                    for msg in messages {
                        if predicate(&msg) {
                            return Ok(Some(msg));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Error checking messages: {}", e);
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Ok(None)
    }

    /// Invite a bridge bot and wait for it to join
    pub async fn setup_bridge_room(
        &self,
        access_token: &str,
        service: &str,
    ) -> Result<(String, String)> {
        let room_id = self
            .create_room(access_token, Some(&format!("{} Test Room", service)))
            .await?;

        let bot_id = self.config.bot_user_id(service);
        self.invite_to_room(access_token, &room_id, &bot_id).await?;

        let joined = self
            .wait_for_member(access_token, &room_id, &bot_id, Duration::from_secs(10))
            .await?;

        if !joined {
            return Err(anyhow!("{} bot did not join within timeout", service));
        }

        Ok((room_id, bot_id))
    }
}

impl Default for MatrixTestClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Predicates for common message matching patterns
pub mod message_predicates {
    use serde_json::Value;

    /// Check if message is an image from a specific sender
    pub fn is_image_from(sender: &str) -> impl Fn(&Value) -> bool + '_ {
        move |msg: &Value| {
            msg["content"]["msgtype"] == "m.image" && msg["sender"].as_str() == Some(sender)
        }
    }

    /// Check if message body contains text (case insensitive)
    pub fn body_contains(text: &str) -> impl Fn(&Value) -> bool + '_ {
        let text_lower = text.to_lowercase();
        move |msg: &Value| {
            msg["content"]["body"]
                .as_str()
                .map(|b| b.to_lowercase().contains(&text_lower))
                .unwrap_or(false)
        }
    }

    /// Check if message is from sender and body contains text
    pub fn from_with_body<'a>(sender: &'a str, text: &'a str) -> impl Fn(&Value) -> bool + 'a {
        let text_lower = text.to_lowercase();
        move |msg: &Value| {
            msg["sender"].as_str() == Some(sender)
                && msg["content"]["body"]
                    .as_str()
                    .map(|b| b.to_lowercase().contains(&text_lower))
                    .unwrap_or(false)
        }
    }

    /// Check if message is a notice (system message from bot)
    pub fn is_notice_from(sender: &str) -> impl Fn(&Value) -> bool + '_ {
        move |msg: &Value| {
            msg["content"]["msgtype"] == "m.notice" && msg["sender"].as_str() == Some(sender)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = MatrixTestConfig::default();
        assert_eq!(config.homeserver_url, "http://localhost:8008");
        assert_eq!(config.domain, "matrix.local");
    }

    #[test]
    fn test_bot_user_id() {
        let config = MatrixTestConfig::default();
        assert_eq!(config.bot_user_id("whatsapp"), "@whatsappbot:matrix.local");
        assert_eq!(config.bot_user_id("signal"), "@signalbot:matrix.local");
        assert_eq!(config.bot_user_id("telegram"), "@telegrambot:matrix.local");
    }

    #[test]
    fn test_message_predicates() {
        let msg = serde_json::json!({
            "sender": "@whatsappbot:matrix.local",
            "content": {
                "msgtype": "m.notice",
                "body": "Scan this QR code to login"
            }
        });

        assert!(message_predicates::body_contains("qr code")(&msg));
        assert!(message_predicates::body_contains("QR CODE")(&msg));
        assert!(message_predicates::is_notice_from(
            "@whatsappbot:matrix.local"
        )(&msg));
        assert!(!message_predicates::is_image_from(
            "@whatsappbot:matrix.local"
        )(&msg));
    }
}
