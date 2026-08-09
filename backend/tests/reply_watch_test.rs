use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use backend::tools::messaging::arm_wait_for_reply;
use serial_test::serial;

#[test]
#[serial]
fn standalone_watch_matches_only_target_room_and_is_claimed_once() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    state
        .ontology_repository
        .upsert_person(
            user.id,
            "Anna",
            "whatsapp",
            Some("anna@wa"),
            Some("!anna:example.org"),
        )
        .unwrap();

    let confirmation = arm_wait_for_reply(&state, user.id, "Anna", None);
    assert_eq!(
        confirmation,
        "Watching for Anna's next WhatsApp reply for 24 hours."
    );

    // An unrelated incoming conversation must not consume or trigger Anna's watch.
    assert!(state
        .pending_reply_watches_repository
        .claim_active_bridge(user.id, "!someone-else:example.org")
        .unwrap()
        .is_none());
    assert!(state
        .pending_reply_watches_repository
        .find_active_bridge(user.id, "!anna:example.org")
        .unwrap()
        .is_some());

    // The matching incoming reply atomically takes the watch. A second reply
    // cannot trigger it again because automatic removal happened in the claim.
    let claimed = state
        .pending_reply_watches_repository
        .claim_active_bridge(user.id, "!anna:example.org")
        .unwrap()
        .expect("Anna's next reply should claim the watch");
    assert_eq!(claimed.contact_display_name, "Anna");
    assert!(state
        .pending_reply_watches_repository
        .claim_active_bridge(user.id, "!anna:example.org")
        .unwrap()
        .is_none());
    assert!(state
        .pending_reply_watches_repository
        .find_active_bridge(user.id, "!anna:example.org")
        .unwrap()
        .is_none());
}

#[test]
#[serial]
fn rearming_same_conversation_still_fires_only_once() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    state
        .pending_reply_watches_repository
        .arm_bridge(user.id, "!anna:example.org", "anna@wa", "Anna")
        .unwrap();
    state
        .pending_reply_watches_repository
        .arm_bridge(user.id, "!anna:example.org", "anna@wa", "Anna")
        .unwrap();

    assert!(state
        .pending_reply_watches_repository
        .claim_active_bridge(user.id, "!anna:example.org")
        .unwrap()
        .is_some());
    assert!(state
        .pending_reply_watches_repository
        .claim_active_bridge(user.id, "!anna:example.org")
        .unwrap()
        .is_none());
}

#[test]
#[serial]
fn ambiguous_contact_or_conversation_requests_clarification() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    state
        .ontology_repository
        .upsert_person(
            user.id,
            "Anna Stone",
            "whatsapp",
            Some("stone@wa"),
            Some("!stone:example.org"),
        )
        .unwrap();
    state
        .ontology_repository
        .upsert_person(
            user.id,
            "Anna Jones",
            "telegram",
            Some("12345"),
            Some("!jones:example.org"),
        )
        .unwrap();

    let people_clarification = arm_wait_for_reply(&state, user.id, "Anna", None);
    assert!(people_clarification.starts_with("Which contact did you mean:"));
    assert!(people_clarification.contains("Anna Stone"));
    assert!(people_clarification.contains("Anna Jones"));

    state
        .ontology_repository
        .upsert_person(
            user.id,
            "Sam",
            "whatsapp",
            Some("sam@wa"),
            Some("!sam-wa:example.org"),
        )
        .unwrap();
    state
        .ontology_repository
        .upsert_person(
            user.id,
            "Sam",
            "telegram",
            Some("67890"),
            Some("!sam-tg:example.org"),
        )
        .unwrap();

    let platform_clarification = arm_wait_for_reply(&state, user.id, "Sam", None);
    assert_eq!(
        platform_clarification,
        "Which conversation should I watch for Sam: WhatsApp, Telegram?"
    );
    assert_eq!(
        arm_wait_for_reply(&state, user.id, "Sam", Some("telegram")),
        "Watching for Sam's next Telegram reply for 24 hours."
    );
}

#[test]
fn wait_for_reply_is_available_to_the_conversational_agent() {
    let registry = backend::build_tool_registry();
    assert!(registry.get("wait_for_reply").is_some());
}

#[test]
#[serial]
fn dashboard_listing_and_cancellation_are_user_scoped() {
    let state = create_test_state();
    let owner = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let other = create_test_user(&state, &TestUserParams::finland_user(10.0, 5.0));
    let watch = state
        .pending_reply_watches_repository
        .arm_bridge(owner.id, "!anna:example.org", "anna@wa", "Anna")
        .unwrap();
    let now = chrono::Utc::now().timestamp() as i32;

    assert_eq!(
        state
            .pending_reply_watches_repository
            .active_for_user(owner.id, now)
            .unwrap()
            .len(),
        1
    );
    assert!(state
        .pending_reply_watches_repository
        .active_for_user(other.id, now)
        .unwrap()
        .is_empty());
    assert!(!state
        .pending_reply_watches_repository
        .delete_for_user(other.id, watch.id)
        .unwrap());
    assert!(state
        .pending_reply_watches_repository
        .delete_for_user(owner.id, watch.id)
        .unwrap());
}
