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

const DEFAULT_LEGACY_PRODUCT_CUTOFF_TS: i64 = 1_780_704_000; // 2026-06-06 00:00:00 UTC

fn env_product_matches(env_key: &str, product_id: &str) -> bool {
    std::env::var(env_key)
        .map(|p| p == product_id)
        .unwrap_or(false)
}

fn legacy_product_cutoff_ts() -> i64 {
    std::env::var("STRIPE_LEGACY_PRODUCT_CUTOFF_TS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_LEGACY_PRODUCT_CUTOFF_TS)
}

/// Determine plan type from a Stripe product ID.
///
/// Unknown pre-cutoff subscription products are treated as legacy Autopilot
/// subscriptions so existing customers keep access while their current Stripe
/// subscription remains active. Unknown post-cutoff products are rejected by
/// returning `None` instead of silently granting a plan.
pub fn plan_type_from_product(
    product_id: &str,
    subscription_created: Option<i64>,
) -> Option<&'static str> {
    use stripe_webhook_logic::plan_type_for_product;

    let product_kind = stripe_product_kind(product_id);
    let subscription_age = subscription_age_for_legacy_cutoff(subscription_created);
    plan_type_for_product(product_kind, subscription_age).map(|plan| plan.as_str())
}

pub fn stripe_product_kind(product_id: &str) -> stripe_webhook_logic::ProductKind {
    use stripe_webhook_logic::ProductKind;

    if env_product_matches("STRIPE_ASSISTANT_PRODUCT_ID", product_id) {
        return ProductKind::Assistant;
    }
    if env_product_matches("STRIPE_AUTOPILOT_PRODUCT_ID", product_id) {
        return ProductKind::Autopilot;
    }
    if env_product_matches("STRIPE_CREDITS_PRODUCT_ID", product_id) {
        return ProductKind::CreditsAddOn;
    }
    ProductKind::Unknown
}

pub fn subscription_age_for_legacy_cutoff(
    subscription_created: Option<i64>,
) -> stripe_webhook_logic::SubscriptionAge {
    use stripe_webhook_logic::SubscriptionAge;

    match subscription_created {
        Some(created) if created < legacy_product_cutoff_ts() => SubscriptionAge::PreLegacyCutoff,
        Some(_) => SubscriptionAge::AtOrAfterLegacyCutoff,
        None => SubscriptionAge::Missing,
    }
}
