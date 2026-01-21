//! Bridge Integration Tests (Tier 3: Full Docker Stack Required)
//!
//! These tests run against the full Docker infrastructure including:
//! - Synapse homeserver
//! - mautrix-whatsapp bridge
//! - mautrix-signal bridge
//! - mautrix-telegram bridge
//!
//! Run with: cargo test --test bridge_integration_test -- --ignored
//! Requires: docker compose up -d (wait ~30s for bridges to initialize)
//!
//! IMPORTANT: These tests interact with real bridges and can test the
//! connection flow up to the point where a QR code needs to be scanned.

use hmac::{Hmac, Mac};
use reqwest::Client as HttpClient;
use serde_json::{json, Value};
use sha1::Sha1;
use std::time::Duration;
use tokio::time::sleep;

/// Test configuration - matches docker-compose.yml defaults
struct TestConfig {
    homeserver_url: String,
    shared_secret: String,
    domain: String,
}

impl Default for TestConfig {
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

/// Helper to register a test user via Synapse admin API
async fn register_test_user(
    config: &TestConfig,
    username: &str,
) -> Result<(String, String, String), String> {
    let http_client = HttpClient::new();

    // Get registration nonce
    let nonce_res = http_client
        .get(format!(
            "{}/_synapse/admin/v1/register",
            config.homeserver_url
        ))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch nonce: {}", e))?
        .json::<Value>()
        .await
        .map_err(|e| format!("Failed to parse nonce response: {}", e))?;

    let nonce = nonce_res["nonce"]
        .as_str()
        .ok_or_else(|| "No nonce in response".to_string())?;

    // Generate password
    let password = format!("testpass_{}", uuid::Uuid::new_v4());

    // Calculate HMAC
    let mac_content = format!("{}\0{}\0{}\0notadmin", nonce, username, password);
    let mut mac = Hmac::<Sha1>::new_from_slice(config.shared_secret.as_bytes())
        .map_err(|e| format!("Failed to create HMAC: {}", e))?;
    mac.update(mac_content.as_bytes());
    let mac_result = hex::encode(mac.finalize().into_bytes());

    // Register user
    let response = http_client
        .post(format!(
            "{}/_synapse/admin/v1/register",
            config.homeserver_url
        ))
        .json(&json!({
            "nonce": nonce,
            "username": username,
            "password": password,
            "admin": false,
            "mac": mac_result
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to send registration request: {}", e))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Registration failed: {}", error_text));
    }

    let register_res: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse registration response: {}", e))?;

    let access_token = register_res["access_token"]
        .as_str()
        .ok_or_else(|| "No access_token in response".to_string())?
        .to_string();

    let device_id = register_res["device_id"]
        .as_str()
        .ok_or_else(|| "No device_id in response".to_string())?
        .to_string();

    Ok((access_token, device_id, password))
}

/// Helper to create a room
async fn create_room(
    config: &TestConfig,
    access_token: &str,
    name: Option<&str>,
) -> Result<String, String> {
    let http_client = HttpClient::new();

    let mut body = json!({
        "preset": "private_chat",
        "is_direct": true
    });

    if let Some(room_name) = name {
        body["name"] = json!(room_name);
    }

    let response = http_client
        .post(format!(
            "{}/_matrix/client/v3/createRoom",
            config.homeserver_url
        ))
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to create room: {}", e))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Room creation failed: {}", error_text));
    }

    let room_res: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse room response: {}", e))?;

    room_res["room_id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No room_id in response".to_string())
}

/// Helper to invite a user to a room
async fn invite_to_room(
    config: &TestConfig,
    access_token: &str,
    room_id: &str,
    user_id: &str,
) -> Result<(), String> {
    let http_client = HttpClient::new();

    let response = http_client
        .post(format!(
            "{}/_matrix/client/v3/rooms/{}/invite",
            config.homeserver_url,
            urlencoding::encode(room_id)
        ))
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({ "user_id": user_id }))
        .send()
        .await
        .map_err(|e| format!("Failed to invite user: {}", e))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Invite failed: {}", error_text));
    }

    Ok(())
}

