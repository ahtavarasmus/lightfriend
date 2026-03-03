//! Tests for rule-based quiet mode: suppress/allow rules, platform/sender filtering,
//! backward compatibility, and platform extraction.

use backend::proactive::utils::{extract_platform_from_content_type, parse_summary_tags};
use backend::repositories::user_core::rule_matches;
use backend::test_utils::mock_user_core::MockUserCore;
use backend::UserCoreOps;

fn future_ts() -> i32 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    now + 3600 // 1 hour from now
}

// =============================================================================
// 1. Global suppress blocks all notifications
// =============================================================================

#[test]
fn test_global_suppress_blocks_all() {
    let mock = MockUserCore::new();
    let uid = 1;

    // Enable global quiet mode (indefinite)
    mock.set_quiet_mode(uid, Some(0)).unwrap();

    // Should suppress regardless of context
    assert!(mock
        .check_quiet_with_context(uid, Some("whatsapp"), Some("Mom"), None)
        .unwrap());
    assert!(mock
        .check_quiet_with_context(uid, Some("telegram"), None, None)
        .unwrap());
    assert!(mock
        .check_quiet_with_context(uid, None, None, None)
        .unwrap());
}

// =============================================================================
// 2. Suppress rule for whatsapp blocks whatsapp, allows telegram
// =============================================================================

#[test]
fn test_suppress_whatsapp_allows_telegram() {
    let mock = MockUserCore::new();
    let uid = 1;

    mock.add_quiet_rule(
        uid,
        future_ts(),
        "suppress",
        Some("whatsapp"),
        None,
        None,
        "No WhatsApp",
    )
    .unwrap();

    // WhatsApp suppressed
    assert!(mock
        .check_quiet_with_context(uid, Some("whatsapp"), Some("Alice"), None)
        .unwrap());

    // Telegram allowed
    assert!(!mock
        .check_quiet_with_context(uid, Some("telegram"), Some("Alice"), None)
        .unwrap());

    // No platform - allowed (rule only matches whatsapp)
    assert!(!mock
        .check_quiet_with_context(uid, None, Some("Alice"), None)
        .unwrap());
}

// =============================================================================
// 3. Allow rule for sender "Mom" blocks everything except Mom
// =============================================================================

#[test]
fn test_allow_sender_mom_blocks_others() {
    let mock = MockUserCore::new();
    let uid = 1;

    mock.add_quiet_rule(
        uid,
        future_ts(),
        "allow",
        None,
        Some("Mom"),
        None,
        "Only Mom",
    )
    .unwrap();

    // Mom's messages allowed
    assert!(!mock
        .check_quiet_with_context(uid, Some("whatsapp"), Some("Mom"), None)
        .unwrap());

    // Anyone else blocked
    assert!(mock
        .check_quiet_with_context(uid, Some("whatsapp"), Some("Alice"), None)
        .unwrap());

    // No sender info - blocked (can't match the allow rule)
    assert!(mock
        .check_quiet_with_context(uid, Some("whatsapp"), None, None)
        .unwrap());

    // Mom in content (substring match) - also allowed
    assert!(!mock
        .check_quiet_with_context(uid, Some("whatsapp"), None, Some("Message from Mom"))
        .unwrap());
}

// =============================================================================
// 4. Multiple suppress rules combine with OR
// =============================================================================

#[test]
fn test_multiple_suppress_rules_or() {
    let mock = MockUserCore::new();
    let uid = 1;

    mock.add_quiet_rule(
        uid,
        future_ts(),
        "suppress",
        Some("whatsapp"),
        None,
        None,
        "No WA",
    )
    .unwrap();
    mock.add_quiet_rule(
        uid,
        future_ts(),
        "suppress",
        Some("telegram"),
        None,
        None,
        "No TG",
    )
    .unwrap();

    // Both platforms suppressed
    assert!(mock
        .check_quiet_with_context(uid, Some("whatsapp"), None, None)
        .unwrap());
    assert!(mock
        .check_quiet_with_context(uid, Some("telegram"), None, None)
        .unwrap());

    // Signal still allowed
    assert!(!mock
        .check_quiet_with_context(uid, Some("signal"), None, None)
        .unwrap());
}

// =============================================================================
// 5. Expired rules auto-cleaned (mock returns empty for expired)
// =============================================================================

#[test]
fn test_expired_rules_ignored() {
    let mock = MockUserCore::new();
    let uid = 1;

    let past_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32
        - 100; // 100 seconds in the past

    mock.add_quiet_rule(
        uid,
        past_ts,
        "suppress",
        Some("whatsapp"),
        None,
        None,
        "Expired rule",
    )
    .unwrap();

    // Should NOT suppress because rule is expired
    assert!(!mock
        .check_quiet_with_context(uid, Some("whatsapp"), None, None)
        .unwrap());
}

// =============================================================================
// 6. extract_platform_from_content_type covers all known patterns
// =============================================================================

