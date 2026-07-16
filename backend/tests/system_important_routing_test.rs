use backend::proactive::utils::resolve_system_important_content_type;

#[test]
fn email_with_call_preference_routes_to_call_and_sms() {
    assert_eq!(
        resolve_system_important_content_type(Some("call"), false, "email"),
        "system_important_call"
    );
}

#[test]
fn email_with_sms_preference_routes_to_sms() {
    assert_eq!(
        resolve_system_important_content_type(Some("sms"), false, "email"),
        "system_important_sms"
    );
}

#[test]
fn known_chat_contact_keeps_call_escalation() {
    assert_eq!(
        resolve_system_important_content_type(Some("sms"), true, "whatsapp"),
        "system_important_call"
    );
}
