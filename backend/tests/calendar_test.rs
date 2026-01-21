//! Tests for Google Calendar pure functions

use backend::handlers::google_calendar::{
    calculate_end_time, calculate_event_duration_minutes, events_overlap, is_valid_time_range,
    parse_datetime_rfc3339,
};
use chrono::{TimeZone, Utc};

// ============================================================
// Events Overlap Tests
// ============================================================

#[test]
fn test_events_overlap_complete_overlap() {
    // Event completely within range
    let event_start = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let event_end = Utc.with_ymd_and_hms(2024, 1, 1, 11, 0, 0).unwrap();
    let range_start = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
    let range_end = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

    assert!(events_overlap(
        event_start,
        event_end,
        range_start,
        range_end
    ));
}

#[test]
fn test_events_overlap_partial_start() {
    // Event starts before range, ends during range
    let event_start = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
    let event_end = Utc.with_ymd_and_hms(2024, 1, 1, 11, 0, 0).unwrap();
    let range_start = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let range_end = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

    assert!(events_overlap(
        event_start,
        event_end,
        range_start,
        range_end
    ));
}

#[test]
fn test_events_overlap_partial_end() {
    // Event starts during range, ends after range
    let event_start = Utc.with_ymd_and_hms(2024, 1, 1, 11, 0, 0).unwrap();
    let event_end = Utc.with_ymd_and_hms(2024, 1, 1, 13, 0, 0).unwrap();
    let range_start = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let range_end = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

    assert!(events_overlap(
        event_start,
        event_end,
        range_start,
        range_end
    ));
}

#[test]
fn test_events_overlap_event_contains_range() {
    // Event spans beyond range on both sides
    let event_start = Utc.with_ymd_and_hms(2024, 1, 1, 8, 0, 0).unwrap();
    let event_end = Utc.with_ymd_and_hms(2024, 1, 1, 14, 0, 0).unwrap();
    let range_start = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let range_end = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

    assert!(events_overlap(
        event_start,
        event_end,
        range_start,
        range_end
    ));
}

#[test]
fn test_events_overlap_no_overlap_before() {
    // Event completely before range
    let event_start = Utc.with_ymd_and_hms(2024, 1, 1, 8, 0, 0).unwrap();
    let event_end = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
    let range_start = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let range_end = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

    assert!(!events_overlap(
        event_start,
        event_end,
        range_start,
        range_end
    ));
}

#[test]
fn test_events_overlap_no_overlap_after() {
    // Event completely after range
    let event_start = Utc.with_ymd_and_hms(2024, 1, 1, 13, 0, 0).unwrap();
    let event_end = Utc.with_ymd_and_hms(2024, 1, 1, 14, 0, 0).unwrap();
    let range_start = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let range_end = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

    assert!(!events_overlap(
        event_start,
        event_end,
        range_start,
        range_end
    ));
}

#[test]
fn test_events_overlap_adjacent_events_no_overlap() {
    // Event ends exactly when range starts - no overlap
    let event_start = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
    let event_end = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let range_start = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let range_end = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

    assert!(!events_overlap(
        event_start,
        event_end,
        range_start,
        range_end
    ));
}

#[test]
fn test_events_overlap_exact_match() {
    // Event exactly matches range
    let event_start = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let event_end = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
    let range_start = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let range_end = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

    assert!(events_overlap(
        event_start,
        event_end,
        range_start,
        range_end
    ));
}

// ============================================================
// Calculate Event Duration Tests
// ============================================================

#[test]
fn test_calculate_event_duration_one_hour() {
    let start = Some(Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap());
    let end = Some(Utc.with_ymd_and_hms(2024, 1, 1, 11, 0, 0).unwrap());

    assert_eq!(calculate_event_duration_minutes(start, end), 60);
}

#[test]
fn test_calculate_event_duration_30_minutes() {
    let start = Some(Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap());
    let end = Some(Utc.with_ymd_and_hms(2024, 1, 1, 10, 30, 0).unwrap());

    assert_eq!(calculate_event_duration_minutes(start, end), 30);
}

#[test]
fn test_calculate_event_duration_multi_hour() {
    let start = Some(Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap());
    let end = Some(Utc.with_ymd_and_hms(2024, 1, 1, 17, 0, 0).unwrap());

    assert_eq!(calculate_event_duration_minutes(start, end), 480); // 8 hours
}

#[test]
fn test_calculate_event_duration_no_start() {
    let end = Some(Utc.with_ymd_and_hms(2024, 1, 1, 11, 0, 0).unwrap());

    assert_eq!(calculate_event_duration_minutes(None, end), 0);
}

#[test]
fn test_calculate_event_duration_no_end() {
    let start = Some(Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap());

    assert_eq!(calculate_event_duration_minutes(start, None), 0);
}

#[test]
fn test_calculate_event_duration_both_none() {
    assert_eq!(calculate_event_duration_minutes(None, None), 0);
}

// ============================================================
// Parse DateTime Tests
// ============================================================

#[test]
fn test_parse_datetime_rfc3339_valid() {
    let result = parse_datetime_rfc3339("2024-01-15T10:30:00Z");
    assert!(result.is_ok());
    let dt = result.unwrap();
    assert_eq!(dt.year(), 2024);
    assert_eq!(dt.month(), 1);
    assert_eq!(dt.day(), 15);
}

#[test]
fn test_parse_datetime_rfc3339_with_timezone() {
    let result = parse_datetime_rfc3339("2024-01-15T10:30:00+02:00");
    assert!(result.is_ok());
    // Should be converted to UTC
    let dt = result.unwrap();
    assert_eq!(dt.hour(), 8); // 10:30 +02:00 = 08:30 UTC
}

#[test]
fn test_parse_datetime_rfc3339_invalid() {
    let result = parse_datetime_rfc3339("not-a-date");
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Invalid datetime format");
}

#[test]
fn test_parse_datetime_rfc3339_empty() {
    let result = parse_datetime_rfc3339("");
    assert!(result.is_err());
}

// ============================================================
// Valid Time Range Tests
// ============================================================

#[test]
fn test_is_valid_time_range_valid() {
    let start = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2024, 1, 1, 11, 0, 0).unwrap();

    assert!(is_valid_time_range(start, end));
}

#[test]
fn test_is_valid_time_range_same_time() {
    let time = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();

    assert!(!is_valid_time_range(time, time));
}

#[test]
fn test_is_valid_time_range_end_before_start() {
    let start = Utc.with_ymd_and_hms(2024, 1, 1, 11, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();

    assert!(!is_valid_time_range(start, end));
}

// ============================================================
// Calculate End Time Tests
// ============================================================

#[test]
fn test_calculate_end_time_30_minutes() {
    let start = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let end = calculate_end_time(start, 30);

    assert_eq!(end.hour(), 10);
    assert_eq!(end.minute(), 30);
}

#[test]
fn test_calculate_end_time_2_hours() {
    let start = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let end = calculate_end_time(start, 120);

    assert_eq!(end.hour(), 12);
    assert_eq!(end.minute(), 0);
}

#[test]
fn test_calculate_end_time_crosses_midnight() {
    let start = Utc.with_ymd_and_hms(2024, 1, 1, 23, 0, 0).unwrap();
    let end = calculate_end_time(start, 120);

    assert_eq!(end.day(), 2);
    assert_eq!(end.hour(), 1);
}

#[test]
fn test_calculate_end_time_zero_duration() {
    let start = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
    let end = calculate_end_time(start, 0);

    assert_eq!(end, start);
}

use chrono::Datelike;
use chrono::Timelike;
