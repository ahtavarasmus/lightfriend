use backend::handlers::dashboard_handlers::Contact;
use backend::handlers::rule_handlers::{
    build_email_always_show_rule, build_platform_always_show_rule,
    build_platform_always_show_rule_with_mode, is_always_show_rule,
    platform_supports_authoritative_mentions, ALWAYS_SHOW_LOGIC_TYPE,
};
use backend::models::ontology_models::{NewOntRule, OntRule};
use backend::proactive::rules::matches_trigger;
use serde_json::json;

fn persisted(rule: NewOntRule) -> OntRule {
    OntRule {
        id: 42,
        user_id: rule.user_id,
        name: rule.name,
        trigger_type: rule.trigger_type,
        trigger_config: rule.trigger_config,
        logic_type: rule.logic_type,
        logic_prompt: rule.logic_prompt,
        logic_fetch: rule.logic_fetch,
        action_type: rule.action_type,
        action_config: rule.action_config,
        status: rule.status,
        next_fire_at: rule.next_fire_at,
        expires_at: rule.expires_at,
        last_triggered_at: None,
        created_at: rule.created_at,
        updated_at: rule.updated_at,
        flow_config: rule.flow_config,
    }
}

#[test]
fn platform_entry_builds_a_tagged_immediate_passthrough_rule() {
    let contact = Contact {
        id: "bridge_room:!alice:example.org".to_string(),
        display_name: "Alice".to_string(),
        subtitle: Some("DM · signal".to_string()),
        platform: Some("signal".to_string()),
        room_id: Some("!alice:example.org".to_string()),
        chat_id: None,
        person_id: None,
        is_group: false,
        source: "bridge_room".to_string(),
    };
    let rule = persisted(build_platform_always_show_rule(7, &contact, 100).unwrap());

    assert!(is_always_show_rule(&rule));
    assert_eq!(rule.logic_type, ALWAYS_SHOW_LOGIC_TYPE);
    let trigger: serde_json::Value = serde_json::from_str(&rule.trigger_config).unwrap();
    assert_eq!(trigger["delay_seconds"], 0);
    assert_eq!(trigger["incoming_only"], true);
    assert_eq!(trigger["resolved_room_id"], "!alice:example.org");
    let flow: serde_json::Value =
        serde_json::from_str(rule.flow_config.as_deref().unwrap()).unwrap();
    assert_eq!(flow["type"], "action");
    assert_eq!(flow["action_type"], "notify");

    assert!(matches_trigger(
        &rule,
        "Message",
        "created",
        &json!({
            "platform": "signal",
            "room_id": "!alice:example.org",
            "sender_name": "Alice",
            "content": "hello",
            "is_group": false,
            "is_outgoing": false
        })
    ));
    assert!(!matches_trigger(
        &rule,
        "Message",
        "created",
        &json!({
            "platform": "signal",
            "room_id": "!someone-else:example.org",
            "sender_name": "Alice",
            "content": "hello",
            "is_group": false,
            "is_outgoing": false
        })
    ));
}

#[test]
fn always_show_rule_never_matches_an_outgoing_message() {
    let contact = Contact {
        id: "chat:telegram:alice".to_string(),
        display_name: "Alice".to_string(),
        subtitle: Some("telegram".to_string()),
        platform: Some("telegram".to_string()),
        room_id: None,
        chat_id: None,
        person_id: None,
        is_group: false,
        source: "chat".to_string(),
    };
    let rule = persisted(build_platform_always_show_rule(7, &contact, 100).unwrap());

    assert!(!matches_trigger(
        &rule,
        "Message",
        "created",
        &json!({
            "platform": "telegram",
            "room_id": "!alice:example.org",
            "sender_name": "Alice",
            "content": "sent by me",
            "is_group": false,
            "is_outgoing": true
        })
    ));
}

