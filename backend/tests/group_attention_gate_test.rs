use backend::models::ontology_models::OntRule;
use backend::proactive::rules::matches_trigger;
use backend::repositories::whatsapp_bridge_repository::whatsapp_mute_is_active;
use serde_json::json;

fn rule(trigger_config: serde_json::Value) -> OntRule {
    OntRule {
        id: 1,
        user_id: 1,
        name: "test rule".to_string(),
        trigger_type: "ontology_change".to_string(),
        trigger_config: trigger_config.to_string(),
        logic_type: "always".to_string(),
        logic_prompt: None,
        logic_fetch: None,
        action_type: "notify".to_string(),
        action_config: "{}".to_string(),
        status: "active".to_string(),
        next_fire_at: None,
        expires_at: None,
        last_triggered_at: None,
        created_at: 0,
        updated_at: 0,
        flow_config: None,
    }
}

fn group_snapshot(content: &str) -> serde_json::Value {
    json!({
        "platform": "whatsapp",
        "room_id": "!group:server",
        "sender_name": "Family",
        "content": content,
        "is_group": true
    })
}

#[test]
fn broad_message_rule_does_not_match_group_message() {
    let rule = rule(json!({
        "entity_type": "Message",
        "change": "created"
    }));

    assert!(!matches_trigger(
        &rule,
        "Message",
        "created",
        &group_snapshot("hello")
    ));
}

#[test]
fn exact_group_all_rule_matches_group_message() {
    let rule = rule(json!({
        "entity_type": "Message",
        "change": "created",
        "resolved_room_id": "!group:server",
        "group_mode": "all"
    }));

    assert!(matches_trigger(
        &rule,
        "Message",
        "created",
        &group_snapshot("hello")
    ));
}

#[test]
fn exact_group_rule_does_not_match_other_group_room() {
    let rule = rule(json!({
        "entity_type": "Message",
        "change": "created",
        "resolved_room_id": "!other:server",
        "group_mode": "all"
    }));

    assert!(!matches_trigger(
        &rule,
        "Message",
        "created",
        &group_snapshot("hello")
    ));
}

#[test]
fn group_mention_only_requires_mention_marker() {
    let rule = rule(json!({
        "entity_type": "Message",
        "change": "created",
        "resolved_room_id": "!group:server",
        "group_mode": "mention_only"
    }));

    assert!(!matches_trigger(
        &rule,
        "Message",
        "created",
        &group_snapshot("hello")
    ));
    assert!(matches_trigger(
        &rule,
        "Message",
        "created",
        &group_snapshot("@Rasmus hello")
    ));
}

#[test]
fn exact_group_rule_does_not_match_outgoing_group_message() {
    let rule = rule(json!({
        "entity_type": "Message",
        "change": "created",
        "resolved_room_id": "!group:server",
        "group_mode": "all"
    }));
    let snapshot = json!({
        "platform": "whatsapp",
        "room_id": "!group:server",
        "sender_name": "You",
        "content": "sent by me",
        "is_group": true,
        "is_outgoing": true
    });

    assert!(!matches_trigger(&rule, "Message", "created", &snapshot));
}

#[test]
fn dm_message_still_matches_broad_message_rule() {
    let rule = rule(json!({
        "entity_type": "Message",
        "change": "created"
    }));
    let snapshot = json!({
        "platform": "whatsapp",
        "room_id": "!dm:server",
        "sender_name": "Alice",
        "content": "hello",
        "is_group": false
    });

    assert!(matches_trigger(&rule, "Message", "created", &snapshot));
}

#[test]
fn whatsapp_mute_end_time_semantics() {
    assert!(!whatsapp_mute_is_active(0, 100));
    assert!(!whatsapp_mute_is_active(99, 100));
    assert!(whatsapp_mute_is_active(101, 100));
    assert!(!whatsapp_mute_is_active(1_699_999_000_000, 1_700_000_000));
    assert!(whatsapp_mute_is_active(1_700_001_000_000, 1_700_000_000));
    assert!(whatsapp_mute_is_active(-1, 100));
}