#[test]
fn test_extract_platform_from_content_type() {
    assert_eq!(
        extract_platform_from_content_type("whatsapp_profile_sms"),
        Some("whatsapp".to_string())
    );
    assert_eq!(
        extract_platform_from_content_type("telegram_mention_call"),
        Some("telegram".to_string())
    );
    assert_eq!(
        extract_platform_from_content_type("signal_critical"),
        Some("signal".to_string())
    );
    assert_eq!(
        extract_platform_from_content_type("email_incoming"),
        Some("email".to_string())
    );
    assert_eq!(
        extract_platform_from_content_type("tesla_charging"),
        Some("tesla".to_string())
    );
    assert_eq!(
        extract_platform_from_content_type("digest_morning"),
        Some("digest".to_string())
    );
    // Unknown content type
    assert_eq!(extract_platform_from_content_type("unknown_type"), None);
    assert_eq!(extract_platform_from_content_type("item_sms"), None);
    // Removed platforms should return None
    assert_eq!(
        extract_platform_from_content_type("calendar_reminder"),
        None
    );
    assert_eq!(extract_platform_from_content_type("messenger_sms"), None);
    assert_eq!(extract_platform_from_content_type("instagram_dm"), None);
    assert_eq!(extract_platform_from_content_type("bluesky_mention"), None);
}

// =============================================================================
// 7. add_rule rejects indefinite duration (checked at tool level,
//    but we verify the mock stores/retrieves correctly)
// =============================================================================

#[test]
fn test_add_rule_stores_and_retrieves() {
    let mock = MockUserCore::new();
    let uid = 1;

    let id = mock
        .add_quiet_rule(
            uid,
            future_ts(),
            "suppress",
            Some("email"),
            None,
            None,
            "No email",
        )
        .unwrap();
    assert!(id > 0);

    let rules = mock.get_quiet_rules(uid).unwrap();
    assert_eq!(rules.len(), 1);

    let tags = parse_summary_tags(&rules[0].summary);
    assert_eq!(tags.quiet.as_deref(), Some("suppress"));
    assert_eq!(tags.platform.as_deref(), Some("email"));
    assert!(tags.sender.is_none());
}

// =============================================================================
// 8. Global quiet takes precedence over rules
// =============================================================================

#[test]
fn test_global_quiet_supersedes_rules() {
    let mock = MockUserCore::new();
    let uid = 1;

    // Add an allow rule for Mom
    mock.add_quiet_rule(
        uid,
        future_ts(),
        "allow",
        None,
        Some("Mom"),
        None,
        "Only Mom",
    )
    .unwrap();

    // Mom should be allowed with just the rule
    assert!(!mock
        .check_quiet_with_context(uid, None, Some("Mom"), None)
        .unwrap());

    // Now enable global quiet - this clears all rules
    mock.set_quiet_mode(uid, Some(0)).unwrap();

    // Even Mom should be blocked now (global suppress)
    assert!(mock
        .check_quiet_with_context(uid, None, Some("Mom"), None)
        .unwrap());
}

// =============================================================================
// 9. Backward compat - tagless items work as global suppress
// =============================================================================

#[test]
fn test_backward_compat_tagless_global() {
    let mock = MockUserCore::new();
    let uid = 1;

    // set_quiet_mode creates a tagless item (no [quiet:...] tag)
    mock.set_quiet_mode(uid, Some(future_ts())).unwrap();

    // Should suppress everything (backward compat)
    assert!(mock
        .check_quiet_with_context(uid, Some("whatsapp"), Some("Alice"), None)
        .unwrap());
    assert!(mock
        .check_quiet_with_context(uid, None, None, None)
        .unwrap());
}

// =============================================================================
// rule_matches unit tests
// =============================================================================

#[test]
fn test_rule_matches_platform_exact() {
    let tags = parse_summary_tags("[quiet:suppress] [platform:WhatsApp]");
    // Case-insensitive exact match
    assert!(rule_matches(&tags, Some("whatsapp"), None, None));
    assert!(rule_matches(&tags, Some("WHATSAPP"), None, None));
    assert!(!rule_matches(&tags, Some("telegram"), None, None));
    assert!(!rule_matches(&tags, None, None, None));
}

#[test]
fn test_rule_matches_sender_substring() {
    let tags = parse_summary_tags("[quiet:suppress] [sender:Mom]");
    assert!(rule_matches(&tags, None, Some("Mom"), None));
    assert!(rule_matches(&tags, None, Some("My Mom's phone"), None));
    assert!(rule_matches(&tags, None, None, Some("from Mom: hi")));
    assert!(!rule_matches(&tags, None, Some("Dad"), None));
    assert!(!rule_matches(&tags, None, None, None));
}

#[test]
fn test_rule_matches_all_conditions_and() {
    let tags = parse_summary_tags("[quiet:suppress] [platform:whatsapp] [sender:Bob]");
    // Both must match
    assert!(rule_matches(&tags, Some("whatsapp"), Some("Bob"), None));
    // Only one matches - should fail
    assert!(!rule_matches(&tags, Some("telegram"), Some("Bob"), None));
    assert!(!rule_matches(&tags, Some("whatsapp"), Some("Alice"), None));
}

#[test]
fn test_rule_matches_no_conditions_matches_everything() {
    let tags = parse_summary_tags("[quiet:suppress]");
    assert!(rule_matches(&tags, Some("whatsapp"), Some("Alice"), None));
    assert!(rule_matches(&tags, None, None, None));
}

#[test]
fn test_parse_quiet_tag() {
    let tags = parse_summary_tags("[quiet:suppress] [platform:whatsapp]\nSome description");
    assert_eq!(tags.quiet.as_deref(), Some("suppress"));
    assert_eq!(tags.platform.as_deref(), Some("whatsapp"));
    assert!(tags.has_tags);

    let tags2 = parse_summary_tags("[quiet:allow] [sender:Mom]");
    assert_eq!(tags2.quiet.as_deref(), Some("allow"));
    assert_eq!(tags2.sender.as_deref(), Some("Mom"));
}
