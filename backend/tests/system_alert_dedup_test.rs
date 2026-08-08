use backend::proactive::system_behaviors::{
    alert_event_fingerprint, alerts_substantially_equivalent,
};

#[test]
fn equivalent_cross_channel_alerts_share_an_event_fingerprint() {
    let whatsapp = "Matti: Flight AY133 delayed until 19:30.";
    let email = "Finnair email: AY133 flight delay — new departure 19:30. [email_ref uid=44]";

    assert!(alerts_substantially_equivalent(whatsapp, email));
}

#[test]
fn delivery_feedback_suffix_does_not_change_equivalence() {
    let sent =
        "Bank: Card ending 4242 declined at Central Market.\n\nReply 1=worth it, 2=should wait.";
    let repeated = "Your bank card ending 4242 was declined at Central Market";

    assert!(alerts_substantially_equivalent(sent, repeated));
    assert!(!alert_event_fingerprint(sent).contains("reply"));
}

#[test]
fn different_real_world_events_are_not_suppressed() {
    let first = "Finnair: Flight AY133 delayed until 19:30.";
    let second = "Finnair: Flight AY134 delayed until 21:00.";

    assert!(!alerts_substantially_equivalent(first, second));
}

#[test]
fn distinct_events_from_the_same_sender_and_platform_are_not_suppressed() {
    let first = "Alice: Dad is locked out at home and needs the spare key.";
    let second = "Alice: The car broke down on Highway 4 and needs a tow.";

    assert!(!alerts_substantially_equivalent(first, second));
}
