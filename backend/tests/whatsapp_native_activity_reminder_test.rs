use backend::jobs::scheduler::{
    whatsapp_native_activity_reminder_due, WHATSAPP_NATIVE_ACTIVITY_REMINDER_AFTER_SECS,
};
use backend::utils::bridge::is_whatsapp_native_activity_receipt;
use matrix_sdk::ruma::{
    event_id,
    events::receipt::{Receipt, ReceiptEventContent, ReceiptType},
    user_id,
};
use std::collections::BTreeMap;

const NOW: i32 = 2_000_000_000;

fn receipt_content(
    user: &matrix_sdk::ruma::UserId,
    receipt_type: ReceiptType,
) -> ReceiptEventContent {
    let mut users = BTreeMap::new();
    users.insert(user.to_owned(), Receipt::default());
    let mut receipts = BTreeMap::new();
    receipts.insert(receipt_type, users);
    ReceiptEventContent(BTreeMap::from([(
        event_id!("$native-read:example.com").to_owned(),
        receipts,
    )]))
}

#[test]
fn only_own_public_read_receipt_in_whatsapp_portal_counts_as_native_activity() {
    let own_user = user_id!("@lightfriend-user:example.com");
    let other_user = user_id!("@whatsappbot:example.com");
    let own_read = receipt_content(own_user, ReceiptType::Read);

    assert!(is_whatsapp_native_activity_receipt(
        &own_read,
        own_user,
        Some("whatsapp")
    ));
    assert!(!is_whatsapp_native_activity_receipt(
        &own_read,
        own_user,
        Some("telegram")
    ));
    assert!(!is_whatsapp_native_activity_receipt(
        &own_read,
        own_user,
        Some("signal")
    ));
    assert!(!is_whatsapp_native_activity_receipt(
        &own_read, own_user, None
    ));

    let bridge_or_other_user_read = receipt_content(other_user, ReceiptType::Read);
    assert!(!is_whatsapp_native_activity_receipt(
        &bridge_or_other_user_read,
        own_user,
        Some("whatsapp")
    ));

    let private_receipt = receipt_content(own_user, ReceiptType::ReadPrivate);
    assert!(!is_whatsapp_native_activity_receipt(
        &private_receipt,
        own_user,
        Some("whatsapp")
    ));
}

#[test]
fn reminder_becomes_due_at_twelve_days() {
    assert!(!whatsapp_native_activity_reminder_due(
        Some(NOW - WHATSAPP_NATIVE_ACTIVITY_REMINDER_AFTER_SECS + 1),
        None,
        NOW,
    ));
    assert!(whatsapp_native_activity_reminder_due(
        Some(NOW - WHATSAPP_NATIVE_ACTIVITY_REMINDER_AFTER_SECS),
        None,
        NOW,
    ));
}

#[test]
fn reminder_is_sent_only_once_per_inactivity_period() {
    let old_activity = NOW - WHATSAPP_NATIVE_ACTIVITY_REMINDER_AFTER_SECS;
    assert!(!whatsapp_native_activity_reminder_due(
        Some(old_activity),
        Some(old_activity + 1),
        NOW,
    ));

    let newer_activity = old_activity + 10;
    let later = newer_activity + WHATSAPP_NATIVE_ACTIVITY_REMINDER_AFTER_SECS;
    assert!(whatsapp_native_activity_reminder_due(
        Some(newer_activity),
        Some(old_activity + 1),
        later,
    ));
}

#[test]
fn missing_or_future_activity_is_not_eligible() {
    assert!(!whatsapp_native_activity_reminder_due(None, None, NOW));
    assert!(!whatsapp_native_activity_reminder_due(
        Some(NOW + 60),
        None,
        NOW,
    ));
}
