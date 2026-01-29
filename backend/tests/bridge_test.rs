//! Integration tests for bridge functionality using trait-based mocks.
//!
//! These tests verify the testable bridge functions without requiring
//! a real Matrix server connection. Includes:
//! - Pure function tests (fast, no I/O)
//! - End-to-end workflow tests using mocks (fast, realistic data flows)

use backend::api::matrix_client::{
    infer_service_from_room, is_disconnection_message, is_error_message, is_health_check_message,
    should_process_message,
};

// ============================================================================
// Pure Function Tests
// ============================================================================

mod pure_function_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_should_process_message_accepts_recent() {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // 1 second ago should be processed with 30 min window
        assert!(should_process_message(now_ms - 1000, 30 * 60 * 1000));

        // 5 minutes ago should be processed
        assert!(should_process_message(
            now_ms - 5 * 60 * 1000,
            30 * 60 * 1000
        ));

        // 29 minutes ago should still be processed
        assert!(should_process_message(
            now_ms - 29 * 60 * 1000,
            30 * 60 * 1000
        ));
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

        // 1 hour ago should definitely not be processed
        assert!(!should_process_message(
            now_ms - 60 * 60 * 1000,
            30 * 60 * 1000
        ));
    }

    #[test]
    fn test_should_process_message_handles_edge_cases() {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Message from the future (clock skew)
        assert!(should_process_message(now_ms + 1000, 30 * 60 * 1000));

        // Exactly at the boundary
        assert!(should_process_message(
            now_ms - 30 * 60 * 1000,
            30 * 60 * 1000
        ));
    }

    #[test]
    fn test_is_disconnection_message_detects_patterns() {
        // Various disconnection patterns
        assert!(is_disconnection_message("Device has been disconnected"));
        assert!(is_disconnection_message("Connection lost to server"));
        assert!(is_disconnection_message("You have been logged out"));
        assert!(is_disconnection_message("Authentication failed"));
        assert!(is_disconnection_message("Login failed: bad_credentials"));
        assert!(is_disconnection_message("Session timeout occurred"));
        assert!(is_disconnection_message("Invalid token"));
        assert!(is_disconnection_message("wa-logged-out event received"));
        assert!(is_disconnection_message("wa-not-logged-in state"));
        assert!(is_disconnection_message(
            "Your device has been device_removed"
        ));
        assert!(is_disconnection_message(
            "Please relogin to continue using the service"
        ));
    }

    #[test]
    fn test_is_disconnection_message_ignores_normal_messages() {
        assert!(!is_disconnection_message("Hello, how are you?"));
        assert!(!is_disconnection_message("Successfully logged in"));
        assert!(!is_disconnection_message("Message delivered"));
        assert!(!is_disconnection_message("Connected to server"));
        assert!(!is_disconnection_message("Your friend John is online"));
    }

    #[test]
    fn test_is_disconnection_message_case_insensitive() {
        assert!(is_disconnection_message("DISCONNECTED"));
        assert!(is_disconnection_message("Disconnected"));
        assert!(is_disconnection_message("LOGGED OUT"));
        assert!(is_disconnection_message("Logged Out"));
    }

    #[test]
    fn test_is_health_check_message_detects_patterns() {
        assert!(is_health_check_message("Already logged in"));
        assert!(is_health_check_message(
            "You are already logged in to WhatsApp"
        ));
        assert!(is_health_check_message("Successfully logged in"));
        assert!(is_health_check_message(
            "Successfully logged in to your account"
        ));
        assert!(is_health_check_message("Queued sync operation"));
        assert!(is_health_check_message("Unknown command: help"));
    }

    #[test]
    fn test_is_health_check_message_ignores_regular_messages() {
        assert!(!is_health_check_message("Hello from John"));
        assert!(!is_health_check_message("New message received"));
        assert!(!is_health_check_message("File shared"));
        assert!(!is_health_check_message("Call ended"));
    }

    #[test]
    fn test_is_error_message_detects_bridge_errors() {
        assert!(is_error_message("Failed to bridge media: timeout"));
        assert!(is_error_message(
            "The media no longer available on the server"
        ));
        assert!(is_error_message("Decrypting message from WhatsApp failed"));
        assert!(is_error_message("* Failed to send message"));
        assert!(is_error_message("* Failed to bridge attachment"));
    }

    #[test]
    fn test_is_error_message_ignores_normal_messages() {
        assert!(!is_error_message("Hello!"));
        assert!(!is_error_message("Check out this image"));
        assert!(!is_error_message("Here is the document"));
        assert!(!is_error_message("Call me when you can"));
    }

    #[test]
    fn test_infer_service_from_room_whatsapp() {
        // Sender localpart patterns (room name no longer used for detection)
        assert_eq!(
            infer_service_from_room("Some Room", "whatsapp_1234567890"),
            Some("whatsapp".to_string())
        );
        assert_eq!(
            infer_service_from_room("Some Room", "whatsapp"),
            Some("whatsapp".to_string())
        );
        // Room name alone should NOT detect service
        assert_eq!(infer_service_from_room("John Smith (WA)", "user123"), None);
    }

    #[test]
    fn test_infer_service_from_room_telegram() {
        // Sender localpart patterns
        assert_eq!(
            infer_service_from_room("Some Room", "telegram_1234567890"),
            Some("telegram".to_string())
        );
        assert_eq!(
            infer_service_from_room("Some Room", "telegram"),
            Some("telegram".to_string())
        );
        // Room name alone should NOT detect service
        assert_eq!(infer_service_from_room("Work Chat (TG)", "user123"), None);
    }

    #[test]
    fn test_infer_service_from_room_signal() {
        // Sender localpart patterns
        assert_eq!(
            infer_service_from_room("Some Room", "signal_1234567890"),
            Some("signal".to_string())
        );
        assert_eq!(
            infer_service_from_room("Some Room", "signal"),
            Some("signal".to_string())
        );
        // Room name alone should NOT detect service
        assert_eq!(
            infer_service_from_room("Signal Chat with John", "user123"),
            None
        );
    }

    #[test]
    fn test_infer_service_from_room_messenger() {
        // Sender localpart patterns
        assert_eq!(
            infer_service_from_room("Some Room", "messenger_1234567890"),
            Some("messenger".to_string())
        );
        assert_eq!(
            infer_service_from_room("Some Room", "messenger"),
            Some("messenger".to_string())
        );
    }

    #[test]
    fn test_infer_service_from_room_instagram() {
        // Sender localpart patterns
        assert_eq!(
            infer_service_from_room("Some Room", "instagram_1234567890"),
            Some("instagram".to_string())
        );
        assert_eq!(
            infer_service_from_room("Some Room", "instagram"),
            Some("instagram".to_string())
        );
    }

    #[test]
    fn test_infer_service_from_room_returns_none_for_unknown() {
        assert_eq!(infer_service_from_room("Random Room", "random_user"), None);
        assert_eq!(infer_service_from_room("Work Meeting", "john_doe"), None);
        assert_eq!(infer_service_from_room("Family Chat", "mom"), None);
        // Room names with service patterns should NOT match without matching sender
        assert_eq!(infer_service_from_room("John (WA)", "regular_user"), None);
        assert_eq!(infer_service_from_room("Chat (TG)", "regular_user"), None);
    }

    #[test]
    fn test_infer_service_exact_match_no_underscore() {
        // Test exact service name match (no underscore)
        assert_eq!(
            infer_service_from_room("Room", "whatsapp"),
            Some("whatsapp".to_string())
        );
        assert_eq!(
            infer_service_from_room("Room", "telegram"),
            Some("telegram".to_string())
        );
        assert_eq!(
            infer_service_from_room("Room", "signal"),
            Some("signal".to_string())
        );
        assert_eq!(
            infer_service_from_room("Room", "messenger"),
            Some("messenger".to_string())
        );
        assert_eq!(
            infer_service_from_room("Room", "instagram"),
            Some("instagram".to_string())
        );
    }

    #[test]
    fn test_infer_service_case_insensitive() {
        assert_eq!(
            infer_service_from_room("Room", "WHATSAPP_123"),
            Some("whatsapp".to_string())
        );
        assert_eq!(
            infer_service_from_room("Room", "WhatsApp_456"),
            Some("whatsapp".to_string())
        );
        assert_eq!(
            infer_service_from_room("Room", "TELEGRAM"),
            Some("telegram".to_string())
        );
    }

    #[test]
    fn test_infer_service_with_whitespace() {
        assert_eq!(
            infer_service_from_room("Room", "  whatsapp_123  "),
            Some("whatsapp".to_string())
        );
        assert_eq!(
            infer_service_from_room("Room", "\ttelegram_456\n"),
            Some("telegram".to_string())
        );
    }
}

