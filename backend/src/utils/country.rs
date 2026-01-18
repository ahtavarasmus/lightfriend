//! Country detection and whitelist for notification-only service.
//!
//! This module handles:
//! 1. Detection of country from phone number using libphonenumber (handles all countries including US vs CA)
//! 2. Distinction between local-number countries (full service) and notification-only countries (all others)

/// Countries with local Twilio numbers (we have dedicated phone numbers for these)
const LOCAL_NUMBER_COUNTRY_CODES: &[&str] = &["US", "CA", "FI", "NL", "GB", "AU"];

/// Get ISO country code from any phone number worldwide.
/// Uses libphonenumber which handles US vs CA area code detection automatically.
pub fn get_country_code_from_phone(phone: &str) -> Option<String> {
    phonenumber::parse(None, phone)
        .ok()
        .and_then(|p| p.country().id())
        .map(|id| id.as_ref().to_string())
}

/// Check if phone number is from a country with local Twilio numbers.
/// These countries use hardcoded pricing and have dedicated Twilio numbers.
pub fn is_local_number_country(phone: &str) -> bool {
    matches!(
        get_country_code_from_phone(phone).as_deref(),
        Some("US" | "CA" | "FI" | "NL" | "GB" | "AU")
    )
}

/// Check if phone number is from a notification-only country.
/// Returns true for ANY valid phone number that's not a local-number country.
/// These countries use dynamic Twilio API pricing and messages are sent via US messaging service.
pub fn is_notification_only_country(phone: &str) -> bool {
    if is_local_number_country(phone) {
        return false;
    }
    // Any valid phone number that's not a local-number country is notification-only
    get_country_code_from_phone(phone).is_some()
}

/// Check if a country code is a notification-only country.
/// Returns true for ANY country code that's not a local-number country.
pub fn is_notification_only_country_code(country: &str) -> bool {
    !LOCAL_NUMBER_COUNTRY_CODES.contains(&country)
}

/// Check if a country code is eligible for euro plans.
/// This includes all non-US/CA countries.
pub fn is_euro_plan_country(country: &str) -> bool {
    !matches!(country, "US" | "CA")
}

/// Check if a Stripe price ID is the Monitor plan
pub fn is_monitor_plan_price(price_id: &str) -> bool {
    std::env::var("STRIPE_MONITOR_PLAN_PRICE_ID")
        .map(|p| p == price_id)
        .unwrap_or(false)
}

/// Check if a Stripe price ID is the Digest plan
pub fn is_digest_plan_price(price_id: &str) -> bool {
    std::env::var("STRIPE_DIGEST_PLAN_PRICE_ID")
        .map(|p| p == price_id)
        .unwrap_or(false)
}

/// Check if a Stripe price ID is a legacy euro plan (for migration)
pub fn is_legacy_euro_plan_price(price_id: &str) -> bool {
    let legacy_price_ids = [
        std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_FI").ok(),
        std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_NL").ok(),
        std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_UK").ok(),
        std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_AU").ok(),
        std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_OTHER").ok(),
    ];

    legacy_price_ids
        .iter()
        .filter_map(|p| p.as_ref())
        .any(|p| p == price_id)
}

/// Check if a Stripe price ID is the BYOT plan
pub fn is_byot_plan_price(price_id: &str) -> bool {
    std::env::var("STRIPE_BYOT_PLAN_PRICE_ID")
        .map(|p| p == price_id)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_euro_plan_country() {
        // US/CA are NOT euro plan countries
        assert!(!is_euro_plan_country("US"));
        assert!(!is_euro_plan_country("CA"));

        // All others are euro plan countries
        assert!(is_euro_plan_country("FI"));
        assert!(is_euro_plan_country("NL"));
        assert!(is_euro_plan_country("GB"));
        assert!(is_euro_plan_country("AU"));
        assert!(is_euro_plan_country("DE"));
        assert!(is_euro_plan_country("JP"));
        assert!(is_euro_plan_country("BR"));
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
}
