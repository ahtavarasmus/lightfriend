use backend::handlers::whatsapp_auth::linked_device_attention_state;

const DAY: i32 = 24 * 60 * 60;

#[test]
fn linked_device_check_is_healthy_before_ten_days() {
    assert_eq!(linked_device_attention_state(0, 9 * DAY), "healthy");
}

#[test]
fn linked_device_check_is_at_risk_from_ten_days() {
    assert_eq!(linked_device_attention_state(0, 10 * DAY), "risk_soon");
    assert_eq!(linked_device_attention_state(0, 12 * DAY), "risk_soon");
}

#[test]
fn linked_device_check_needs_action_from_thirteen_days() {
    assert_eq!(linked_device_attention_state(0, 13 * DAY), "action_now");
}

#[test]
fn confirming_primary_phone_restores_healthy_state() {
    let now = 20 * DAY;
    assert_eq!(linked_device_attention_state(now, now), "healthy");
}