// ============================================================================
// IncomingMessageContent Tests
// ============================================================================

mod incoming_message_content_tests {
    use backend::api::matrix_client::IncomingMessageContent;

    #[test]
    fn test_body_extraction() {
        let text = IncomingMessageContent::Text {
            body: "Hello world".to_string(),
        };
        assert_eq!(text.body(), Some("Hello world"));

        let notice = IncomingMessageContent::Notice {
            body: "System notice".to_string(),
        };
        assert_eq!(notice.body(), Some("System notice"));

        let image = IncomingMessageContent::Image {
            body: "photo.jpg".to_string(),
            url: Some("mxc://server/media".to_string()),
        };
        assert_eq!(image.body(), Some("photo.jpg"));

        let location = IncomingMessageContent::Location;
        assert_eq!(location.body(), Some("[Location]"));

        let other = IncomingMessageContent::Other;
        assert_eq!(other.body(), None);
    }

    #[test]
    fn test_message_type_str() {
        assert_eq!(
            IncomingMessageContent::Text {
                body: String::new()
            }
            .message_type_str(),
            "text"
        );
        assert_eq!(
            IncomingMessageContent::Notice {
                body: String::new()
            }
            .message_type_str(),
            "notice"
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
            IncomingMessageContent::Video {
                body: String::new(),
                url: None
            }
            .message_type_str(),
            "video"
        );
        assert_eq!(
            IncomingMessageContent::Audio {
                body: String::new(),
                url: None
            }
            .message_type_str(),
            "audio"
        );
        assert_eq!(
            IncomingMessageContent::File {
                body: String::new(),
                url: None
            }
            .message_type_str(),
            "file"
        );
        assert_eq!(
            IncomingMessageContent::Location.message_type_str(),
            "location"
        );
        assert_eq!(
            IncomingMessageContent::Emote {
                body: String::new()
            }
            .message_type_str(),
            "emote"
        );
        assert_eq!(IncomingMessageContent::Other.message_type_str(), "other");
    }
}

