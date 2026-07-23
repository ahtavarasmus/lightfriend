use backend::services::metronome_billing::{
    cost_to_microusd, legacy_overage_migration_target, verify_webhook_signature,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;

fn sign(secret: &str, date: &str, body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(date.as_bytes());
    mac.update(b"\n");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

#[test]
fn verifies_exact_body_and_rejects_tampering() {
    let secret = "correct-horse-battery-staple";
    let date = "Mon, 02 Jan 2006 22:04:05 GMT";
    let body = br#"{"id":"evt_1","type":"payment_gate.payment_status"}"#;
    let signature = sign(secret, date, body);
    let now = chrono::DateTime::parse_from_rfc2822(date)
        .unwrap()
        .timestamp();

    verify_webhook_signature(secret, date, body, &signature, now).unwrap();
    assert!(verify_webhook_signature(secret, date, b"{}", &signature, now).is_err());
}

#[test]
fn rejects_webhooks_outside_the_five_minute_window() {
    let secret = "secret";
    let date = "Mon, 02 Jan 2006 22:04:05 GMT";
    let body = b"{}";
    let signature = sign(secret, date, body);
    let sent_at = chrono::DateTime::parse_from_rfc2822(date)
        .unwrap()
        .timestamp();

    assert!(verify_webhook_signature(secret, date, body, &signature, sent_at + 301).is_err());
}

#[test]
fn converts_fractional_dollar_costs_without_float_ledger_values() {
    assert_eq!(cost_to_microusd(0.013).unwrap(), 13_000);
    assert_eq!(cost_to_microusd(25.0).unwrap(), 25_000_000);
    assert!(cost_to_microusd(0.0).is_err());
    assert!(cost_to_microusd(f64::NAN).is_err());
}

#[test]
fn preserves_legacy_auto_topup_opt_in_when_payment_is_ready() {
    assert_eq!(
        legacy_overage_migration_target(true, true, false),
        Some(true)
    );
}

#[test]
fn keeps_overage_off_for_users_without_a_legacy_opt_in() {
    assert_eq!(
        legacy_overage_migration_target(false, true, false),
        Some(false)
    );
}

#[test]
fn waits_for_payment_setup_before_preserving_legacy_opt_in() {
    assert_eq!(legacy_overage_migration_target(true, false, false), None);
}

#[test]
fn never_reapplies_an_already_migrated_preference() {
    assert_eq!(legacy_overage_migration_target(true, true, true), None);
    assert_eq!(legacy_overage_migration_target(false, true, true), None);
}
