//! Tests for ElevenLabs pure functions

use backend::api::elevenlabs::{
    build_conversation_variables, format_timezone_offset, get_first_message_for_language,
    get_voice_id_for_language, get_welcome_message_for_language, validate_webhook_secret_value,
};

// ============================================================
// Webhook Secret Validation Tests
// ============================================================

#[test]
fn test_validate_webhook_secret_matching() {
    assert!(validate_webhook_secret_value(
        Some("secret123"),
        "secret123"
    ));
}

#[test]
fn test_validate_webhook_secret_non_matching() {
    assert!(!validate_webhook_secret_value(
        Some("wrong_secret"),
        "secret123"
    ));
}

#[test]
fn test_validate_webhook_secret_none() {
    assert!(!validate_webhook_secret_value(None, "secret123"));
}

#[test]
fn test_validate_webhook_secret_empty_header() {
    assert!(!validate_webhook_secret_value(Some(""), "secret123"));
}

#[test]
fn test_validate_webhook_secret_empty_expected() {
    assert!(validate_webhook_secret_value(Some(""), ""));
}

// ============================================================
// Build Conversation Variables Tests
// ============================================================

#[test]
fn test_build_conversation_variables_basic() {
    let vars = build_conversation_variables(
        123,
        "John",
        "User likes hiking",
        "San Francisco, CA",
        "Golden Gate Park, Pier 39",
        "America/Los_Angeles",
        "-08:00",
        "user: Hello\nassistant: Hi there!",
    );

    assert_eq!(vars.get("user_id").unwrap(), 123);
    assert_eq!(vars.get("name").unwrap(), "John");
    assert_eq!(vars.get("user_info").unwrap(), "User likes hiking");
    assert_eq!(vars.get("location").unwrap(), "San Francisco, CA");
    assert_eq!(
        vars.get("nearby_places").unwrap(),
        "Golden Gate Park, Pier 39"
    );
    assert_eq!(vars.get("timezone").unwrap(), "America/Los_Angeles");
    assert_eq!(vars.get("timezone_offset_from_utc").unwrap(), "-08:00");
    assert_eq!(
        vars.get("recent_conversation").unwrap(),
        "user: Hello\nassistant: Hi there!"
    );
}

#[test]
fn test_build_conversation_variables_default_fields() {
    let vars = build_conversation_variables(1, "Test", "", "", "", "UTC", "+00:00", "");

    // Check default values are set
    assert_eq!(vars.get("email_id").unwrap(), "-1");
    assert_eq!(vars.get("content_type").unwrap(), "");
    assert_eq!(vars.get("notification_message").unwrap(), "");
}

#[test]
fn test_build_conversation_variables_empty_values() {
    let vars = build_conversation_variables(0, "", "", "", "", "", "", "");

    assert_eq!(vars.get("user_id").unwrap(), 0);
    assert_eq!(vars.get("name").unwrap(), "");
    assert_eq!(vars.get("user_info").unwrap(), "");
}

// ============================================================
// Format Timezone Offset Tests
// ============================================================

#[test]
fn test_format_timezone_offset_positive() {
    assert_eq!(format_timezone_offset(2, 0), "+02:00");
    assert_eq!(format_timezone_offset(5, 30), "+05:30");
    assert_eq!(format_timezone_offset(12, 0), "+12:00");
}

#[test]
fn test_format_timezone_offset_negative() {
    assert_eq!(format_timezone_offset(-5, 0), "-05:00");
    assert_eq!(format_timezone_offset(-8, 0), "-08:00");
    assert_eq!(format_timezone_offset(-5, 30), "-05:30");
}

#[test]
fn test_format_timezone_offset_zero() {
    assert_eq!(format_timezone_offset(0, 0), "+00:00");
}

#[test]
fn test_format_timezone_offset_leading_zeros() {
    assert_eq!(format_timezone_offset(1, 5), "+01:05");
    assert_eq!(format_timezone_offset(-1, 5), "-01:05");
}

// ============================================================
// Voice ID Selection Tests
// ============================================================

#[test]
fn test_get_voice_id_for_language_english() {
    let voice_id = get_voice_id_for_language("en", "us_voice", "fi_voice", "de_voice");
    assert_eq!(voice_id, "us_voice");
}

#[test]
fn test_get_voice_id_for_language_finnish() {
    let voice_id = get_voice_id_for_language("fi", "us_voice", "fi_voice", "de_voice");
    assert_eq!(voice_id, "fi_voice");
}

#[test]
fn test_get_voice_id_for_language_german() {
    let voice_id = get_voice_id_for_language("de", "us_voice", "fi_voice", "de_voice");
    assert_eq!(voice_id, "de_voice");
}

#[test]
fn test_get_voice_id_for_language_unknown_defaults_to_us() {
    let voice_id = get_voice_id_for_language("fr", "us_voice", "fi_voice", "de_voice");
    assert_eq!(voice_id, "us_voice");

    let voice_id = get_voice_id_for_language("", "us_voice", "fi_voice", "de_voice");
    assert_eq!(voice_id, "us_voice");
}

// ============================================================
// First Message Tests
// ============================================================

#[test]
fn test_get_first_message_for_language_english() {
    let msg = get_first_message_for_language("en", "John");
    assert_eq!(msg, "Hello John!");
}

#[test]
fn test_get_first_message_for_language_finnish() {
    let msg = get_first_message_for_language("fi", "Matti");
    assert_eq!(msg, "Moi Matti!");
}

#[test]
fn test_get_first_message_for_language_german() {
    let msg = get_first_message_for_language("de", "Hans");
    assert_eq!(msg, "Hallo Hans!");
}

#[test]
fn test_get_first_message_for_language_with_placeholder() {
    // The function uses {{name}} format in the actual code but here returns formatted
    let msg = get_first_message_for_language("en", "{{name}}");
    assert_eq!(msg, "Hello {{name}}!");
}

// ============================================================
// Welcome Message Tests
// ============================================================

#[test]
fn test_get_welcome_message_for_language_english() {
    let msg = get_welcome_message_for_language("en");
    assert!(msg.contains("Welcome"));
    assert!(msg.contains("verified"));
}

#[test]
fn test_get_welcome_message_for_language_finnish() {
    let msg = get_welcome_message_for_language("fi");
    assert!(msg.contains("Tervetuloa"));
    assert!(msg.contains("vahvistettu"));
}

#[test]
fn test_get_welcome_message_for_language_german() {
    let msg = get_welcome_message_for_language("de");
    assert!(msg.contains("Willkommen"));
    assert!(msg.contains("verifiziert"));
}

#[test]
fn test_get_welcome_message_for_language_unknown() {
    // Unknown language defaults to English
    let msg = get_welcome_message_for_language("fr");
    assert!(msg.contains("Welcome"));
}
