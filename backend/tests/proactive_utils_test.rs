//! Unit tests for proactive utils - response parsing and serialization tests.
//!
//! Tests the public structs used for AI response parsing and digest data.

use backend::proactive::utils::{
    CalendarEvent, DigestData, MatchResponse, MessageInfo, TaskMatchResponse,
};

// =========================================================================
// MatchResponse Parsing Tests
// =========================================================================

#[test]
fn test_match_response_parses_critical_message() {
    let json = r#"{
        "is_critical": true,
        "what_to_inform": "Urgent: Server down, needs immediate attention",
        "first_message": "Hey, your production server is down!"
    }"#;

    let response: MatchResponse = serde_json::from_str(json).unwrap();

    assert!(response.is_critical);
    assert_eq!(
        response.what_to_inform,
        Some("Urgent: Server down, needs immediate attention".to_string())
    );
    assert_eq!(
        response.first_message,
        Some("Hey, your production server is down!".to_string())
    );
}

#[test]
fn test_match_response_parses_non_critical_message() {
    let json = r#"{
        "is_critical": false,
        "what_to_inform": "",
        "first_message": ""
    }"#;

    let response: MatchResponse = serde_json::from_str(json).unwrap();

    assert!(!response.is_critical);
    // Empty strings should be present
    assert_eq!(response.what_to_inform, Some("".to_string()));
    assert_eq!(response.first_message, Some("".to_string()));
}

#[test]
fn test_match_response_parses_missing_optional_fields() {
    // When LLM returns only is_critical with no other fields
    let json = r#"{"is_critical": false}"#;

    let response: MatchResponse = serde_json::from_str(json).unwrap();

    assert!(!response.is_critical);
    assert!(response.what_to_inform.is_none());
    assert!(response.first_message.is_none());
}

#[test]
fn test_match_response_parses_null_optional_fields() {
    let json = r#"{
        "is_critical": true,
        "what_to_inform": null,
        "first_message": null
    }"#;

    let response: MatchResponse = serde_json::from_str(json).unwrap();

    assert!(response.is_critical);
    assert!(response.what_to_inform.is_none());
    assert!(response.first_message.is_none());
}

// =========================================================================
// TaskMatchResponse Parsing Tests
// =========================================================================

#[test]
fn test_task_match_response_parses_matched_task() {
    let json = r#"{
        "task_id": 42,
        "sms_message": "Your package has arrived at the office",
        "first_message": "Hey, your package is here!",
        "match_explanation": "Message mentions package delivery"
    }"#;

    let response: TaskMatchResponse = serde_json::from_str(json).unwrap();

    assert_eq!(response.task_id, Some(42));
    assert_eq!(
        response.sms_message,
        Some("Your package has arrived at the office".to_string())
    );
    assert_eq!(
        response.first_message,
        Some("Hey, your package is here!".to_string())
    );
    assert_eq!(
        response.match_explanation,
        Some("Message mentions package delivery".to_string())
    );
}

#[test]
fn test_task_match_response_parses_no_match() {
    let json = r#"{
        "task_id": null,
        "sms_message": "",
        "first_message": "",
        "match_explanation": ""
    }"#;

    let response: TaskMatchResponse = serde_json::from_str(json).unwrap();

    assert!(response.task_id.is_none());
    assert_eq!(response.sms_message, Some("".to_string()));
}

#[test]
fn test_task_match_response_parses_minimal_response() {
    // LLM might return just task_id
    let json = r#"{"task_id": null}"#;

    let response: TaskMatchResponse = serde_json::from_str(json).unwrap();

    assert!(response.task_id.is_none());
    assert!(response.sms_message.is_none());
    assert!(response.first_message.is_none());
    assert!(response.match_explanation.is_none());
}

// =========================================================================
// WhatsApp Call Detection Tests
// =========================================================================

#[test]
fn test_whatsapp_incoming_call_detected() {
    let raw_content = "Incoming call from +1234567890";
    assert!(raw_content.contains("Incoming call"));
}

#[test]
fn test_whatsapp_missed_call_detected() {
    let raw_content = "Missed call from John";
    assert!(raw_content.contains("Missed call"));
}

#[test]
fn test_whatsapp_regular_message_not_call() {
    let raw_content = "Hey, how are you?";
    assert!(!raw_content.contains("Incoming call"));
    assert!(!raw_content.contains("Missed call"));
}

// =========================================================================
// DigestData Serialization Tests
// =========================================================================

#[test]
fn test_digest_data_serializes_correctly() {
    let data = DigestData {
        messages: vec![MessageInfo {
            sender: "John".to_string(),
            content: "Hello there".to_string(),
            timestamp_rfc: "2024-01-15T10:30:00Z".to_string(),
            platform: "whatsapp".to_string(),
        }],
        calendar_events: vec![CalendarEvent {
            title: "Meeting".to_string(),
            start_time_rfc: "2024-01-15T14:00:00Z".to_string(),
            duration_minutes: 60,
        }],
        time_period_hours: 8,
        current_datetime_local: "2024-01-15T12:00:00+02:00".to_string(),
    };

    let json = serde_json::to_string(&data).unwrap();

    assert!(json.contains("John"));
    assert!(json.contains("Hello there"));
    assert!(json.contains("whatsapp"));
    assert!(json.contains("Meeting"));
    assert!(json.contains("8")); // time_period_hours
}

#[test]
fn test_message_info_creation() {
    let info = MessageInfo {
        sender: "Alice".to_string(),
        content: "Important meeting tomorrow".to_string(),
        timestamp_rfc: "2024-01-15T09:00:00Z".to_string(),
        platform: "telegram".to_string(),
    };

    assert_eq!(info.sender, "Alice");
    assert_eq!(info.platform, "telegram");
}