// ============================================================================
// Bridge Helper Function Tests
// ============================================================================

mod bridge_helper_tests {
    use backend::utils::bridge::{
        find_exact_room, get_best_matches, remove_bridge_suffix, search_best_match, BridgeRoom,
    };

    fn create_test_rooms() -> Vec<BridgeRoom> {
        vec![
            BridgeRoom {
                room_id: "!room1:server".to_string(),
                display_name: "John Smith (WA)".to_string(),
                last_activity: 1000,
                last_activity_formatted: "2024-01-01 10:00:00".to_string(),
            },
            BridgeRoom {
                room_id: "!room2:server".to_string(),
                display_name: "Jane Doe (WA)".to_string(),
                last_activity: 2000,
                last_activity_formatted: "2024-01-01 11:00:00".to_string(),
            },
            BridgeRoom {
                room_id: "!room3:server".to_string(),
                display_name: "Family Group (WA)".to_string(),
                last_activity: 3000,
                last_activity_formatted: "2024-01-01 12:00:00".to_string(),
            },
            BridgeRoom {
                room_id: "!room4:server".to_string(),
                display_name: "Work Chat (TG)".to_string(),
                last_activity: 4000,
                last_activity_formatted: "2024-01-01 13:00:00".to_string(),
            },
        ]
    }

    #[test]
    fn test_remove_bridge_suffix_whatsapp() {
        assert_eq!(remove_bridge_suffix("John Smith (WA)"), "John Smith");
        assert_eq!(remove_bridge_suffix("Family Group (WA)"), "Family Group");
    }

    #[test]
    fn test_remove_bridge_suffix_telegram() {
        assert_eq!(remove_bridge_suffix("John Smith (Telegram)"), "John Smith");
        assert_eq!(remove_bridge_suffix("Work Chat (Telegram)"), "Work Chat");
    }

    #[test]
    fn test_remove_bridge_suffix_no_suffix() {
        assert_eq!(remove_bridge_suffix("Regular Room"), "Regular Room");
        assert_eq!(remove_bridge_suffix("No Suffix"), "No Suffix");
    }

    #[test]
    fn test_find_exact_room_matches() {
        let rooms = create_test_rooms();

        // Exact match (case insensitive, suffix removed)
        let result = find_exact_room(&rooms, "John Smith");
        assert!(result.is_some());
        assert_eq!(result.unwrap().room_id, "!room1:server");

        let result = find_exact_room(&rooms, "jane doe");
        assert!(result.is_some());
        assert_eq!(result.unwrap().room_id, "!room2:server");
    }