#[test]
fn email_entry_matches_the_normalized_envelope_sender_not_display_name() {
    let rule = persisted(build_email_always_show_rule(7, "  Alice@Example.COM ", 100).unwrap());

    assert!(matches_trigger(
        &rule,
        "Message",
        "created",
        &json!({
            "platform": "email",
            "room_id": "imap:4:123",
            "sender_name": "Alice Smith",
            "sender_key": "ALICE@example.com",
            "content": "hello",
            "is_group": false
        })
    ));
    assert!(!matches_trigger(
        &rule,
        "Message",
        "created",
        &json!({
            "platform": "email",
            "room_id": "imap:4:124",
            "sender_name": "Alice Smith",
            "sender_key": "other@example.com",
            "content": "hello",
            "is_group": false
        })
    ));
}

#[test]
fn platform_entry_requires_a_platform_specific_contact() {
    let aggregate_person = Contact {
        id: "person:12".to_string(),
        display_name: "Alice".to_string(),
        subtitle: Some("Person · 2 channels".to_string()),
        platform: None,
        room_id: None,
        chat_id: None,
        person_id: Some(12),
        is_group: false,
        source: "person".to_string(),
    };

    assert!(build_platform_always_show_rule(7, &aggregate_person, 100).is_err());
    assert!(build_email_always_show_rule(7, "not-an-email", 100).is_err());
}

fn group_contact(platform: &str) -> Contact {
    Contact {
        id: format!("bridge_group:{}:family", platform),
        display_name: "Family".to_string(),
        subtitle: Some(format!("Group · {}", platform)),
        platform: Some(platform.to_string()),
        room_id: Some(format!("!family-{}:example.org", platform)),
        chat_id: Some("family-native-id".to_string()),
        person_id: None,
        is_group: true,
        source: "bridge_group".to_string(),
    }
}

#[test]
fn group_all_messages_mode_matches_mentions_and_non_mentions() {
    let rule = persisted(
        build_platform_always_show_rule_with_mode(7, &group_contact("whatsapp"), Some("all"), 100)
            .unwrap(),
    );
    for is_mentioned in [false, true] {
        assert!(matches_trigger(
            &rule,
            "Message",
            "created",
            &json!({
                "platform": "whatsapp",
                "room_id": "!family-whatsapp:example.org",
                "sender_name": "Family",
                "content": "hello",
                "is_group": true,
                "is_mentioned": is_mentioned,
            })
        ));
    }
}

#[test]
fn mentions_only_requires_authoritative_metadata_not_at_text() {
    let rule = persisted(
        build_platform_always_show_rule_with_mode(
            7,
            &group_contact("whatsapp"),
            Some("mention_only"),
            100,
        )
        .unwrap(),
    );
    assert!(!matches_trigger(
        &rule,
        "Message",
        "created",
        &json!({
            "platform": "whatsapp",
            "room_id": "!family-whatsapp:example.org",
            "sender_name": "Family",
            "content": "@Rasmus hello",
            "is_group": true,
            "is_mentioned": false,
        })
    ));
    assert!(matches_trigger(
        &rule,
        "Message",
        "created",
        &json!({
            "platform": "whatsapp",
            "room_id": "!family-whatsapp:example.org",
            "sender_name": "Family",
            "content": "hello",
            "is_group": true,
            "is_mentioned": true,
        })
    ));
}

#[test]
fn direct_entries_are_unchanged_and_unverified_platform_mentions_are_rejected() {
    let direct = Contact {
        is_group: false,
        ..group_contact("whatsapp")
    };
    assert!(build_platform_always_show_rule_with_mode(7, &direct, None, 100).is_ok());
    assert!(
        build_platform_always_show_rule_with_mode(7, &direct, Some("mention_only"), 100).is_err()
    );

    assert!(platform_supports_authoritative_mentions("whatsapp"));
    assert!(!platform_supports_authoritative_mentions("telegram"));
    assert!(build_platform_always_show_rule_with_mode(
        7,
        &group_contact("telegram"),
        Some("mention_only"),
        100,
    )
    .is_err());
}
