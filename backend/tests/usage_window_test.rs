use backend::utils::usage::{current_included_usage_window, INCLUDED_USAGE_WINDOW_SECONDS};

#[test]
fn active_usage_window_is_left_unchanged() {
    let now = 1_700_000_000;
    let start = now - 1_000;
    let end = now + 1_000;

    let (window, should_reset) = current_included_usage_window(now, Some(start), Some(end));

    assert_eq!(window.start, start);
    assert_eq!(window.end, end);
    assert!(!should_reset);
}

#[test]
fn missing_usage_window_starts_at_now() {
    let now = 1_700_000_000;

    let (window, should_reset) = current_included_usage_window(now, None, None);

    assert_eq!(window.start, now);
    assert_eq!(window.end, now + INCLUDED_USAGE_WINDOW_SECONDS);
    assert!(should_reset);
}

#[test]
fn expired_usage_window_advances_until_current() {
    let start = 1_700_000_000;
    let end = start + INCLUDED_USAGE_WINDOW_SECONDS;
    let now = end + INCLUDED_USAGE_WINDOW_SECONDS + 60;

    let (window, should_reset) = current_included_usage_window(now, Some(start), Some(end));

    assert_eq!(window.start, start + (2 * INCLUDED_USAGE_WINDOW_SECONDS));
    assert_eq!(window.end, start + (3 * INCLUDED_USAGE_WINDOW_SECONDS));
    assert!(should_reset);
}