    #[test]
    fn test_find_exact_room_no_match() {
        let rooms = create_test_rooms();

        let result = find_exact_room(&rooms, "Unknown Person");
        assert!(result.is_none());

        // Partial match should not work for exact search
        let result = find_exact_room(&rooms, "John");
        assert!(result.is_none());
    }

    #[test]
    fn test_search_best_match_exact() {
        let rooms = create_test_rooms();

        let result = search_best_match(&rooms, "John Smith");
        assert!(result.is_some());
        assert_eq!(result.unwrap().room_id, "!room1:server");
    }

    #[test]
    fn test_search_best_match_substring() {
        let rooms = create_test_rooms();

        // Should find rooms containing "Family"
        let result = search_best_match(&rooms, "Family");
        assert!(result.is_some());
        assert_eq!(result.unwrap().display_name, "Family Group (WA)");
    }

    #[test]
    fn test_search_best_match_fuzzy() {
        let rooms = create_test_rooms();

        // Should find "John Smith" even with typo
        let result = search_best_match(&rooms, "Jon Smith");
        assert!(result.is_some());
        assert_eq!(result.unwrap().display_name, "John Smith (WA)");
    }

    #[test]
    fn test_get_best_matches() {
        let rooms = create_test_rooms();

        // Should return similar matches (need a closer match for jaro-winkler >= 0.7)
        let matches = get_best_matches(&rooms, "John");
        assert!(!matches.is_empty());
        // "John Smith" should be in the results
        assert!(matches.iter().any(|m| m.contains("John")));
    }

    #[test]
    fn test_get_best_matches_limits_to_5() {
        // Create more than 5 similar rooms
        let rooms: Vec<BridgeRoom> = (0..10)
            .map(|i| BridgeRoom {
                room_id: format!("!room{}:server", i),
                display_name: format!("Test Room {} (WA)", i),
                last_activity: i as i64,
                last_activity_formatted: "".to_string(),
            })
            .collect();

        let matches = get_best_matches(&rooms, "Test Room");
        assert!(matches.len() <= 5);
    }
}

// ============================================================================
// E2E Workflow Tests (using mocks)
// ============================================================================

mod workflow_tests {
    use backend::api::matrix_client::{MockMatrixClient, MockRoom, RoomMember};
    use backend::utils::bridge::{
        fetch_bridge_messages_trait, get_service_rooms_trait, send_bridge_message_trait,
        BridgeMessage,
    };

    // Helper to create a mock room member
    fn member(localpart: &str) -> RoomMember {
        RoomMember {
            user_id: format!("@{}:server.com", localpart),
            localpart: localpart.to_string(),
        }
    }

    // Helper to create a test message
    fn test_message(sender: &str, content: &str, timestamp: i64) -> BridgeMessage {
        BridgeMessage {
            sender: format!("@{}:server.com", sender),
            sender_display_name: sender.to_string(),
            content: content.to_string(),
            timestamp,
            formatted_timestamp: "".to_string(),
            message_type: "text".to_string(),
            room_name: "".to_string(),
            media_url: None,
        }
    }

    // -----------------------------------------------------------------------
    // get_service_rooms_trait tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_get_service_rooms_filters_by_service() {
        // Create rooms with different services
        let whatsapp_room = MockRoom::new("!wa1:server", "John Smith (WA)")
            .with_members(vec![member("whatsapp_123"), member("me")])
            .with_last_activity(1000);

        let telegram_room = MockRoom::new("!tg1:server", "Work Chat (TG)")
            .with_members(vec![member("telegram_456"), member("me")])
            .with_last_activity(2000);

        let client = MockMatrixClient::new().with_rooms(vec![whatsapp_room, telegram_room]);

        // Should only return WhatsApp rooms
        let wa_rooms = get_service_rooms_trait(&client, "whatsapp").await.unwrap();
        assert_eq!(wa_rooms.len(), 1);
        assert_eq!(wa_rooms[0].display_name, "John Smith (WA)");

