use crate::api::twilio_client::{TwilioClient, TwilioCredentials};
use crate::models::user_models::{CountryAvailability, NewCountryAvailability};
use crate::schema::country_availability;
use crate::AppState;
use chrono::Utc;
use diesel::prelude::*;
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountryTier {
    UsCanada,         // Full service, unlimited usage with monitoring
    FullService,      // Local number available in country
    NotificationOnly, // No local number, but can send from US number
    NotSupported,     // Cannot send messages to this country at all
}

#[derive(Debug, Serialize)]
pub struct CountryCapabilityInfo {
    pub available: bool,
    pub plan_type: String, // "us_ca", "full_service", "notification_only"
    pub can_receive_sms: bool,
    pub outbound_sms_price: Option<f32>,
    pub inbound_sms_price: Option<f32>,
    pub outbound_voice_price_per_min: Option<f32>,
    pub inbound_voice_price_per_min: Option<f32>,
}


/// Countries where we can send messages from a US number without issues
/// Based on Twilio's A2P 10DLC coverage and practical experience
const NOTIFICATION_SUPPORTED_COUNTRIES: &[&str] = &[
    // Americas
    "CA", "MX", "BR", "AR", "CL", "CO", "PE", "VE", "EC", "GT", "CU", "BO", "HT", "DO", "HN", "PY",
    "NI", "SV", "CR", "PA", "UY", "PR", "JM", "TT", "GY", "SR", "GF", "BZ", "BS", "BB", "GD", "LC",
    "VC", "AG", "DM", "KN", "AW", "CW", "SX", "BQ", "TC", "VG", "KY", "BM", "AI", "MS", "FK", "GS",
    "PM", // Europe
    "GB", "DE", "FR", "IT", "ES", "NL", "BE", "SE", "NO", "DK", "FI", "PL", "GR", "PT", "CZ", "RO",
    "HU", "AT", "CH", "IE", "SK", "BG", "HR", "LT", "SI", "LV", "EE", "CY", "LU", "MT", "IS", "AL",
    "RS", "BA", "MK", "ME", "XK", "MD", "BY", "UA", "GE", "AM", "AZ", "LI", "MC", "SM", "VA", "AD",
    "GI", "JE", "GG", "IM", "FO", "AX", // Asia-Pacific
    "AU", "NZ", "JP", "KR", "SG", "HK", "MY", "TH", "PH", "ID", "VN", "TW", "IN", "PK", "BD", "LK",
    "NP", "MM", "KH", "LA", "BN", "MO", "MV", "BT", "TL", "MN", "FJ", "PG", "NC", "PF", "GU", "MP",
    "AS", "WS", "PW", "FM", "MH", "KI", "NR", "TV", "TO", "VU", "SB", "CK", "NU", "TK", "WF",
    // Middle East
    "IL", "TR", "SA", "QA", "KW", "BH", "OM", "JO", "LB", "PS", "YE", "IQ", "SY", "CY",
    // Africa
    "ZA", "NG", "EG", "KE", "GH", "TZ", "UG", "ZW", "MU", "RW", "ET", "MA", "TN", "DZ", "SN", "CI",
    "CM", "MG", "BW", "NA", "MW", "ZM", "MZ", "AO", "SD", "SL", "LR", "BJ", "TG", "BF", "ML", "NE",
    "TD", "SS", "CF", "CG", "GA", "GQ", "ST", "CV", "GM", "GW", "GN", "BI", "DJ", "ER", "SO", "SC",
    "KM", "YT", "RE", "MU", "LS", "SZ",
];

/// Check if a country supports local phone numbers using TwilioClient
async fn check_local_numbers_available(
    twilio_client: &dyn TwilioClient,
    credentials: &TwilioCredentials,
    country_code: &str,
) -> Result<bool, String> {
    twilio_client
        .check_phone_numbers_available(credentials, country_code)
        .await
        .map_err(|e| e.to_string())
}

/// Fetch messaging and voice pricing for a country using TwilioClient
async fn fetch_pricing(
    twilio_client: &dyn TwilioClient,
    credentials: &TwilioCredentials,
    country_code: &str,
) -> Result<(Option<f32>, Option<f32>, Option<f32>, Option<f32>), String> {
    // Fetch messaging prices
    let messaging_pricing = twilio_client
        .get_messaging_pricing(credentials, country_code)
        .await
        .map_err(|e| e.to_string())?;

    // Fetch voice prices (use v1 API for standard pricing)
    let voice_pricing = twilio_client
        .get_voice_pricing(credentials, country_code, false)
        .await
        .map_err(|e| e.to_string())?;

    Ok((
        messaging_pricing.outbound_sms_price,
        messaging_pricing.inbound_sms_price,
        voice_pricing.outbound_price_per_min,
        voice_pricing.inbound_price_per_min,
    ))
}

/// Determine the tier for a given country using TwilioClient
async fn check_country_capability_internal(
    twilio_client: &dyn TwilioClient,
    credentials: &TwilioCredentials,
    country_code: &str,
) -> Result<
    (
        CountryTier,
        Option<f32>,
        Option<f32>,
        Option<f32>,
        Option<f32>,
    ),
    String,
