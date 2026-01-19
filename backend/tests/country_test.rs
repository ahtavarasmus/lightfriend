//! Unit tests for country detection and whitelist utilities.
//!
//! Tests the phone number country detection and notification-only country logic.

use backend::utils::country::{
    get_country_code_from_phone, is_local_number_country, is_notification_only_country,
    is_notification_only_country_code,
};

#[test]
fn test_local_number_countries() {
    assert!(is_local_number_country("+14155551234")); // US
    assert!(is_local_number_country("+16475551234")); // CA (Toronto area code)
    assert!(is_local_number_country("+358401234567")); // Finland
    assert!(is_local_number_country("+31612345678")); // Netherlands
    assert!(is_local_number_country("+442071234567")); // UK (London landline, valid format)
    assert!(is_local_number_country("+61412345678")); // Australia

    assert!(!is_local_number_country("+4915123456789")); // Germany - not local
    assert!(!is_local_number_country("+819012345678")); // Japan - not local
}

#[test]
fn test_notification_only_countries() {
    // Traditional notification-only countries
    assert!(is_notification_only_country("+4915123456789")); // Germany
    assert!(is_notification_only_country("+33612345678")); // France
    assert!(is_notification_only_country("+34612345678")); // Spain
    assert!(is_notification_only_country("+393331234567")); // Italy (valid mobile format)
    assert!(is_notification_only_country("+351912345678")); // Portugal

    // Now ALL non-local countries are notification-only
    assert!(is_notification_only_country("+819012345678")); // Japan (valid mobile)
    assert!(is_notification_only_country("+8613812345678")); // China (valid mobile)
    assert!(is_notification_only_country("+5511987654321")); // Brazil
    assert!(is_notification_only_country("+972501234567")); // Israel

    // Local number countries are NOT notification-only
    assert!(!is_notification_only_country("+14155551234")); // US
    assert!(!is_notification_only_country("+16475551234")); // CA
}

#[test]
fn test_get_country_code() {
    // US vs CA detection via area codes
    assert_eq!(
        get_country_code_from_phone("+14155551234"),
        Some("US".to_string())
    ); // San Francisco
    assert_eq!(
        get_country_code_from_phone("+12125551234"),
        Some("US".to_string())
    ); // New York
    assert_eq!(
        get_country_code_from_phone("+16475551234"),
        Some("CA".to_string())
    ); // Toronto
    assert_eq!(
        get_country_code_from_phone("+15145551234"),
        Some("CA".to_string())
    ); // Montreal

    // European countries
    assert_eq!(
        get_country_code_from_phone("+4915123456789"),
        Some("DE".to_string())
    );
    assert_eq!(
        get_country_code_from_phone("+420123456789"),
        Some("CZ".to_string())
    );
    assert_eq!(
        get_country_code_from_phone("+351912345678"),
        Some("PT".to_string())
    );

    // Worldwide support - these all work now
    assert_eq!(
        get_country_code_from_phone("+81312345678"),
        Some("JP".to_string())
    ); // Japan
    assert_eq!(
        get_country_code_from_phone("+5511987654321"),
        Some("BR".to_string())
    ); // Brazil
    assert_eq!(
        get_country_code_from_phone("+972501234567"),
        Some("IL".to_string())
    ); // Israel
}

#[test]
fn test_notification_only_country_code() {
    // Local number countries are NOT notification-only
    assert!(!is_notification_only_country_code("US"));
    assert!(!is_notification_only_country_code("CA"));
    assert!(!is_notification_only_country_code("FI"));
    assert!(!is_notification_only_country_code("NL"));
    assert!(!is_notification_only_country_code("GB"));
    assert!(!is_notification_only_country_code("AU"));

    // All others ARE notification-only
    assert!(is_notification_only_country_code("DE"));
    assert!(is_notification_only_country_code("JP"));
    assert!(is_notification_only_country_code("BR"));
    assert!(is_notification_only_country_code("IL"));
}