        // Should only return Telegram rooms
        let tg_rooms = get_service_rooms_trait(&client, "telegram").await.unwrap();
        assert_eq!(tg_rooms.len(), 1);
        assert_eq!(tg_rooms[0].display_name, "Work Chat (TG)");
    }

    #[tokio::test]
    async fn test_get_service_rooms_skips_management_rooms() {
        // Create user room and management/bot rooms
        // Note: skip_terms use capitalize("whatsapp") = "Whatsapp", not "WhatsApp"
        let user_room = MockRoom::new("!wa1:server", "John Smith (WA)")
            .with_members(vec![member("whatsapp_123"), member("me")])
            .with_last_activity(1000);

        let bot_room = MockRoom::new("!bot:server", "whatsappbot status")
            .with_members(vec![member("whatsapp_bot")])
            .with_last_activity(2000);

        let bridge_room = MockRoom::new("!bridge:server", "Whatsapp Bridge")
            .with_members(vec![member("whatsapp_bridge")])
            .with_last_activity(3000);

        let client = MockMatrixClient::new().with_rooms(vec![user_room, bot_room, bridge_room]);

        let rooms = get_service_rooms_trait(&client, "whatsapp").await.unwrap();

        // Should only return user room, not bot/bridge rooms
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].display_name, "John Smith (WA)");
    }

    #[tokio::test]
    async fn test_get_service_rooms_sorts_by_activity() {
        let old_room = MockRoom::new("!old:server", "Old Chat (WA)")
            .with_members(vec![member("whatsapp_1"), member("me")])
            .with_last_activity(1000);

        let new_room = MockRoom::new("!new:server", "New Chat (WA)")
            .with_members(vec![member("whatsapp_2"), member("me")])
            .with_last_activity(5000);

        let mid_room = MockRoom::new("!mid:server", "Mid Chat (WA)")
            .with_members(vec![member("whatsapp_3"), member("me")])
            .with_last_activity(3000);

        let client = MockMatrixClient::new().with_rooms(vec![old_room, new_room, mid_room]);

        let rooms = get_service_rooms_trait(&client, "whatsapp").await.unwrap();

        assert_eq!(rooms.len(), 3);
        // Should be sorted by last_activity descending (most recent first)
        assert_eq!(rooms[0].display_name, "New Chat (WA)");
        assert_eq!(rooms[1].display_name, "Mid Chat (WA)");
        assert_eq!(rooms[2].display_name, "Old Chat (WA)");
    }

    #[tokio::test]
    async fn test_get_service_rooms_requires_service_member() {
        // Room without service member should be excluded
        let no_service_room = MockRoom::new("!no:server", "Random Chat (WA)")
            .with_members(vec![member("random_user"), member("me")])
            .with_last_activity(1000);

        let with_service_room = MockRoom::new("!with:server", "John (WA)")
            .with_members(vec![member("whatsapp_123"), member("me")])
            .with_last_activity(2000);

        let client = MockMatrixClient::new().with_rooms(vec![no_service_room, with_service_room]);

        let rooms = get_service_rooms_trait(&client, "whatsapp").await.unwrap();

        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].display_name, "John (WA)");
    }

    #[tokio::test]
    async fn test_get_service_rooms_empty_when_no_rooms() {
        let client = MockMatrixClient::new();
        let rooms = get_service_rooms_trait(&client, "whatsapp").await.unwrap();
        assert!(rooms.is_empty());
    }

    // -----------------------------------------------------------------------
    // send_bridge_message_trait tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_send_message_finds_room_and_sends_text() {
        let room = MockRoom::new("!wa1:server", "John Smith (WA)")
            .with_members(vec![member("whatsapp_123"), member("me")])
            .with_last_activity(1000);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        let result =
            send_bridge_message_trait(&client, "whatsapp", "John Smith", "Hello!", None, None)
                .await;

        assert!(result.is_ok());
        let msg = result.unwrap();
        assert_eq!(msg.content, "Hello!");
        assert_eq!(msg.sender, "You");
        assert_eq!(msg.message_type, "text");
        assert_eq!(msg.room_name, "John Smith (WA)");
    }

    #[tokio::test]
    async fn test_send_message_room_not_found_returns_error() {
        let room = MockRoom::new("!wa1:server", "John Smith (WA)")
            .with_members(vec![member("whatsapp_123"), member("me")])
            .with_last_activity(1000);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        // Try to send to a room that doesn't exist
        let result =
            send_bridge_message_trait(&client, "whatsapp", "Unknown Person", "Hello!", None, None)
                .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Could not find exact matching"));
    }

    #[tokio::test]
    async fn test_send_message_suggests_similar_rooms() {
        let room = MockRoom::new("!wa1:server", "Jonathan Smith (WA)")
            .with_members(vec![member("whatsapp_123"), member("me")])
            .with_last_activity(1000);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        // Search for "John Smith" but only "Jonathan Smith" exists
        let result =
            send_bridge_message_trait(&client, "whatsapp", "John Smith", "Hello!", None, None)
                .await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        // Should suggest similar room
        assert!(err.contains("Jonathan Smith") || err.contains("Did you mean"));
    }

    #[tokio::test]
    async fn test_send_message_case_insensitive_room_match() {
        let room = MockRoom::new("!wa1:server", "John Smith (WA)")
            .with_members(vec![member("whatsapp_123"), member("me")])
            .with_last_activity(1000);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        // Should match case-insensitively
        let result =
            send_bridge_message_trait(&client, "whatsapp", "john smith", "Hello!", None, None)
                .await;

        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // fetch_bridge_messages_trait tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_fetch_messages_filters_by_timestamp() {
        let now = chrono::Utc::now().timestamp();
        let old_msg = test_message("whatsapp_123", "Old message", now - 7200); // 2 hours ago
        let new_msg = test_message("whatsapp_123", "New message", now - 60); // 1 minute ago

        let room = MockRoom::new("!wa1:server", "John Smith (WA)")
            .with_members(vec![member("whatsapp_123"), member("me")])
            .with_last_activity(now)
            .with_messages(vec![old_msg, new_msg]);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        // Fetch messages from last hour
        let start_time = now - 3600;
        let messages = fetch_bridge_messages_trait(&client, "whatsapp", start_time, None)
            .await
            .unwrap();

        // Should only include the new message
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "New message");
    }

    #[tokio::test]
    async fn test_fetch_messages_skips_muted_rooms() {
        let now = chrono::Utc::now().timestamp();
        let msg = test_message("whatsapp_123", "Hello", now);

        let muted_room = MockRoom::new("!muted:server", "Muted Chat (WA)")
            .with_members(vec![member("whatsapp_1"), member("me")])
            .with_last_activity(now)
            .with_messages(vec![msg.clone()])
            .with_muted(true);

        let unmuted_room = MockRoom::new("!unmuted:server", "Active Chat (WA)")
            .with_members(vec![member("whatsapp_2"), member("me")])
            .with_last_activity(now)
            .with_messages(vec![msg]);

        let client = MockMatrixClient::new().with_rooms(vec![muted_room, unmuted_room]);

        let start_time = now - 3600;
        let messages = fetch_bridge_messages_trait(&client, "whatsapp", start_time, None)
            .await
            .unwrap();

        // Should only include messages from unmuted room
        assert_eq!(messages.len(), 1);
        // Messages should be from unmuted room (room_name gets set from display_name)
    }

    #[tokio::test]
    async fn test_fetch_messages_limits_to_top_5_rooms() {
        let now = chrono::Utc::now().timestamp();

        // Create 7 rooms with different activity levels
        let rooms: Vec<MockRoom> = (0..7)
            .map(|i| {
                let msg = test_message(&format!("whatsapp_{}", i), &format!("Message {}", i), now);
                MockRoom::new(&format!("!room{}:server", i), &format!("Room {} (WA)", i))
                    .with_members(vec![member(&format!("whatsapp_{}", i)), member("me")])
                    .with_last_activity(now - (i as i64 * 100)) // Different activity times
                    .with_messages(vec![msg])
            })
            .collect();

        let client = MockMatrixClient::new().with_rooms(rooms);

        let start_time = now - 3600;
        let messages = fetch_bridge_messages_trait(&client, "whatsapp", start_time, None)
            .await
            .unwrap();

        // Should process at most 5 rooms (as per the function implementation)
        assert!(messages.len() <= 5);
    }

    #[tokio::test]
    async fn test_fetch_messages_sorts_by_timestamp_descending() {
        let now = chrono::Utc::now().timestamp();

        let msg1 = test_message("whatsapp_1", "First", now - 300); // 5 min ago
        let msg2 = test_message("whatsapp_1", "Second", now - 60); // 1 min ago
        let msg3 = test_message("whatsapp_1", "Third", now - 180); // 3 min ago

        let room = MockRoom::new("!wa1:server", "John (WA)")
            .with_members(vec![member("whatsapp_1"), member("me")])
            .with_last_activity(now)
            .with_messages(vec![msg1, msg2, msg3]);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        let start_time = now - 3600;
        let messages = fetch_bridge_messages_trait(&client, "whatsapp", start_time, None)
            .await
            .unwrap();

        assert_eq!(messages.len(), 3);
        // Should be sorted by timestamp descending (most recent first)
        assert_eq!(messages[0].content, "Second");
        assert_eq!(messages[1].content, "Third");
        assert_eq!(messages[2].content, "First");
    }

    #[tokio::test]
    async fn test_fetch_messages_empty_when_no_messages() {
        let now = chrono::Utc::now().timestamp();

        let room = MockRoom::new("!wa1:server", "John (WA)")
            .with_members(vec![member("whatsapp_1"), member("me")])
            .with_last_activity(now);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        let start_time = now - 3600;
        let messages = fetch_bridge_messages_trait(&client, "whatsapp", start_time, None)
            .await
            .unwrap();

        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_messages_formats_room_name_without_suffix() {
        let now = chrono::Utc::now().timestamp();
        let msg = test_message("whatsapp_1", "Hello", now);

        let room = MockRoom::new("!wa1:server", "John Smith (WA)")
            .with_members(vec![member("whatsapp_1"), member("me")])
            .with_last_activity(now)
            .with_messages(vec![msg]);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        let start_time = now - 3600;
        let messages = fetch_bridge_messages_trait(&client, "whatsapp", start_time, None)
            .await
            .unwrap();

        assert_eq!(messages.len(), 1);
        // Room name should have suffix removed
        assert_eq!(messages[0].room_name, "John Smith");
    }

    // -----------------------------------------------------------------------
    // Integration scenarios
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_full_conversation_workflow() {
        let now = chrono::Utc::now().timestamp();

        // Simulate an existing conversation
        let existing_messages = vec![
            test_message("whatsapp_friend", "Hey, are you free?", now - 300),
            test_message("whatsapp_friend", "Want to grab coffee?", now - 60),
        ];

        let room = MockRoom::new("!friend:server", "Best Friend (WA)")
            .with_members(vec![member("whatsapp_friend"), member("me")])
            .with_last_activity(now)
            .with_messages(existing_messages);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        // Step 1: Fetch recent messages
        let messages = fetch_bridge_messages_trait(&client, "whatsapp", now - 3600, None)
            .await
            .unwrap();
        assert_eq!(messages.len(), 2);
        // Messages should be sorted by timestamp descending
        assert_eq!(messages[0].content, "Want to grab coffee?");
        assert_eq!(messages[1].content, "Hey, are you free?");

        // Step 2: Reply to the conversation
        let reply = send_bridge_message_trait(
            &client,
            "whatsapp",
            "Best Friend",
            "Sure! See you at 3pm",
            None,
            None,
        )
        .await;
        assert!(reply.is_ok());
        let reply = reply.unwrap();
        assert_eq!(reply.content, "Sure! See you at 3pm");
        assert_eq!(reply.room_name, "Best Friend (WA)");
    }

    #[tokio::test]
    async fn test_multi_service_scenario() {
        let now = chrono::Utc::now().timestamp();

        let wa_msg = test_message("whatsapp_john", "WhatsApp message", now);
        let tg_msg = test_message("telegram_jane", "Telegram message", now);

        let wa_room = MockRoom::new("!wa:server", "John (WA)")
            .with_members(vec![member("whatsapp_john"), member("me")])
            .with_last_activity(now)
            .with_messages(vec![wa_msg]);

        let tg_room = MockRoom::new("!tg:server", "Jane (TG)")
            .with_members(vec![member("telegram_jane"), member("me")])
            .with_last_activity(now)
            .with_messages(vec![tg_msg]);

        let client = MockMatrixClient::new().with_rooms(vec![wa_room, tg_room]);

        // Fetch WhatsApp messages
        let wa_messages = fetch_bridge_messages_trait(&client, "whatsapp", now - 3600, None)
            .await
            .unwrap();
        assert_eq!(wa_messages.len(), 1);
        assert_eq!(wa_messages[0].content, "WhatsApp message");

        // Fetch Telegram messages
        let tg_messages = fetch_bridge_messages_trait(&client, "telegram", now - 3600, None)
            .await
            .unwrap();
        assert_eq!(tg_messages.len(), 1);
        assert_eq!(tg_messages[0].content, "Telegram message");
    }
}