/// Helper to send a message to a room
async fn send_message(
    config: &TestConfig,
    access_token: &str,
    room_id: &str,
    message: &str,
) -> Result<String, String> {
    let http_client = HttpClient::new();
    let txn_id = uuid::Uuid::new_v4().to_string();

    let response = http_client
        .put(format!(
            "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
            config.homeserver_url,
            urlencoding::encode(room_id),
            txn_id
        ))
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&json!({
            "msgtype": "m.text",
            "body": message
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to send message: {}", e))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Send message failed: {}", error_text));
    }

    let send_res: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse send response: {}", e))?;

    send_res["event_id"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No event_id in response".to_string())
}

/// Helper to get room members
async fn get_room_members(
    config: &TestConfig,
    access_token: &str,
    room_id: &str,
) -> Result<Vec<String>, String> {
    let http_client = HttpClient::new();

    let response = http_client
        .get(format!(
            "{}/_matrix/client/v3/rooms/{}/members",
            config.homeserver_url,
            urlencoding::encode(room_id)
        ))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| format!("Failed to get members: {}", e))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Get members failed: {}", error_text));
    }

    let members_res: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse members response: {}", e))?;

    let members: Vec<String> = members_res["chunk"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|m| m["state_key"].as_str().map(|s| s.to_string()))
        .collect();

    Ok(members)
}

/// Helper to get recent messages from a room
async fn get_room_messages(
    config: &TestConfig,
    access_token: &str,
    room_id: &str,
    limit: u32,
) -> Result<Vec<Value>, String> {
    let http_client = HttpClient::new();

    let response = http_client
        .get(format!(
            "{}/_matrix/client/v3/rooms/{}/messages",
            config.homeserver_url,
            urlencoding::encode(room_id)
        ))
        .header("Authorization", format!("Bearer {}", access_token))
        .query(&[("dir", "b"), ("limit", &limit.to_string())])
        .send()
        .await
        .map_err(|e| format!("Failed to get messages: {}", e))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Get messages failed: {}", error_text));
    }

    let messages_res: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse messages response: {}", e))?;

    Ok(messages_res["chunk"]
        .as_array()
        .cloned()
        .unwrap_or_default())
}

/// Helper to wait for a user to join a room
async fn wait_for_member(
    config: &TestConfig,
    access_token: &str,
    room_id: &str,
    user_id: &str,
    timeout: Duration,
) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        match get_room_members(config, access_token, room_id).await {
            Ok(members) => {
                if members.iter().any(|m| m == user_id) {
                    return true;
                }
            }
            Err(e) => {
                eprintln!("Error checking members: {}", e);
            }
        }
        sleep(Duration::from_millis(500)).await;
    }
    false
}

