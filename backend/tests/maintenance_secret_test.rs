use axum::http::{HeaderMap, HeaderValue};
use backend::handlers::maintenance_handlers::check_secret;
use serial_test::serial;

#[test]
#[serial]
fn maintenance_secret_fails_closed_and_accepts_only_an_exact_match() {
    std::env::remove_var("MAINTENANCE_SECRET");
    assert!(!check_secret(&HeaderMap::new()));

    std::env::set_var("MAINTENANCE_SECRET", "a-long-random-maintenance-secret");

    let mut matching_headers = HeaderMap::new();
    matching_headers.insert(
        "X-Maintenance-Secret",
        HeaderValue::from_static("a-long-random-maintenance-secret"),
    );
    assert!(check_secret(&matching_headers));

    let mut wrong_headers = HeaderMap::new();
    wrong_headers.insert(
        "X-Maintenance-Secret",
        HeaderValue::from_static("a-long-random-maintenance-secreu"),
    );
    assert!(!check_secret(&wrong_headers));

    assert!(!check_secret(&HeaderMap::new()));

    std::env::remove_var("MAINTENANCE_SECRET");
}
