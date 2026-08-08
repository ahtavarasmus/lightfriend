use backend::handlers::dashboard_handlers::Contact;
use backend::tool_call_utils::bridge::{select_cached_contact, CachedContactMatch};

fn contact(
    id: &str,
    name: &str,
    platform: &str,
    room_id: Option<&str>,
    chat_id: Option<&str>,
) -> Contact {
    Contact {
        id: id.to_string(),
        display_name: name.to_string(),
        subtitle: None,
        platform: Some(platform.to_string()),
        room_id: room_id.map(str::to_string),
        chat_id: chat_id.map(str::to_string),
        person_id: None,
        is_group: false,
        source: "bridge_room".to_string(),
    }
}

#[test]
fn cold_telegram_contact_resolves_from_picker_metadata_without_message_history() {
    let contacts = vec![contact(
        "tg_bridge:user:12345",
        "Alice Example",
        "telegram",
        None,
        Some("12345"),
    )];

    match select_cached_contact(&contacts, "telegram", "alice") {
        CachedContactMatch::Match(found) => {
            assert_eq!(found.chat_id.as_deref(), Some("12345"));
            assert!(found.room_id.is_none());
        }
        other => panic!("expected cold Telegram contact match, got {other:?}"),
    }
}

#[test]
fn cold_whatsapp_contact_resolves_from_the_same_picker_metadata() {
    let contacts = vec![contact(
        "wa_bridge:358401234567@s.whatsapp.net",
        "Bob Example",
        "whatsapp",
        None,
        Some("358401234567@s.whatsapp.net"),
    )];

    match select_cached_contact(&contacts, "whatsapp", "Bob Example") {
        CachedContactMatch::Match(found) => assert_eq!(
            found.chat_id.as_deref(),
            Some("358401234567@s.whatsapp.net")
        ),
        other => panic!("expected cold WhatsApp contact match, got {other:?}"),
    }
}

#[test]
fn similarly_named_distinct_routes_are_rejected_as_ambiguous() {
    let contacts = vec![
        contact("tg_bridge:user:1", "Alex", "telegram", None, Some("1")),
        contact("tg_bridge:user:2", "Alexa", "telegram", None, Some("2")),
    ];

    assert!(matches!(
        select_cached_contact(&contacts, "telegram", "Ale"),
        CachedContactMatch::Ambiguous(_)
    ));
}

#[test]
fn matching_is_platform_scoped() {
    let contacts = vec![contact(
        "tg_bridge:user:1",
        "Alice",
        "telegram",
        None,
        Some("1"),
    )];

    assert!(matches!(
        select_cached_contact(&contacts, "whatsapp", "Alice"),
        CachedContactMatch::None
    ));
}
