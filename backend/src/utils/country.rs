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

/// Check if a Stripe price ID is the Assistant plan (new tier)
pub fn is_assistant_plan_price(price_id: &str) -> bool {
    let ids = [
        std::env::var("STRIPE_ASSISTANT_PLAN_PRICE_ID").ok(),
        std::env::var("STRIPE_ASSISTANT_PLAN_PRICE_ID_US").ok(),
    ];
    ids.iter().filter_map(|p| p.as_ref()).any(|p| p == price_id)
}

/// Check if a Stripe price ID is the BYOT plan
pub fn is_byot_plan_price(price_id: &str) -> bool {
    // Legacy BYOT price ID for user on older subscription
    if price_id == "price_1RWavGKxKvG0CX8G9MFEIy93" {
        return true;
    }
    std::env::var("STRIPE_BYOT_PLAN_PRICE_ID")
        .map(|p| p == price_id)
        .unwrap_or(false)
}