// ============================================================================
// Room Member Fallback Detection Tests
// ============================================================================
// These tests verify that service detection works via ghost users (bridged users)
// in the room when the message sender doesn't have a service prefix.

mod room_member_fallback_tests {
    use backend::api::matrix_client::{MockMatrixClient, MockRoom, RoomMember};
    use backend::utils::bridge::get_service_rooms_trait;

    // Helper to create a mock room member
    fn member(localpart: &str) -> RoomMember {
        RoomMember {
            user_id: format!("@{}:server.com", localpart),
            localpart: localpart.to_string(),
        }
    }

    #[tokio::test]
    async fn test_detects_whatsapp_room_via_ghost_user() {
        // Room with a WhatsApp ghost user but non-service sender
        let room = MockRoom::new("!wa:server", "John Smith (WA)")
            .with_members(vec![member("whatsapp_12345"), member("me")])
            .with_last_activity(1000);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        let rooms = get_service_rooms_trait(&client, "whatsapp").await.unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].display_name, "John Smith (WA)");
    }

    #[tokio::test]
    async fn test_detects_telegram_room_via_ghost_user() {
        let room = MockRoom::new("!tg:server", "Work Chat (TG)")
            .with_members(vec![member("telegram_67890"), member("me")])
            .with_last_activity(1000);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        let rooms = get_service_rooms_trait(&client, "telegram").await.unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].display_name, "Work Chat (TG)");
    }

    #[tokio::test]
    async fn test_detects_signal_room_via_ghost_user() {
        let room = MockRoom::new("!sig:server", "Private Chat (Signal)")
            .with_members(vec![member("signal_abc123"), member("me")])
            .with_last_activity(1000);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        let rooms = get_service_rooms_trait(&client, "signal").await.unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].display_name, "Private Chat (Signal)");
    }

    #[tokio::test]
    async fn test_detects_messenger_room_via_ghost_user() {
        let room = MockRoom::new("!msg:server", "FB Chat")
            .with_members(vec![member("messenger_999"), member("me")])
            .with_last_activity(1000);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        let rooms = get_service_rooms_trait(&client, "messenger").await.unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].display_name, "FB Chat");
    }

    #[tokio::test]
    async fn test_detects_instagram_room_via_ghost_user() {
        let room = MockRoom::new("!ig:server", "IG DM")
            .with_members(vec![member("instagram_user123"), member("me")])
            .with_last_activity(1000);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        let rooms = get_service_rooms_trait(&client, "instagram").await.unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].display_name, "IG DM");
    }

    #[tokio::test]
    async fn test_room_without_ghost_user_not_detected() {
        // Room with only regular members (no service prefix)
        let room = MockRoom::new("!regular:server", "Regular Chat")
            .with_members(vec![member("john_doe"), member("jane_doe"), member("me")])
            .with_last_activity(1000);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        // Should not find any WhatsApp rooms
        let rooms = get_service_rooms_trait(&client, "whatsapp").await.unwrap();
        assert!(rooms.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_ghost_users_in_room() {
        // Group chat with multiple WhatsApp ghost users
        let room = MockRoom::new("!group:server", "Family Group (WA)")
            .with_members(vec![
                member("whatsapp_mom"),
                member("whatsapp_dad"),
                member("whatsapp_sibling"),
                member("me"),
            ])
            .with_last_activity(1000);

        let client = MockMatrixClient::new().with_rooms(vec![room]);

        let rooms = get_service_rooms_trait(&client, "whatsapp").await.unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].display_name, "Family Group (WA)");
    }

    #[tokio::test]
    async fn test_mixed_services_detected_correctly() {
        // Multiple rooms with different services
        let wa_room = MockRoom::new("!wa:server", "WA Chat")
            .with_members(vec![member("whatsapp_1"), member("me")])
            .with_last_activity(1000);

        let tg_room = MockRoom::new("!tg:server", "TG Chat")
            .with_members(vec![member("telegram_2"), member("me")])
            .with_last_activity(2000);

        let sig_room = MockRoom::new("!sig:server", "Signal Chat")
            .with_members(vec![member("signal_3"), member("me")])
            .with_last_activity(3000);

        let client = MockMatrixClient::new().with_rooms(vec![wa_room, tg_room, sig_room]);

        // Each service query should only return matching rooms
        let wa_rooms = get_service_rooms_trait(&client, "whatsapp").await.unwrap();
        assert_eq!(wa_rooms.len(), 1);
        assert_eq!(wa_rooms[0].display_name, "WA Chat");

        let tg_rooms = get_service_rooms_trait(&client, "telegram").await.unwrap();
        assert_eq!(tg_rooms.len(), 1);
        assert_eq!(tg_rooms[0].display_name, "TG Chat");

        let sig_rooms = get_service_rooms_trait(&client, "signal").await.unwrap();
        assert_eq!(sig_rooms.len(), 1);
        assert_eq!(sig_rooms[0].display_name, "Signal Chat");
    }
}
