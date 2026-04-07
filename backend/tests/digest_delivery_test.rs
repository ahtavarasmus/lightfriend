//! Unit tests for digest slot matching across timezones.
//!
//! `should_deliver_now` is the pure helper at the heart of the scheduler's
//! delivery window check. Given a list of user-configured minute-of-day slots
//! and the *local* minute-of-day (computed by the caller from `now + tz_offset`),
//! it returns whether the current 10-minute slot matches any configured slot.
//!
//! These tests exercise the behavior with realistic timezone offsets to make
//! sure a user in UTC+5 or UTC-5 gets fired at their local 08:00, not server UTC.

use backend::jobs::scheduler::{parse_digest_times, should_deliver_now};

/// Helper: compute the local minute-of-day that a scheduler tick would see
/// given the UTC minute-of-day and the user's offset in minutes.
fn local_minute_of_day(utc_minute_of_day: u16, tz_offset_minutes: i32) -> u16 {
    let local = utc_minute_of_day as i32 + tz_offset_minutes;
    (((local % 1440) + 1440) % 1440) as u16
}

#[test]
fn fires_at_exact_slot_in_utc_plus_5() {
    // User in UTC+5 (e.g. Asia/Karachi) wants a digest at 08:00 local.
    // At UTC 03:00, local time is 08:00 → should fire.
    let slots = parse_digest_times("08:00").unwrap().1;
    let local = local_minute_of_day(180, 5 * 60); // UTC 03:00 + 5h = 08:00 local
    assert_eq!(local, 480);
    assert!(should_deliver_now(&slots, local));
}

#[test]
fn fires_at_exact_slot_in_utc_minus_5() {
    // User in UTC-5 (e.g. America/New_York, EST) wants a digest at 08:00 local.
    // At UTC 13:00, local time is 08:00 → should fire.
    let slots = parse_digest_times("08:00").unwrap().1;
    let local = local_minute_of_day(780, -5 * 60); // UTC 13:00 - 5h = 08:00 local
    assert_eq!(local, 480);
    assert!(should_deliver_now(&slots, local));
}

#[test]
fn does_not_fire_outside_slot() {
    let slots = parse_digest_times("08:00").unwrap().1;
    // Local 07:59 snaps to slot 450, not 480 → no fire
    assert!(!should_deliver_now(&slots, 479));
    // Local 08:10 snaps to slot 490, not 480 → no fire
    assert!(!should_deliver_now(&slots, 490));
}

#[test]
fn floor_snapping_covers_entire_10min_window() {
    // Anywhere in [08:00, 08:09] should fire for slot 480
    let slots = parse_digest_times("08:00").unwrap().1;
    for minute in 480..490 {
        assert!(
            should_deliver_now(&slots, minute),
            "slot 480 should fire for local minute {}",
            minute
        );
    }
    // 08:10 should NOT fire (next slot)
    assert!(!should_deliver_now(&slots, 490));
}

#[test]
fn fires_at_late_night_slot_23_50() {
    // Edge of the day — slot 1430 is 23:50
    let slots = parse_digest_times("23:50").unwrap().1;
    assert_eq!(slots, vec![1430]);
    assert!(should_deliver_now(&slots, 1430));
    assert!(should_deliver_now(&slots, 1439)); // 23:59 still in the 23:50 bucket
    assert!(!should_deliver_now(&slots, 1429)); // 23:49 belongs to 23:40 slot
}

#[test]
fn wraps_across_midnight_for_positive_offset() {
    // User in UTC+8. Scheduler tick at UTC 16:30. Local = 00:30 next day.
    // Slot 30 (00:30) should fire.
    let slots = parse_digest_times("00:30").unwrap().1;
    let local = local_minute_of_day(990, 8 * 60); // UTC 16:30 + 8h = 24:30 → wraps to 00:30
    assert_eq!(local, 30);
    assert!(should_deliver_now(&slots, local));
}

#[test]
fn wraps_across_midnight_for_negative_offset() {
    // User in UTC-8. Scheduler tick at UTC 05:30. Local = 21:30 previous day.
    let slots = parse_digest_times("21:30").unwrap().1;
    let local = local_minute_of_day(330, -8 * 60); // UTC 05:30 - 8h wraps to 21:30
    assert_eq!(local, 1290);
    assert!(should_deliver_now(&slots, local));
}

#[test]
fn fires_on_any_configured_slot_of_multiple() {
    let slots = parse_digest_times("08:00,12:00,18:00").unwrap().1;
    assert!(should_deliver_now(&slots, 480));
    assert!(should_deliver_now(&slots, 720));
    assert!(should_deliver_now(&slots, 1080));
    // In between slots → no fire
    assert!(!should_deliver_now(&slots, 600));
}

#[test]
fn empty_slots_never_fires() {
    let empty: Vec<u16> = vec![];
    for minute in 0..1440 {
        assert!(!should_deliver_now(&empty, minute));
    }
}
