//! Country detection and whitelist for notification-only service.
//!
//! This module handles:
//! 1. Detection of country from phone number prefix
//! 2. Distinction between local-number countries (full service) and notification-only countries

/// Countries with local Twilio numbers (existing, hardcoded pricing)
pub const LOCAL_NUMBER_COUNTRIES: &[(&str, &str)] = &[
    ("+1", "US"),    // US & Canada
    ("+358", "FI"),  // Finland
    ("+31", "NL"),   // Netherlands
    ("+44", "GB"),   // UK
    ("+61", "AU"),   // Australia
];

/// Countries supported for notification-only (US number, dynamic pricing via Twilio API)
pub const NOTIFICATION_ONLY_COUNTRIES: &[(&str, &str)] = &[
    ("+49", "DE"),   // Germany
    ("+33", "FR"),   // France
    ("+34", "ES"),   // Spain
    ("+39", "IT"),   // Italy
    ("+351", "PT"),  // Portugal
    ("+32", "BE"),   // Belgium
    ("+43", "AT"),   // Austria
    ("+41", "CH"),   // Switzerland
    ("+48", "PL"),   // Poland
    ("+420", "CZ"),  // Czech Republic
    ("+46", "SE"),   // Sweden
    ("+45", "DK"),   // Denmark
    ("+47", "NO"),   // Norway
    ("+353", "IE"),  // Ireland
    ("+64", "NZ"),   // New Zealand
];

/// Get ISO country code from phone number prefix
/// Returns Some(country_code) if the phone number is from a supported country,
/// None if unsupported
pub fn get_country_code_from_phone(phone: &str) -> Option<String> {
    // Check local number countries first (more specific prefixes first)
    // Sort by prefix length descending to match longer prefixes first
    let mut all_countries: Vec<(&str, &str)> = LOCAL_NUMBER_COUNTRIES
        .iter()
        .chain(NOTIFICATION_ONLY_COUNTRIES.iter())
        .copied()
        .collect();

    // Sort by prefix length descending (longer prefixes first for accurate matching)
    all_countries.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    for (prefix, code) in all_countries {
        if phone.starts_with(prefix) {
            return Some(code.to_string());
        }
    }
    None
}

/// Check if phone number is from a country with local Twilio numbers
/// These countries use hardcoded pricing
pub fn is_local_number_country(phone: &str) -> bool {
    // Sort by prefix length descending for accurate matching
    let mut sorted: Vec<_> = LOCAL_NUMBER_COUNTRIES.to_vec();
    sorted.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    sorted.iter().any(|(prefix, _)| phone.starts_with(prefix))
}

/// Check if phone number is from a notification-only country
/// These countries use dynamic Twilio API pricing
pub fn is_notification_only_country(phone: &str) -> bool {
    // Sort by prefix length descending for accurate matching
    let mut sorted: Vec<_> = NOTIFICATION_ONLY_COUNTRIES.to_vec();
    sorted.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    sorted.iter().any(|(prefix, _)| phone.starts_with(prefix))
}

/// Check if a country code is a notification-only country
pub fn is_notification_only_country_code(country: &str) -> bool {
    NOTIFICATION_ONLY_COUNTRIES.iter().any(|(_, code)| *code == country)
}

/// Check if a country code is eligible for €29/€59 euro plans
/// This includes all non-US/CA countries (local-number + notification-only)
pub fn is_euro_plan_country(country: &str) -> bool {
    matches!(country,
        "FI" | "NL" | "GB" | "AU" |  // local-number (non-US/CA)
        "DE" | "FR" | "ES" | "IT" | "PT" | "BE" | "AT" | "CH" |
        "PL" | "CZ" | "SE" | "DK" | "NO" | "IE" | "NZ"  // notification-only
    )
}

/// Check if a Stripe price ID is the Monitor plan (€29/mo, 40 messages)
pub fn is_monitor_plan_price(price_id: &str) -> bool {
    std::env::var("STRIPE_MONITOR_PLAN_PRICE_ID")
        .map(|p| p == price_id)
        .unwrap_or(false)
}

/// Check if a Stripe price ID is the Digest plan (€49/mo, 120 messages)
pub fn is_digest_plan_price(price_id: &str) -> bool {
    std::env::var("STRIPE_DIGEST_PLAN_PRICE_ID")
        .map(|p| p == price_id)
        .unwrap_or(false)
}

/// Check if a Stripe price ID is a legacy €19 euro plan (for migration)
pub fn is_legacy_euro_plan_price(price_id: &str) -> bool {
    // Check against all the old country-specific price IDs
    let legacy_price_ids = [
        std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_FI").ok(),
        std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_NL").ok(),
        std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_UK").ok(),
        std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_AU").ok(),
        std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_OTHER").ok(),
    ];

    legacy_price_ids.iter()
        .filter_map(|p| p.as_ref())
        .any(|p| p == price_id)
}

/// Check if a Stripe price ID is the BYOT plan (€19/mo, bring your own Twilio)
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
        assert!(is_local_number_country("+14155551234"));  // US
        assert!(is_local_number_country("+358401234567")); // Finland
        assert!(is_local_number_country("+31612345678"));  // Netherlands
        assert!(is_local_number_country("+447911123456")); // UK
        assert!(is_local_number_country("+61412345678"));  // Australia

        assert!(!is_local_number_country("+4915123456789")); // Germany - not local
    }

    #[test]
    fn test_notification_only_countries() {
        assert!(is_notification_only_country("+4915123456789")); // Germany
        assert!(is_notification_only_country("+33612345678"));   // France
        assert!(is_notification_only_country("+34612345678"));   // Spain
        assert!(is_notification_only_country("+39312345678"));   // Italy
        assert!(is_notification_only_country("+351912345678"));  // Portugal

        assert!(!is_notification_only_country("+14155551234")); // US - not notification-only
    }

    #[test]
    fn test_get_country_code() {
        assert_eq!(get_country_code_from_phone("+14155551234"), Some("US".to_string()));
        assert_eq!(get_country_code_from_phone("+4915123456789"), Some("DE".to_string()));
        assert_eq!(get_country_code_from_phone("+420123456789"), Some("CZ".to_string()));
        assert_eq!(get_country_code_from_phone("+351912345678"), Some("PT".to_string()));

        // Unsupported country
        assert_eq!(get_country_code_from_phone("+86123456789"), None); // China
    }

    #[test]
    fn test_longer_prefix_priority() {
        // +420 (Czech) should match before +42 (which doesn't exist but tests prefix logic)
        assert_eq!(get_country_code_from_phone("+420123456789"), Some("CZ".to_string()));
        // +351 (Portugal) should match correctly
        assert_eq!(get_country_code_from_phone("+351912345678"), Some("PT".to_string()));
    }
}
