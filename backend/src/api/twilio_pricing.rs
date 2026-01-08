/// Twilio Pricing helpers for euro plan countries.
///
/// This module provides calculated pricing for euro countries using Twilio's
/// pricing API with segment-based multipliers:
/// - Notifications: Twilio price × 1.5 × 1.3 (1.5 segments avg, some are longer)
/// - Regular messages: Twilio price × 3 × 1.3 (3 segments, typical conversation)
/// - Digests: Twilio price × 3 × 1.3 (3 segments avg, varies by content)
/// - Voice: Twilio price × 1.3 (per minute)
use crate::api::twilio_availability::get_country_capability;
use crate::AppState;
use std::sync::Arc;

/// Segment multipliers for different message types
const NOTIFICATION_SEGMENT_MULTIPLIER: f32 = 1.5;  // Notifications: 1.5 segments avg
const REGULAR_MSG_SEGMENT_MULTIPLIER: f32 = 3.0;   // Typical messages: 3 segments
const DIGEST_SEGMENT_MULTIPLIER: f32 = 3.0;        // Digests: 3 segments avg

/// VAT/margin multiplier: 30%
const VAT_MARGIN_MULTIPLIER: f32 = 1.3;

/// Pricing result for euro plan countries with segment-based pricing
#[derive(Debug, Clone)]
pub struct NotificationPricing {
    /// Price for notifications: raw × 1.5 × 1.3 (1.5 segments avg)
    pub notification_price: f32,
    /// Price for regular messages: raw × 3 × 1.3 (3 segments)
    pub regular_message_price: f32,
    /// Price for digests: raw × 3 × 1.3 (3 segments avg)
    pub digest_price: f32,
    /// Calculated voice price per minute: raw × 1.3
    pub calculated_voice_price: f32,
    /// Legacy field - same as regular_message_price for backwards compatibility
    pub calculated_sms_price: f32,
}

impl NotificationPricing {
    /// Create pricing from raw Twilio prices with segment-based multipliers
    pub fn from_raw_prices(sms_price: f32, voice_price: f32) -> Self {
        let notification = sms_price * NOTIFICATION_SEGMENT_MULTIPLIER * VAT_MARGIN_MULTIPLIER;
        let regular = sms_price * REGULAR_MSG_SEGMENT_MULTIPLIER * VAT_MARGIN_MULTIPLIER;
        let digest = sms_price * DIGEST_SEGMENT_MULTIPLIER * VAT_MARGIN_MULTIPLIER;
        let voice = voice_price * VAT_MARGIN_MULTIPLIER;

        Self {
            notification_price: notification,
            regular_message_price: regular,
            digest_price: digest,
            calculated_voice_price: voice,
            // Legacy: use regular message price as default
            calculated_sms_price: regular,
        }
    }
}

/// Get pricing for a country with segment-based multipliers.
///
/// This fetches pricing from Twilio (with 24h cache) and applies
/// segment-based formulas:
/// - Notifications: raw_price × 1.5 × 1.3 (1.5 segments avg)
/// - Regular messages: raw_price × 3 × 1.3 (3 segments)
/// - Digests: raw_price × 3 × 1.3 (3 segments avg)
/// - Voice: raw_price × 1.3 (per minute, first minute always charged in full)
///
/// Returns Err if the country is not supported or pricing is unavailable.
pub async fn get_notification_only_pricing(
    state: &Arc<AppState>,
    country_code: &str,
) -> Result<NotificationPricing, String> {
    let capability = get_country_capability(state, country_code).await?;

    // Get raw prices, defaulting to reasonable fallbacks if not available
    let raw_sms = capability.outbound_sms_price.unwrap_or(0.10);
    let raw_voice = capability.outbound_voice_price_per_min.unwrap_or(0.10);

    Ok(NotificationPricing::from_raw_prices(raw_sms, raw_voice))
}

/// Get pricing for any euro plan country (local-number or notification-only).
///
/// This is a unified function that works for all non-US/CA countries:
/// - Local-number countries: FI, NL, GB, AU
/// - Notification-only countries: DE, FR, ES, IT, PT, BE, AT, CH, PL, CZ, SE, DK, NO, IE, NZ
///
/// Uses segment-based formulas for SMS and voice pricing
pub async fn get_euro_country_pricing(
    state: &Arc<AppState>,
    country_code: &str,
) -> Result<NotificationPricing, String> {
    // Use the same function as notification-only pricing
    // The Twilio API works for all countries
    get_notification_only_pricing(state, country_code).await
}

