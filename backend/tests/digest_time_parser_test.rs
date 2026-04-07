//! Unit tests for the custom digest-time parser.
//!
//! Verifies that user-supplied digest times are parsed, validated, snapped to
//! 10-minute boundaries (matching the scheduler cadence), deduplicated, and
//! sorted into canonical form.

use backend::jobs::scheduler::parse_digest_times;

#[test]
fn parses_simple_hh_mm_list() {
    let (canonical, slots) = parse_digest_times("08:00,18:00").unwrap();
    assert_eq!(canonical, "08:00,18:00");
    assert_eq!(slots, vec![480, 1080]);
}

#[test]
fn snaps_to_nearest_ten_minutes() {
    // 08:53 -> 08:50 (rounds down because 3 < 5)
    let (canonical, slots) = parse_digest_times("08:53").unwrap();
    assert_eq!(canonical, "08:50");
    assert_eq!(slots, vec![530]);

    // 08:55 -> 09:00 (rounds up at boundary)
    let (canonical, slots) = parse_digest_times("08:55").unwrap();
    assert_eq!(canonical, "09:00");
    assert_eq!(slots, vec![540]);

    // 08:57 -> 09:00
    let (canonical, _) = parse_digest_times("08:57").unwrap();
    assert_eq!(canonical, "09:00");
}

#[test]
fn handles_already_aligned_times() {
    let (canonical, _) = parse_digest_times("08:50,09:00,09:10").unwrap();
    assert_eq!(canonical, "08:50,09:00,09:10");
}

#[test]
fn deduplicates_and_sorts() {
    let (canonical, slots) = parse_digest_times("18:00, 08:00, 08:00, 12:00").unwrap();
    assert_eq!(canonical, "08:00,12:00,18:00");
    assert_eq!(slots, vec![480, 720, 1080]);
}

#[test]
fn deduplicates_after_snapping() {
    // 08:52 and 08:54 both snap to 08:50
    let (canonical, slots) = parse_digest_times("08:52,08:54").unwrap();
    assert_eq!(canonical, "08:50");
    assert_eq!(slots, vec![530]);
}

#[test]
fn accepts_bare_hour() {
    let (canonical, slots) = parse_digest_times("8,18").unwrap();
    assert_eq!(canonical, "08:00,18:00");
    assert_eq!(slots, vec![480, 1080]);
}

#[test]
fn trims_whitespace() {
    let (canonical, _) = parse_digest_times("  08:00  ,  18:00  ").unwrap();
    assert_eq!(canonical, "08:00,18:00");
}

#[test]
fn rejects_no_colon_concat_format() {
    // The bug that started this fix: "0850" was being parsed as hour=850 and silently dropped.
    // It must now error explicitly.
    let result = parse_digest_times("0850,0900,0910,1000,1100");
    assert!(result.is_err(), "Should reject HHMM format without colons");
    let err = result.unwrap_err();
    assert!(
        err.contains("hour must be 0-23") || err.contains("invalid"),
        "Error should explain the issue, got: {}",
        err
    );
}

#[test]
fn rejects_invalid_hour() {
    assert!(parse_digest_times("25:00").is_err());
    assert!(parse_digest_times("99:00").is_err());
}

#[test]
fn rejects_invalid_minute() {
    assert!(parse_digest_times("08:60").is_err());
    assert!(parse_digest_times("08:99").is_err());
}

#[test]
fn rejects_non_numeric() {
    assert!(parse_digest_times("morning").is_err());
    assert!(parse_digest_times("ab:cd").is_err());
}

#[test]
fn rejects_empty_input() {
    assert!(parse_digest_times("").is_err());
    assert!(parse_digest_times("   ").is_err());
    assert!(parse_digest_times(",,,").is_err());
}

#[test]
fn handles_midnight_wrap() {
    // 23:58 -> snaps to 24:00 -> wraps to 00:00
    let (canonical, slots) = parse_digest_times("23:58").unwrap();
    assert_eq!(canonical, "00:00");
    assert_eq!(slots, vec![0]);
}

#[test]
fn parses_up_to_four_slots() {
    let (canonical, slots) = parse_digest_times("08:00,12:00,15:00,20:00").unwrap();
    assert_eq!(canonical, "08:00,12:00,15:00,20:00");
    assert_eq!(slots, vec![480, 720, 900, 1200]);
}

#[test]
fn rejects_more_than_four_slots() {
    let result = parse_digest_times("08:00,10:00,12:00,15:00,20:00");
    assert!(result.is_err(), "Should reject more than 4 slots");
    let err = result.unwrap_err();
    assert!(
        err.contains("max 4"),
        "Error should mention the 4-slot cap, got: {}",
        err
    );
}

#[test]
fn cap_applies_after_dedup() {
    // 5 raw slots but two snap to the same value, so 4 unique → should pass
    let (canonical, slots) = parse_digest_times("08:00,08:02,12:00,15:00,20:00").unwrap();
    assert_eq!(canonical, "08:00,12:00,15:00,20:00");
    assert_eq!(slots.len(), 4);
}