/// Check if Synapse is healthy
async fn check_synapse_health(config: &TestConfig) -> bool {
    let http_client = HttpClient::new();
    match http_client
        .get(format!("{}/health", config.homeserver_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

// ============================================================================
// Infrastructure Tests
// ============================================================================

mod infrastructure_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires Docker Synapse"]
    async fn test_synapse_is_healthy() {
        let config = TestConfig::default();
        assert!(
            check_synapse_health(&config).await,
            "Synapse should be running and healthy. Run: docker compose up -d"
        );
    }

    #[tokio::test]
    #[ignore = "requires Docker Synapse"]
    async fn test_user_registration() {
        let config = TestConfig::default();

        if !check_synapse_health(&config).await {
            panic!("Synapse is not running. Start with: docker compose up -d");
        }

        let username = format!(
            "testuser_{}",
            &uuid::Uuid::new_v4().to_string().replace("-", "")[..8]
        );
        let result = register_test_user(&config, &username).await;

        assert!(result.is_ok(), "Registration should succeed: {:?}", result);
        let (access_token, device_id, _password) = result.unwrap();
        assert!(!access_token.is_empty(), "Should have access token");
        assert!(!device_id.is_empty(), "Should have device ID");
    }

    #[tokio::test]
    #[ignore = "requires Docker Synapse"]
    async fn test_room_creation() {
        let config = TestConfig::default();

        if !check_synapse_health(&config).await {
            panic!("Synapse is not running");
        }

        let username = format!("roomtest_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let (access_token, _, _) = register_test_user(&config, &username)
            .await
            .expect("Registration should succeed");

        let room_id = create_room(&config, &access_token, Some("Test Room"))
            .await
            .expect("Room creation should succeed");

        assert!(room_id.starts_with('!'), "Room ID should start with !");
        assert!(
            room_id.contains(&config.domain),
            "Room ID should contain domain"
        );
    }
}

// ============================================================================
// WhatsApp Bridge Tests
// ============================================================================

mod whatsapp_bridge_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires full Docker stack with bridges"]
    async fn test_whatsapp_bot_joins_room() {
        let config = TestConfig::default();

        if !check_synapse_health(&config).await {
            panic!("Synapse is not running. Start with: docker compose up -d");
        }

        // Register test user
        let username = format!("watest_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let (access_token, _, _) = register_test_user(&config, &username)
            .await
            .expect("Registration should succeed");

        // Create room for WhatsApp bridge
        let room_id = create_room(&config, &access_token, Some("WhatsApp Test"))
            .await
            .expect("Room creation should succeed");

        // Invite WhatsApp bot
        let bot_id = format!("@whatsappbot:{}", config.domain);
        invite_to_room(&config, &access_token, &room_id, &bot_id)
            .await
            .expect("Invite should succeed");

        // Wait for bot to join (timeout after 10 seconds)
        let bot_joined = wait_for_member(
            &config,
            &access_token,
            &room_id,
            &bot_id,
            Duration::from_secs(10),
        )
        .await;

        assert!(bot_joined, "WhatsApp bot should join the room within 10s");
    }

    #[tokio::test]
    #[ignore = "requires full Docker stack with bridges"]
    async fn test_whatsapp_login_qr_command() {
        let config = TestConfig::default();

        if !check_synapse_health(&config).await {
            panic!("Synapse is not running");
        }

        // Setup: Register user and create room with bot
        let username = format!("waqr_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let (access_token, _, _) = register_test_user(&config, &username)
            .await
            .expect("Registration should succeed");

        let room_id = create_room(&config, &access_token, None)
            .await
            .expect("Room creation should succeed");

        let bot_id = format!("@whatsappbot:{}", config.domain);
        invite_to_room(&config, &access_token, &room_id, &bot_id)
            .await
            .expect("Invite should succeed");

        // Wait for bot to join
        let bot_joined = wait_for_member(
            &config,
            &access_token,
            &room_id,
            &bot_id,
            Duration::from_secs(10),
        )
        .await;
        assert!(bot_joined, "Bot should join");

        // Send login command
        send_message(&config, &access_token, &room_id, "!wa login qr")
            .await
            .expect("Send command should succeed");

        // Wait for bot response
        sleep(Duration::from_secs(3)).await;

        // Check for QR code response
        let messages = get_room_messages(&config, &access_token, &room_id, 10)
            .await
            .expect("Should get messages");

        // Look for image message (QR code)
        let has_qr_image = messages
            .iter()
            .any(|m| m["content"]["msgtype"] == "m.image" && m["sender"].as_str() == Some(&bot_id));

        // Look for notice with scan instructions
        let has_scan_notice = messages.iter().any(|m| {
            m["content"]["msgtype"] == "m.notice"
                && m["sender"].as_str() == Some(&bot_id)
                && m["content"]["body"]
                    .as_str()
                    .map(|b| b.to_lowercase().contains("scan") || b.to_lowercase().contains("qr"))
                    .unwrap_or(false)
        });

        assert!(
            has_qr_image || has_scan_notice,
            "Bot should respond with QR code or scan instructions. Messages: {:?}",
            messages
        );
    }

    #[tokio::test]
    #[ignore = "requires full Docker stack with bridges"]
    async fn test_whatsapp_unknown_command() {
        let config = TestConfig::default();

        if !check_synapse_health(&config).await {
            panic!("Synapse is not running");
        }

        let username = format!("waunknown_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let (access_token, _, _) = register_test_user(&config, &username)
            .await
            .expect("Registration should succeed");

        let room_id = create_room(&config, &access_token, None)
            .await
            .expect("Room creation should succeed");

        let bot_id = format!("@whatsappbot:{}", config.domain);
        invite_to_room(&config, &access_token, &room_id, &bot_id)
            .await
            .expect("Invite should succeed");

        let bot_joined = wait_for_member(
            &config,
            &access_token,
            &room_id,
            &bot_id,
            Duration::from_secs(10),
        )
        .await;
        assert!(bot_joined, "Bot should join");

        // Send unknown command
        send_message(&config, &access_token, &room_id, "!wa foobar123")
            .await
            .expect("Send command should succeed");

        sleep(Duration::from_secs(2)).await;

        let messages = get_room_messages(&config, &access_token, &room_id, 10)
            .await
            .expect("Should get messages");

        let has_unknown_response = messages.iter().any(|m| {
            m["sender"].as_str() == Some(&bot_id)
                && m["content"]["body"]
                    .as_str()
                    .map(|b| b.to_lowercase().contains("unknown"))
                    .unwrap_or(false)
        });

        assert!(
            has_unknown_response,
            "Bot should respond to unknown command. Messages: {:?}",
            messages
        );
    }

    #[tokio::test]
    #[ignore = "requires full Docker stack with bridges"]
    async fn test_whatsapp_logout_command() {
        let config = TestConfig::default();

        if !check_synapse_health(&config).await {
            panic!("Synapse is not running");
        }

        let username = format!("walogout_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let (access_token, _, _) = register_test_user(&config, &username)
            .await
            .expect("Registration should succeed");

        let room_id = create_room(&config, &access_token, None)
            .await
            .expect("Room creation should succeed");

        let bot_id = format!("@whatsappbot:{}", config.domain);
        invite_to_room(&config, &access_token, &room_id, &bot_id)
            .await
            .expect("Invite should succeed");

        let bot_joined = wait_for_member(
            &config,
            &access_token,
            &room_id,
            &bot_id,
            Duration::from_secs(10),
        )
        .await;
        assert!(bot_joined, "Bot should join");

        // Send logout command
        send_message(&config, &access_token, &room_id, "!wa logout")
            .await
            .expect("Send command should succeed");

        sleep(Duration::from_secs(2)).await;

        let messages = get_room_messages(&config, &access_token, &room_id, 10)
            .await
            .expect("Should get messages");

        // Bot should respond (either confirming logout or saying not logged in)
        let has_response = messages.iter().any(|m| {
            m["sender"].as_str() == Some(&bot_id)
                && m["content"]["body"]
                    .as_str()
                    .map(|b| {
                        let lower = b.to_lowercase();
                        lower.contains("logout")
                            || lower.contains("logged")
                            || lower.contains("not")
                    })
                    .unwrap_or(false)
        });

        assert!(
            has_response,
            "Bot should respond to logout command. Messages: {:?}",
            messages
        );
    }
}

// ============================================================================
// Signal Bridge Tests
// ============================================================================

mod signal_bridge_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires full Docker stack with bridges"]
    async fn test_signal_bot_joins_room() {
        let config = TestConfig::default();

        if !check_synapse_health(&config).await {
            panic!("Synapse is not running");
        }

        let username = format!("sigtest_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let (access_token, _, _) = register_test_user(&config, &username)
            .await
            .expect("Registration should succeed");

        let room_id = create_room(&config, &access_token, Some("Signal Test"))
            .await
            .expect("Room creation should succeed");

        let bot_id = format!("@signalbot:{}", config.domain);
        invite_to_room(&config, &access_token, &room_id, &bot_id)
            .await
            .expect("Invite should succeed");

        let bot_joined = wait_for_member(
            &config,
            &access_token,
            &room_id,
            &bot_id,
            Duration::from_secs(10),
        )
        .await;

        assert!(bot_joined, "Signal bot should join the room within 10s");
    }

    #[tokio::test]
    #[ignore = "requires full Docker stack with bridges"]
    async fn test_signal_login_command() {
        let config = TestConfig::default();

        if !check_synapse_health(&config).await {
            panic!("Synapse is not running");
        }

        let username = format!("siglogin_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let (access_token, _, _) = register_test_user(&config, &username)
            .await
            .expect("Registration should succeed");

        let room_id = create_room(&config, &access_token, None)
            .await
            .expect("Room creation should succeed");

        let bot_id = format!("@signalbot:{}", config.domain);
        invite_to_room(&config, &access_token, &room_id, &bot_id)
            .await
            .expect("Invite should succeed");

        let bot_joined = wait_for_member(
            &config,
            &access_token,
            &room_id,
            &bot_id,
            Duration::from_secs(10),
        )
        .await;
        assert!(bot_joined, "Bot should join");

        // Send login command
        send_message(&config, &access_token, &room_id, "!signal login")
            .await
            .expect("Send command should succeed");

        sleep(Duration::from_secs(3)).await;

        let messages = get_room_messages(&config, &access_token, &room_id, 10)
            .await
            .expect("Should get messages");

        // Signal should respond with QR code or link
        let has_login_response = messages.iter().any(|m| {
            m["sender"].as_str() == Some(&bot_id)
                && (m["content"]["msgtype"] == "m.image"
                    || m["content"]["body"]
                        .as_str()
                        .map(|b| {
                            let lower = b.to_lowercase();
                            lower.contains("scan") || lower.contains("qr") || lower.contains("link")
                        })
                        .unwrap_or(false))
        });

        assert!(
            has_login_response,
            "Signal bot should respond to login command. Messages: {:?}",
            messages
        );
    }
}

// ============================================================================
// Telegram Bridge Tests
// ============================================================================

mod telegram_bridge_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires full Docker stack with bridges"]
    async fn test_telegram_bot_joins_room() {
        let config = TestConfig::default();

        if !check_synapse_health(&config).await {
            panic!("Synapse is not running");
        }

        let username = format!("tgtest_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let (access_token, _, _) = register_test_user(&config, &username)
            .await
            .expect("Registration should succeed");

        let room_id = create_room(&config, &access_token, Some("Telegram Test"))
            .await
            .expect("Room creation should succeed");

        let bot_id = format!("@telegrambot:{}", config.domain);
        invite_to_room(&config, &access_token, &room_id, &bot_id)
            .await
            .expect("Invite should succeed");

        let bot_joined = wait_for_member(
            &config,
            &access_token,
            &room_id,
            &bot_id,
            Duration::from_secs(10),
        )
        .await;

        assert!(bot_joined, "Telegram bot should join the room within 10s");
    }

    #[tokio::test]
    #[ignore = "requires full Docker stack with bridges"]
    async fn test_telegram_login_command() {
        let config = TestConfig::default();

        if !check_synapse_health(&config).await {
            panic!("Synapse is not running");
        }

        let username = format!("tglogin_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let (access_token, _, _) = register_test_user(&config, &username)
            .await
            .expect("Registration should succeed");

        let room_id = create_room(&config, &access_token, None)
            .await
            .expect("Room creation should succeed");

        let bot_id = format!("@telegrambot:{}", config.domain);
        invite_to_room(&config, &access_token, &room_id, &bot_id)
            .await
            .expect("Invite should succeed");

        let bot_joined = wait_for_member(
            &config,
            &access_token,
            &room_id,
            &bot_id,
            Duration::from_secs(10),
        )
        .await;
        assert!(bot_joined, "Bot should join");

        // Send login command
        send_message(&config, &access_token, &room_id, "!tg login")
            .await
            .expect("Send command should succeed");

        sleep(Duration::from_secs(3)).await;

        let messages = get_room_messages(&config, &access_token, &room_id, 10)
            .await
            .expect("Should get messages");

        // Telegram should respond with login URL or instructions
        let has_login_response = messages.iter().any(|m| {
            m["sender"].as_str() == Some(&bot_id)
                && m["content"]["body"]
                    .as_str()
                    .map(|b| {
                        let lower = b.to_lowercase();
                        lower.contains("login")
                            || lower.contains("url")
                            || lower.contains("telegram")
                            || lower.contains("phone")
                    })
                    .unwrap_or(false)
        });

        assert!(
            has_login_response,
            "Telegram bot should respond to login command. Messages: {:?}",
            messages
        );
    }
}

// ============================================================================
// Multi-Bridge Tests
// ============================================================================

mod multi_bridge_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires full Docker stack with bridges"]
    async fn test_all_bridges_can_join_same_user_rooms() {
        let config = TestConfig::default();

        if !check_synapse_health(&config).await {
            panic!("Synapse is not running");
        }

        let username = format!("multibot_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let (access_token, _, _) = register_test_user(&config, &username)
            .await
            .expect("Registration should succeed");

        let bots = [
            (format!("@whatsappbot:{}", config.domain), "WhatsApp"),
            (format!("@signalbot:{}", config.domain), "Signal"),
            (format!("@telegrambot:{}", config.domain), "Telegram"),
        ];

        for (bot_id, bot_name) in &bots {
            let room_id = create_room(&config, &access_token, Some(&format!("{} Room", bot_name)))
                .await
                .expect("Room creation should succeed");

            invite_to_room(&config, &access_token, &room_id, bot_id)
                .await
                .expect("Invite should succeed");

            let bot_joined = wait_for_member(
                &config,
                &access_token,
                &room_id,
                bot_id,
                Duration::from_secs(10),
            )
            .await;

            assert!(
                bot_joined,
                "{} bot should join its room within 10s",
                bot_name
            );
        }
    }
}
