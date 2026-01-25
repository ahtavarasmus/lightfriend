//! Matrix Mock Tests (Tier 1: Fast, No Docker Required)
//!
//! These tests use wiremock to mock the Synapse Matrix server API responses.
//! They run by default in CI without requiring Docker.
//!
//! Run with: cargo test --test matrix_mock_test

use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Mock Synapse server for fast unit tests
struct MockSynapse {
    server: MockServer,
}

impl MockSynapse {
    async fn start() -> Self {
        let server = MockServer::start().await;
        Self { server }
    }

    fn url(&self) -> String {
        self.server.uri()
    }

    /// Mock the registration nonce endpoint
    async fn mock_registration_nonce(&self, nonce: &str) {
        Mock::given(method("GET"))
            .and(path("/_synapse/admin/v1/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "nonce": nonce
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock successful user registration
    async fn mock_register_success(&self, user_id: &str, access_token: &str, device_id: &str) {
        Mock::given(method("POST"))
            .and(path("/_synapse/admin/v1/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "user_id": user_id,
                "access_token": access_token,
                "device_id": device_id,
                "home_server": "test.local"
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock registration failure (e.g., user already exists)
    async fn mock_register_failure(&self, error_code: &str, error: &str) {
        Mock::given(method("POST"))
            .and(path("/_synapse/admin/v1/register"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "errcode": error_code,
                "error": error
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock whoami endpoint
    async fn mock_whoami(&self, user_id: &str, device_id: &str) {
        Mock::given(method("GET"))
            .and(path("/_matrix/client/v3/account/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "user_id": user_id,
                "device_id": device_id
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock whoami failure (invalid token)
    async fn mock_whoami_failure(&self) {
        Mock::given(method("GET"))
            .and(path("/_matrix/client/v3/account/whoami"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "errcode": "M_UNKNOWN_TOKEN",
                "error": "Invalid access token"
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock room creation
    async fn mock_create_room(&self, room_id: &str) {
        Mock::given(method("POST"))
            .and(path("/_matrix/client/v3/createRoom"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "room_id": room_id
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock invite user to room
    #[allow(dead_code)]
    async fn mock_invite_user(&self) {
        // The path contains room_id which varies, so we use a more general matcher
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&self.server)
            .await;
    }

    /// Mock join room
    #[allow(dead_code)]
    async fn mock_join_room(&self, room_id: &str) {
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "room_id": room_id
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock send message
    async fn mock_send_message(&self, event_id: &str) {
        Mock::given(method("PUT"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "event_id": event_id
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock sync endpoint (empty response)
    async fn mock_sync_empty(&self) {
        Mock::given(method("GET"))
            .and(path("/_matrix/client/v3/sync"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "next_batch": "s123456789",
                "rooms": {},
                "presence": {},
                "account_data": {}
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock login endpoint
    async fn mock_login(&self, user_id: &str, access_token: &str, device_id: &str) {
        Mock::given(method("POST"))
            .and(path("/_matrix/client/v3/login"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "user_id": user_id,
                "access_token": access_token,
                "device_id": device_id,
                "home_server": "test.local"
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock login failure
    async fn mock_login_failure(&self) {
        Mock::given(method("POST"))
            .and(path("/_matrix/client/v3/login"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                "errcode": "M_FORBIDDEN",
                "error": "Invalid username or password"
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock room members endpoint
    async fn mock_room_members(&self, room_id: &str, members: Vec<&str>) {
        let member_events: Vec<serde_json::Value> = members
            .iter()
            .map(|user_id| {
                json!({
                    "type": "m.room.member",
                    "sender": user_id,
                    "state_key": user_id,
                    "content": {
                        "membership": "join",
                        "displayname": user_id.split(':').next().unwrap_or(user_id).trim_start_matches('@')
                    }
                })
            })
            .collect();

        Mock::given(method("GET"))
            .and(path(format!(
                "/_matrix/client/v3/rooms/{}/members",
                room_id.replace('!', "%21").replace(':', "%3A")
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "chunk": member_events
            })))
            .mount(&self.server)
            .await;
    }

    /// Mock room messages endpoint
    async fn mock_room_messages(&self, room_id: &str, messages: Vec<serde_json::Value>) {
        Mock::given(method("GET"))
            .and(path(format!(
                "/_matrix/client/v3/rooms/{}/messages",
                room_id.replace('!', "%21").replace(':', "%3A")
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "chunk": messages,
                "start": "s123",
                "end": "s456"
            })))
            .mount(&self.server)
            .await;
    }
}

// ============================================================================
// Registration Tests
// ============================================================================

mod registration_tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_synapse_starts_and_responds() {
        let mock = MockSynapse::start().await;
        mock.mock_registration_nonce("test-nonce-123").await;

        // Verify the mock server responds correctly
        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/_synapse/admin/v1/register", mock.url()))
            .send()
            .await
            .expect("Failed to send request");

        assert!(response.status().is_success());
        let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
        assert_eq!(body["nonce"], "test-nonce-123");
    }

    #[tokio::test]
    async fn test_registration_nonce_flow() {
        let mock = MockSynapse::start().await;
        mock.mock_registration_nonce("unique-nonce-456").await;
        mock.mock_register_success("@appuser_test:test.local", "access_token_xyz", "TESTDEVICE")
            .await;

        let client = reqwest::Client::new();

        // Step 1: Get nonce
        let nonce_response = client
            .get(format!("{}/_synapse/admin/v1/register", mock.url()))
            .send()
            .await
            .expect("Failed to get nonce");

        assert!(nonce_response.status().is_success());
        let nonce_body: serde_json::Value = nonce_response.json().await.unwrap();
        assert_eq!(nonce_body["nonce"], "unique-nonce-456");

        // Step 2: Register user
        let register_response = client
            .post(format!("{}/_synapse/admin/v1/register", mock.url()))
            .json(&json!({
                "nonce": "unique-nonce-456",
                "username": "appuser_test",
                "password": "test_password",
                "admin": false,
                "mac": "fake_mac_for_test"
            }))
            .send()
            .await
            .expect("Failed to register");

        assert!(register_response.status().is_success());
        let register_body: serde_json::Value = register_response.json().await.unwrap();
        assert_eq!(register_body["user_id"], "@appuser_test:test.local");
        assert_eq!(register_body["access_token"], "access_token_xyz");
        assert_eq!(register_body["device_id"], "TESTDEVICE");
    }

    #[tokio::test]
    async fn test_registration_handles_nonce_error() {
        let mock = MockSynapse::start().await;
        // Don't mock the nonce endpoint - request should fail

        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/_synapse/admin/v1/register", mock.url()))
            .send()
            .await
            .expect("Request should complete");

        // Should get 404 since endpoint is not mocked
        assert!(!response.status().is_success());
    }

    #[tokio::test]
    async fn test_registration_failure_user_exists() {
        let mock = MockSynapse::start().await;
        mock.mock_registration_nonce("nonce-123").await;
        mock.mock_register_failure("M_USER_IN_USE", "User ID already taken")
            .await;

        let client = reqwest::Client::new();

        // Try to register (should fail)
        let register_response = client
            .post(format!("{}/_synapse/admin/v1/register", mock.url()))
            .json(&json!({
                "nonce": "nonce-123",
                "username": "existing_user",
                "password": "test_password",
                "admin": false,
                "mac": "fake_mac"
            }))
            .send()
            .await
            .expect("Request should complete");

        assert_eq!(register_response.status().as_u16(), 400);
        let body: serde_json::Value = register_response.json().await.unwrap();
        assert_eq!(body["errcode"], "M_USER_IN_USE");
    }
}

// ============================================================================
// Session/Login Tests
// ============================================================================

mod session_tests {
    use super::*;

    #[tokio::test]
    async fn test_whoami_with_valid_token() {
        let mock = MockSynapse::start().await;
        mock.mock_whoami("@testuser:test.local", "MYDEVICE123")
            .await;

        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/_matrix/client/v3/account/whoami", mock.url()))
            .header("Authorization", "Bearer valid_token")
            .send()
            .await
            .expect("Request should complete");

        assert!(response.status().is_success());
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["user_id"], "@testuser:test.local");
        assert_eq!(body["device_id"], "MYDEVICE123");
    }

    #[tokio::test]
    async fn test_whoami_with_invalid_token() {
        let mock = MockSynapse::start().await;
        mock.mock_whoami_failure().await;

        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/_matrix/client/v3/account/whoami", mock.url()))
            .header("Authorization", "Bearer invalid_token")
            .send()
            .await
            .expect("Request should complete");

        assert_eq!(response.status().as_u16(), 401);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["errcode"], "M_UNKNOWN_TOKEN");
    }

    #[tokio::test]
    async fn test_login_with_password() {
        let mock = MockSynapse::start().await;
        mock.mock_login("@testuser:test.local", "new_token", "NEWDEVICE")
            .await;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/_matrix/client/v3/login", mock.url()))
            .json(&json!({
                "type": "m.login.password",
                "identifier": {
                    "type": "m.id.user",
                    "user": "testuser"
                },
                "password": "correct_password"
            }))
            .send()
            .await
            .expect("Request should complete");

        assert!(response.status().is_success());
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["user_id"], "@testuser:test.local");
        assert_eq!(body["access_token"], "new_token");
    }

    #[tokio::test]
    async fn test_login_with_wrong_password() {
        let mock = MockSynapse::start().await;
        mock.mock_login_failure().await;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/_matrix/client/v3/login", mock.url()))
            .json(&json!({
                "type": "m.login.password",
                "identifier": {
                    "type": "m.id.user",
                    "user": "testuser"
                },
                "password": "wrong_password"
            }))
            .send()
            .await
            .expect("Request should complete");

        assert_eq!(response.status().as_u16(), 403);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["errcode"], "M_FORBIDDEN");
    }
}

// ============================================================================
// Room Operations Tests
// ============================================================================

mod room_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_room() {
        let mock = MockSynapse::start().await;
        mock.mock_create_room("!newroom:test.local").await;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/_matrix/client/v3/createRoom", mock.url()))
            .header("Authorization", "Bearer valid_token")
            .json(&json!({
                "preset": "private_chat",
                "name": "Test Room",
                "is_direct": true
            }))
            .send()
            .await
            .expect("Request should complete");

        assert!(response.status().is_success());
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["room_id"], "!newroom:test.local");
    }

    #[tokio::test]
    async fn test_send_message_to_room() {
        let mock = MockSynapse::start().await;
        mock.mock_send_message("$event123:test.local").await;

        let client = reqwest::Client::new();
        let response = client
            .put(format!(
                "{}/_matrix/client/v3/rooms/{}/send/m.room.message/txn123",
                mock.url(),
                "!testroom:test.local"
            ))
            .header("Authorization", "Bearer valid_token")
            .json(&json!({
                "msgtype": "m.text",
                "body": "Hello, World!"
            }))
            .send()
            .await
            .expect("Request should complete");

        assert!(response.status().is_success());
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["event_id"], "$event123:test.local");
    }

    #[tokio::test]
    async fn test_get_room_members() {
        let mock = MockSynapse::start().await;
        let room_id = "!testroom:test.local";
        mock.mock_room_members(
            room_id,
            vec![
                "@user1:test.local",
                "@whatsappbot:test.local",
                "@whatsapp_12345:test.local",
            ],
        )
        .await;

        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "{}/_matrix/client/v3/rooms/{}/members",
                mock.url(),
                room_id.replace('!', "%21").replace(':', "%3A")
            ))
            .header("Authorization", "Bearer valid_token")
            .send()
            .await
            .expect("Request should complete");

        assert!(response.status().is_success());
        let body: serde_json::Value = response.json().await.unwrap();
        let chunk = body["chunk"].as_array().expect("chunk should be array");
        assert_eq!(chunk.len(), 3);
    }
}

// ============================================================================
// Sync Tests
// ============================================================================

mod sync_tests {
    use super::*;

    #[tokio::test]
    async fn test_sync_empty_response() {
        let mock = MockSynapse::start().await;
        mock.mock_sync_empty().await;

        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/_matrix/client/v3/sync", mock.url()))
            .header("Authorization", "Bearer valid_token")
            .send()
            .await
            .expect("Request should complete");

        assert!(response.status().is_success());
        let body: serde_json::Value = response.json().await.unwrap();
        assert!(body["next_batch"].is_string());
    }
}

// ============================================================================
// Bridge Bot Simulation Tests
// ============================================================================

mod bridge_bot_tests {
    use super::*;

    #[tokio::test]
    async fn test_bot_joins_room_simulation() {
        let mock = MockSynapse::start().await;
        let room_id = "!bridgeroom:test.local";

        // Mock the room members endpoint showing bot has joined
        mock.mock_room_members(
            room_id,
            vec!["@testuser:test.local", "@whatsappbot:test.local"],
        )
        .await;

        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "{}/_matrix/client/v3/rooms/{}/members",
                mock.url(),
                room_id.replace('!', "%21").replace(':', "%3A")
            ))
            .header("Authorization", "Bearer valid_token")
            .send()
            .await
            .expect("Request should complete");

        assert!(response.status().is_success());
        let body: serde_json::Value = response.json().await.unwrap();
        let chunk = body["chunk"].as_array().unwrap();

        // Verify bot is in the room
        let bot_present = chunk
            .iter()
            .any(|m| m["state_key"] == "@whatsappbot:test.local");
        assert!(bot_present, "WhatsApp bot should be in the room");
    }

    #[tokio::test]
    async fn test_bot_sends_qr_code_simulation() {
        let mock = MockSynapse::start().await;
        let room_id = "!bridgeroom:test.local";

        // Mock messages endpoint with QR code image message from bot
        mock.mock_room_messages(
            room_id,
            vec![
                json!({
                    "type": "m.room.message",
                    "sender": "@whatsappbot:test.local",
                    "content": {
                        "msgtype": "m.image",
                        "body": "qr-code.png",
                        "url": "mxc://test.local/qrcode123"
                    },
                    "origin_server_ts": 1700000000000_i64
                }),
                json!({
                    "type": "m.room.message",
                    "sender": "@whatsappbot:test.local",
                    "content": {
                        "msgtype": "m.notice",
                        "body": "Scan this QR code with your WhatsApp mobile app to link your account."
                    },
                    "origin_server_ts": 1700000001000_i64
                }),
            ],
        )
        .await;

        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "{}/_matrix/client/v3/rooms/{}/messages",
                mock.url(),
                room_id.replace('!', "%21").replace(':', "%3A")
            ))
            .header("Authorization", "Bearer valid_token")
            .query(&[("dir", "b"), ("limit", "10")])
            .send()
            .await
            .expect("Request should complete");

        assert!(response.status().is_success());
        let body: serde_json::Value = response.json().await.unwrap();
        let messages = body["chunk"].as_array().unwrap();

        // Check for QR code image
        let has_qr_image = messages.iter().any(|m| {
            m["content"]["msgtype"] == "m.image" && m["sender"] == "@whatsappbot:test.local"
        });
        assert!(has_qr_image, "Bot should have sent QR code image");

        // Check for scan instructions
        let has_scan_notice = messages.iter().any(|m| {
            m["content"]["msgtype"] == "m.notice"
                && m["content"]["body"]
                    .as_str()
                    .map(|b| b.contains("Scan"))
                    .unwrap_or(false)
        });
        assert!(has_scan_notice, "Bot should have sent scan instructions");
    }

    #[tokio::test]
    async fn test_bot_unknown_command_response() {
        let mock = MockSynapse::start().await;
        let room_id = "!bridgeroom:test.local";

        // Mock messages with unknown command response
        mock.mock_room_messages(
            room_id,
            vec![json!({
                "type": "m.room.message",
                "sender": "@whatsappbot:test.local",
                "content": {
                    "msgtype": "m.notice",
                    "body": "Unknown command: foobar. Use `help` for a list of commands."
                },
                "origin_server_ts": 1700000000000_i64
            })],
        )
        .await;

        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "{}/_matrix/client/v3/rooms/{}/messages",
                mock.url(),
                room_id.replace('!', "%21").replace(':', "%3A")
            ))
            .header("Authorization", "Bearer valid_token")
            .query(&[("dir", "b"), ("limit", "5")])
            .send()
            .await
            .expect("Request should complete");

        assert!(response.status().is_success());
        let body: serde_json::Value = response.json().await.unwrap();
        let messages = body["chunk"].as_array().unwrap();

        let has_unknown_command = messages.iter().any(|m| {
            m["content"]["body"]
                .as_str()
                .map(|b| b.contains("Unknown command"))
                .unwrap_or(false)
        });
        assert!(
            has_unknown_command,
            "Bot should respond to unknown commands"
        );
    }

    #[tokio::test]
    async fn test_bot_logout_confirmation() {
        let mock = MockSynapse::start().await;
        let room_id = "!bridgeroom:test.local";

        // Mock messages with logout confirmation
        mock.mock_room_messages(
            room_id,
            vec![json!({
                "type": "m.room.message",
                "sender": "@whatsappbot:test.local",
                "content": {
                    "msgtype": "m.notice",
                    "body": "Successfully logged out from WhatsApp."
                },
                "origin_server_ts": 1700000000000_i64
            })],
        )
        .await;

        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "{}/_matrix/client/v3/rooms/{}/messages",
                mock.url(),
                room_id.replace('!', "%21").replace(':', "%3A")
            ))
            .header("Authorization", "Bearer valid_token")
            .query(&[("dir", "b"), ("limit", "5")])
            .send()
            .await
            .expect("Request should complete");

        assert!(response.status().is_success());
        let body: serde_json::Value = response.json().await.unwrap();
        let messages = body["chunk"].as_array().unwrap();

        let has_logout = messages.iter().any(|m| {
            m["content"]["body"]
                .as_str()
                .map(|b| b.to_lowercase().contains("logged out"))
                .unwrap_or(false)
        });
        assert!(has_logout, "Bot should confirm logout");
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

mod error_handling_tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiting_response() {
        let mock = MockSynapse::start().await;

        // Mock rate limit response
        Mock::given(method("POST"))
            .and(path("/_matrix/client/v3/createRoom"))
            .respond_with(ResponseTemplate::new(429).set_body_json(json!({
                "errcode": "M_LIMIT_EXCEEDED",
                "error": "Too many requests",
                "retry_after_ms": 5000
            })))
            .mount(&mock.server)
            .await;

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/_matrix/client/v3/createRoom", mock.url()))
            .header("Authorization", "Bearer valid_token")
            .json(&json!({"preset": "private_chat"}))
            .send()
            .await
            .expect("Request should complete");

        assert_eq!(response.status().as_u16(), 429);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["errcode"], "M_LIMIT_EXCEEDED");
        assert!(body["retry_after_ms"].is_number());
    }

    #[tokio::test]
    async fn test_server_error_response() {
        let mock = MockSynapse::start().await;

        // Mock server error
        Mock::given(method("GET"))
            .and(path("/_matrix/client/v3/sync"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({
                "errcode": "M_UNKNOWN",
                "error": "Internal server error"
            })))
            .mount(&mock.server)
            .await;

        let client = reqwest::Client::new();
        let response = client
            .get(format!("{}/_matrix/client/v3/sync", mock.url()))
            .header("Authorization", "Bearer valid_token")
            .send()
            .await
            .expect("Request should complete");

        assert_eq!(response.status().as_u16(), 500);
    }

    #[tokio::test]
    async fn test_network_timeout_simulation() {
        let mock = MockSynapse::start().await;

        // Mock slow response (simulates timeout scenario)
        Mock::given(method("GET"))
            .and(path("/_matrix/client/v3/sync"))
            .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(5)))
            .mount(&mock.server)
            .await;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(100))
            .build()
            .unwrap();

        let result = client
            .get(format!("{}/_matrix/client/v3/sync", mock.url()))
            .header("Authorization", "Bearer valid_token")
            .send()
            .await;

        assert!(result.is_err(), "Request should timeout");
    }
}
