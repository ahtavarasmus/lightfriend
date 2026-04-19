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

/// Determine plan type from a Stripe product ID.
/// Returns "assistant", "byot", or "autopilot" (default).
pub fn plan_type_from_product(product_id: &str) -> &'static str {
    if std::env::var("STRIPE_ASSISTANT_PRODUCT_ID")
        .map(|p| p == product_id)
        .unwrap_or(false)
    {
        return "assistant";
    }
    if std::env::var("STRIPE_BYOT_PRODUCT_ID")
        .map(|p| p == product_id)
        .unwrap_or(false)
    {
        return "byot";
    }
    // Autopilot is the default - any unrecognized product is autopilot
    "autopilot"
}

/// Check if a Stripe product ID is the BYOT plan.
pub fn is_byot_product(product_id: &str) -> bool {
    std::env::var("STRIPE_BYOT_PRODUCT_ID")
        .map(|p| p == product_id)
        .unwrap_or(false)
}