> {
    let country_upper = country_code.to_uppercase();

    // Check if US/CA
    if country_upper == "US" || country_upper == "CA" {
        let (outbound_sms, inbound_sms, outbound_voice, inbound_voice) =
            fetch_pricing(twilio_client, credentials, &country_upper).await?;
        return Ok((
            CountryTier::UsCanada,
            outbound_sms,
            inbound_sms,
            outbound_voice,
            inbound_voice,
        ));
    }

    // Check if local numbers are available
    let has_local =
        check_local_numbers_available(twilio_client, credentials, &country_upper).await?;
    let (outbound_sms, inbound_sms, outbound_voice, inbound_voice) =
        fetch_pricing(twilio_client, credentials, &country_upper).await?;

    if has_local {
        return Ok((
            CountryTier::FullService,
            outbound_sms,
            inbound_sms,
            outbound_voice,
            inbound_voice,
        ));
    }

    // Check if country is in the notification-supported whitelist
    if NOTIFICATION_SUPPORTED_COUNTRIES.contains(&country_upper.as_str()) {
        return Ok((
            CountryTier::NotificationOnly,
            outbound_sms,
            inbound_sms,
            outbound_voice,
            inbound_voice,
        ));
    }

    // Not supported at all
    Ok((CountryTier::NotSupported, None, None, None, None))
}

/// Get country capability from cache or fetch from Twilio and cache it
pub async fn get_country_capability(
    state: &Arc<AppState>,
    country_code: &str,
) -> Result<CountryCapabilityInfo, String> {
    let country_upper = country_code.to_uppercase();
    let now = Utc::now().timestamp() as i32;
    let cache_duration = 86400; // 24 hours

    // Try to get from cache first
    let cached: Option<CountryAvailability> = {
        let mut conn = state
            .db_pool
            .get()
            .map_err(|e| format!("DB connection error: {}", e))?;

        country_availability::table
            .filter(country_availability::country_code.eq(&country_upper))
            .filter(country_availability::last_checked.gt(now - cache_duration))
            .select(CountryAvailability::as_select())
            .first::<CountryAvailability>(&mut conn)
            .optional()
            .map_err(|e| format!("DB query error: {}", e))?
    };

    if let Some(cached_data) = cached {
        return Ok(build_capability_info(
            cached_data.has_local_numbers,
            &country_upper,
            cached_data.outbound_sms_price,
            cached_data.inbound_sms_price,
            cached_data.outbound_voice_price_per_min,
            cached_data.inbound_voice_price_per_min,
        ));
    }

    // Not in cache or expired, fetch from Twilio
    let credentials = TwilioCredentials::from_env().map_err(|e| e.to_string())?;

    let (tier, outbound_sms, inbound_sms, outbound_voice, inbound_voice) =
        check_country_capability_internal(
            state.twilio_client.as_ref(),
            &credentials,
            &country_upper,
        )
        .await?;

    if tier == CountryTier::NotSupported {
        return Err("Country not supported for Lightfriend".to_string());
    }

    let has_local = matches!(tier, CountryTier::UsCanada | CountryTier::FullService);

    // Cache the result
    let new_availability = NewCountryAvailability {
        country_code: country_upper.clone(),
        has_local_numbers: has_local,
        outbound_sms_price: outbound_sms,
        inbound_sms_price: inbound_sms,
        outbound_voice_price_per_min: outbound_voice,
        inbound_voice_price_per_min: inbound_voice,
        last_checked: now,
        created_at: now,
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|e| format!("DB connection error: {}", e))?;

    diesel::insert_into(country_availability::table)
        .values(&new_availability)
        .on_conflict(country_availability::country_code)
        .do_update()
        .set((
            country_availability::has_local_numbers.eq(has_local),
            country_availability::outbound_sms_price.eq(outbound_sms),
            country_availability::inbound_sms_price.eq(inbound_sms),
            country_availability::outbound_voice_price_per_min.eq(outbound_voice),
            country_availability::inbound_voice_price_per_min.eq(inbound_voice),
            country_availability::last_checked.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to cache availability: {}", e))?;

    Ok(build_capability_info(
        has_local,
        &country_upper,
        outbound_sms,
        inbound_sms,
        outbound_voice,
        inbound_voice,
    ))
}

fn build_capability_info(
    has_local: bool,
    country_code: &str,
    outbound_sms: Option<f32>,
    inbound_sms: Option<f32>,
    outbound_voice: Option<f32>,
    inbound_voice: Option<f32>,
) -> CountryCapabilityInfo {
    let is_us_ca = country_code == "US" || country_code == "CA";

    let (plan_type, can_receive) = if is_us_ca {
        ("us_ca".to_string(), true)
    } else if has_local {
        ("full_service".to_string(), true)
    } else {
        ("notification_only".to_string(), false)
    };

    CountryCapabilityInfo {
        available: true,
        plan_type,
        can_receive_sms: can_receive,
        outbound_sms_price: outbound_sms,
        inbound_sms_price: inbound_sms,
        outbound_voice_price_per_min: outbound_voice,
        inbound_voice_price_per_min: inbound_voice,
    }
}
