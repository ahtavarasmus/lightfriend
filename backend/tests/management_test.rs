//! Unit tests for build_summary_from_params - deterministic, no LLM needed.
//!
//! Run with: `cargo test --test management_test`

use backend::tool_call_utils::management::{build_summary_from_params, CreateItemArgs};

/// Helper to create CreateItemArgs with only required fields set
fn minimal_args(item_type: &str, notify: &str, description: &str) -> CreateItemArgs {
    CreateItemArgs {
        item_type: item_type.to_string(),
        notify: notify.to_string(),
        description: description.to_string(),
        due_at: None,
        repeat: None,
        fetch: None,
        platform: None,
        sender: None,
        topic: None,
    }
}

// =============================================================================
// M1: Full params - all fields set, verify exact output format
// =============================================================================

#[test]
fn test_m1_full_params() {
    let args = CreateItemArgs {
        item_type: "tracking".to_string(),
        notify: "sms".to_string(),
        description: "Watch for package delivery from Amazon".to_string(),
        due_at: None,
        repeat: Some("daily 09:00".to_string()),
        fetch: Some("email".to_string()),
        platform: Some("email".to_string()),
        sender: Some("Amazon".to_string()),
        topic: Some("delivery".to_string()),
    };

    let result = build_summary_from_params(&args);
    let lines: Vec<&str> = result.lines().collect();

    // First line has all tags
    assert_eq!(
        lines.len(),
        2,
        "Should have exactly 2 lines (tags + description)"
    );

    let tag_line = lines[0];
    assert!(
        tag_line.contains("[type:tracking]"),
        "Missing type tag: {}",
        tag_line
    );
    assert!(
        tag_line.contains("[notify:sms]"),
        "Missing notify tag: {}",
        tag_line
    );
    assert!(
        tag_line.contains("[repeat:daily 09:00]"),
        "Missing repeat tag: {}",
        tag_line
    );
    assert!(
        tag_line.contains("[fetch:email]"),
        "Missing fetch tag: {}",
        tag_line
    );
    assert!(
        tag_line.contains("[platform:email]"),
        "Missing platform tag: {}",
        tag_line
    );
    assert!(
        tag_line.contains("[sender:Amazon]"),
        "Missing sender tag: {}",
        tag_line
    );
    assert!(
        tag_line.contains("[topic:delivery]"),
        "Missing topic tag: {}",
        tag_line
    );

    // Second line is the description
    assert_eq!(lines[1], "Watch for package delivery from Amazon");
}

// =============================================================================
// M2: Minimal params - only required fields (item_type, notify, description)
// =============================================================================

#[test]
fn test_m2_minimal_params() {
    let args = minimal_args("oneshot", "sms", "Remind the user to call the dentist");

    let result = build_summary_from_params(&args);
    let lines: Vec<&str> = result.lines().collect();

    assert_eq!(lines.len(), 2, "Should have exactly 2 lines");

    let tag_line = lines[0];
    assert_eq!(
        tag_line, "[type:oneshot] [notify:sms]",
        "Only type and notify tags: {}",
        tag_line
    );

    assert_eq!(lines[1], "Remind the user to call the dentist");
}

// =============================================================================
// M3: Tracking with filters - platform + sender + topic
// =============================================================================

#[test]
fn test_m3_tracking_with_filters() {
    let args = CreateItemArgs {
        platform: Some("whatsapp".to_string()),
        sender: Some("boss".to_string()),
        topic: Some("promotion".to_string()),
        ..minimal_args("tracking", "call", "Watch for boss message about promotion")
    };

    let result = build_summary_from_params(&args);
    let tag_line = result.lines().next().unwrap();

    assert!(tag_line.contains("[type:tracking]"));
    assert!(tag_line.contains("[notify:call]"));
    assert!(tag_line.contains("[platform:whatsapp]"));
    assert!(tag_line.contains("[sender:boss]"));
    assert!(tag_line.contains("[topic:promotion]"));
    // No fetch or repeat tags should be present
    assert!(
        !tag_line.contains("[fetch:"),
        "Should not have fetch tag: {}",
        tag_line
    );
    assert!(
        !tag_line.contains("[repeat:"),
        "Should not have repeat tag: {}",
        tag_line
    );
}

