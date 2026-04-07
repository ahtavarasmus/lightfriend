//! Unit tests for country detection service.
//!
//! Tests the phone number country detection logic that distinguishes
//! US from CA (both use +1 prefix) and handles other countries.

use backend::services::country_service::detect_country;

#[test]
fn test_us_detection() {
    // San Francisco
    assert_eq!(detect_country("+14155551234"), Some("US".to_string()));
    // New York
    assert_eq!(detect_country("+12125551234"), Some("US".to_string()));
    // Los Angeles
    assert_eq!(detect_country("+13105551234"), Some("US".to_string()));
}

#[test]
fn test_ca_detection() {
    // Toronto
    assert_eq!(detect_country("+14165551234"), Some("CA".to_string()));
    // Vancouver
    assert_eq!(detect_country("+16045551234"), Some("CA".to_string()));
    // Montreal
    assert_eq!(detect_country("+15145551234"), Some("CA".to_string()));
}

#[test]
fn test_european_detection() {
    // Finland
    assert_eq!(detect_country("+358401234567"), Some("FI".to_string()));
    // Germany
    assert_eq!(detect_country("+4915123456789"), Some("DE".to_string()));
    // UK (London landline - unambiguous)
    assert_eq!(detect_country("+442071234567"), Some("GB".to_string()));
    // Netherlands
    assert_eq!(detect_country("+31612345678"), Some("NL".to_string()));
}

#[test]
fn test_worldwide_detection() {
    // China - now supported via libphonenumber
    assert_eq!(detect_country("+8613812345678"), Some("CN".to_string()));
    // Japan - now supported via libphonenumber
    assert_eq!(detect_country("+819012345678"), Some("JP".to_string()));
}

#[test]
fn test_unknown_north_american() {
    // Invalid +1 area code
    assert_eq!(detect_country("+11115551234"), None);
}