/// Get euro country pricing from cache synchronously.
/// Works for all euro plan countries (local-number + notification-only).
/// Returns None if not cached or on error - caller should use fallback pricing.
pub fn get_cached_euro_pricing_sync(
    state: &Arc<AppState>,
    country_code: &str,
) -> Option<NotificationPricing> {
    get_cached_notification_pricing_sync(state, country_code)
}

/// Get notification-only pricing from cache synchronously.
/// Used by sync functions like deduct_user_credits.
/// Returns None if not cached or on error - caller should use fallback pricing.
pub fn get_cached_notification_pricing_sync(
    state: &Arc<AppState>,
    country_code: &str,
) -> Option<NotificationPricing> {
    use crate::schema::country_availability;
    use crate::models::user_models::CountryAvailability;
    use diesel::prelude::*;
    use chrono::Utc;

    let now = Utc::now().timestamp() as i32;
    let cache_duration = 86400; // 24 hours

    let mut conn = state.db_pool.get().ok()?;

    let cached: CountryAvailability = country_availability::table
        .filter(country_availability::country_code.eq(country_code.to_uppercase()))
        .filter(country_availability::last_checked.gt(now - cache_duration))
        .select(CountryAvailability::as_select())
        .first(&mut conn)
        .ok()?;

    let raw_sms = cached.outbound_sms_price.unwrap_or(0.10);
    let raw_voice = cached.outbound_voice_price_per_min.unwrap_or(0.10);

    Some(NotificationPricing::from_raw_prices(raw_sms, raw_voice))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_based_pricing() {
        // Test with typical German pricing: $0.08 SMS, $0.04 voice
        let pricing = NotificationPricing::from_raw_prices(0.08, 0.04);

        // Notification: 0.08 × 1.5 × 1.3 = 0.156
        assert!((pricing.notification_price - 0.156).abs() < 0.001);

        // Regular: 0.08 × 3 × 1.3 = 0.312
        assert!((pricing.regular_message_price - 0.312).abs() < 0.001);

        // Digest: 0.08 × 3 × 1.3 = 0.312
        assert!((pricing.digest_price - 0.312).abs() < 0.001);

        // Voice: 0.04 × 1.3 = 0.052
        assert!((pricing.calculated_voice_price - 0.052).abs() < 0.001);

        // Legacy field should equal regular message price
        assert!((pricing.calculated_sms_price - pricing.regular_message_price).abs() < 0.001);
    }

    #[test]
    fn test_pricing_from_raw() {
        let pricing = NotificationPricing::from_raw_prices(0.10, 0.05);

        assert_eq!(pricing.raw_sms_price, 0.10);
        assert_eq!(pricing.raw_voice_price, 0.05);

        // Notification: 0.10 × 1.5 × 1.3 = 0.195
        assert!((pricing.notification_price - 0.195).abs() < 0.001);

        // Regular: 0.10 × 3 × 1.3 = 0.39
        assert!((pricing.regular_message_price - 0.39).abs() < 0.001);

        // Digest: 0.10 × 3 × 1.3 = 0.39
        assert!((pricing.digest_price - 0.39).abs() < 0.001);

        // Voice: 0.05 × 1.3 = 0.065
        assert!((pricing.calculated_voice_price - 0.065).abs() < 0.001);
    }

    #[test]
    fn test_effective_message_counts() {
        // With €15.60 allocation (50 regular messages at 0.312)
        let pricing = NotificationPricing::from_raw_prices(0.08, 0.04);
        let allocation = 50.0 * pricing.regular_message_price; // ~15.6

        // How many of each type can you get?
        let notifications = (allocation / pricing.notification_price).floor();
        let regular = (allocation / pricing.regular_message_price).floor();
        let digests = (allocation / pricing.digest_price).floor();

        assert_eq!(notifications, 100.0); // 15.6 / 0.156 = 100
        assert_eq!(regular, 50.0);        // 15.6 / 0.312 = 50
        assert_eq!(digests, 50.0);        // 15.6 / 0.312 = 50 (same as regular now)
    }
}