// =============================================================================
// M3b: topic="any" emits [scope:any] instead of [topic:any]
// =============================================================================

#[test]
fn test_m3b_topic_any_becomes_scope_any() {
    let args = CreateItemArgs {
        platform: Some("whatsapp".to_string()),
        sender: Some("mom".to_string()),
        topic: Some("any".to_string()),
        ..minimal_args(
            "tracking",
            "sms",
            "Notify when mom sends any message on WhatsApp",
        )
    };

    let result = build_summary_from_params(&args);
    let tag_line = result.lines().next().unwrap();

    assert!(
        tag_line.contains("[scope:any]"),
        "topic='any' should produce [scope:any]. Got: {}",
        tag_line
    );
    assert!(
        !tag_line.contains("[topic:any]"),
        "Should NOT have [topic:any]. Got: {}",
        tag_line
    );
    assert!(tag_line.contains("[platform:whatsapp]"));
    assert!(tag_line.contains("[sender:mom]"));
}

// =============================================================================
// M4: Tag ordering - verify consistent order
// =============================================================================

#[test]
fn test_m4_tag_ordering() {
    let args = CreateItemArgs {
        item_type: "tracking".to_string(),
        notify: "sms".to_string(),
        description: "Test".to_string(),
        due_at: None,
        repeat: Some("daily 08:00".to_string()),
        fetch: Some("email,weather".to_string()),
        platform: Some("email".to_string()),
        sender: Some("HR".to_string()),
        topic: Some("offer".to_string()),
    };

    let result = build_summary_from_params(&args);
    let tag_line = result.lines().next().unwrap();

    // Find positions of each tag to verify order
    let type_pos = tag_line.find("[type:").expect("Missing type tag");
    let notify_pos = tag_line.find("[notify:").expect("Missing notify tag");
    let repeat_pos = tag_line.find("[repeat:").expect("Missing repeat tag");
    let fetch_pos = tag_line.find("[fetch:").expect("Missing fetch tag");
    let platform_pos = tag_line.find("[platform:").expect("Missing platform tag");
    let sender_pos = tag_line.find("[sender:").expect("Missing sender tag");
    let topic_pos = tag_line.find("[topic:").expect("Missing topic tag");

    // Order: type, notify, repeat, fetch, platform, sender, topic
    assert!(type_pos < notify_pos, "type should come before notify");
    assert!(notify_pos < repeat_pos, "notify should come before repeat");
    assert!(repeat_pos < fetch_pos, "repeat should come before fetch");
    assert!(
        fetch_pos < platform_pos,
        "fetch should come before platform"
    );
    assert!(
        platform_pos < sender_pos,
        "platform should come before sender"
    );
    assert!(sender_pos < topic_pos, "sender should come before topic");
}

// =============================================================================
// M5: Description with newlines - tags stay on first line
// =============================================================================

#[test]
fn test_m5_description_with_newlines() {
    let args = minimal_args(
        "tracking",
        "sms",
        "Watch for delivery.\nPrevious check: nothing found.\nUpdated Feb 28.",
    );

    let result = build_summary_from_params(&args);
    let lines: Vec<&str> = result.lines().collect();

    // Tags should be only on the first line
    assert!(
        lines[0].starts_with("[type:tracking]"),
        "First line must start with tags"
    );
    assert!(
        !lines[0].contains("Watch for"),
        "First line should not contain description"
    );

    // Description starts on second line
    assert_eq!(lines[1], "Watch for delivery.");
    assert_eq!(lines[2], "Previous check: nothing found.");
    assert_eq!(lines[3], "Updated Feb 28.");

    // Total: 4 lines (1 tag line + 3 description lines)
    assert_eq!(lines.len(), 4);
}
