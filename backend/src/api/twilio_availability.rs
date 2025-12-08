use crate::models::user_models::{CountryAvailability, NewCountryAvailability};
use crate::schema::country_availability;
use crate::AppState;
use chrono::Utc;
use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use diesel::SqliteConnection;
use diesel::r2d2::Pool;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountryTier {
    UsCanada,          // Full service, unlimited usage with monitoring
    FullService,       // Local number available in country
    NotificationOnly,  // No local number, but can send from US number
    NotSupported,      // Cannot send messages to this country at all
}

#[derive(Debug, Serialize)]
pub struct CountryCapabilityInfo {
    pub available: bool,
    pub plan_type: String,  // "us_ca", "full_service", "notification_only"
    pub can_receive_sms: bool,
    pub outbound_sms_price: Option<f32>,
    pub inbound_sms_price: Option<f32>,
    pub outbound_voice_price_per_min: Option<f32>,
    pub inbound_voice_price_per_min: Option<f32>,
}

#[derive(Deserialize)]
struct AvailablePhoneNumbersResponse {
    #[serde(default)]
    available_phone_numbers: Vec<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
struct MessagingPricing {
    #[serde(default)]
    inbound_sms_prices: Vec<InboundSmsPrice>,
    #[serde(default)]
    outbound_sms_prices: Vec<OutboundSmsPrice>,
}

#[derive(Deserialize, Debug)]
struct PhoneNumberPricing {
    #[serde(default)]
    phone_number_prices: Vec<PhoneNumberPrice>,
    price_unit: Option<String>,
}

#[derive(Deserialize, Debug)]
struct PhoneNumberPrice {
    number_type: String,
    current_price: String,
}

#[derive(Deserialize, Debug)]
struct InboundSmsPrice {
    number_type: String,
    #[serde(default)]
    prices: Vec<PriceItem>,
}

#[derive(Deserialize, Debug)]
struct OutboundSmsPrice {
    #[serde(default)]
    prices: Vec<PriceItem>,
}

#[derive(Deserialize, Debug)]
struct PriceItem {
    current_price: String,
}

#[derive(Deserialize, Debug)]
struct VoicePricing {
    #[serde(default)]
    inbound_call_prices: Vec<InboundCallPrice>,
    #[serde(default)]
    outbound_prefix_prices: Vec<OutboundPrefixPrice>,
}

#[derive(Deserialize, Debug)]
struct InboundCallPrice {
    current_price: String,
}

#[derive(Deserialize, Debug)]
struct OutboundPrefixPrice {
    current_price: String,
}

/// Response from Twilio Voice Pricing v2 API (origin-based pricing)
#[derive(Deserialize, Debug)]
struct OriginBasedVoicePricing {
    #[serde(default)]
    originating_call_prices: Vec<OriginatingCallPrice>,
    #[serde(default)]
    terminating_prefix_prices: Vec<TerminatingPrefixPrice>,
}

#[derive(Deserialize, Debug)]
struct OriginatingCallPrice {
    base_price: String,
    current_price: String,
}

#[derive(Deserialize, Debug)]
struct TerminatingPrefixPrice {
    base_price: String,
    current_price: String,
}

/// Countries where we can send messages from a US number without issues
/// Based on Twilio's A2P 10DLC coverage and practical experience
const NOTIFICATION_SUPPORTED_COUNTRIES: &[&str] = &[
    // Americas
    "CA", "MX", "BR", "AR", "CL", "CO", "PE", "VE", "EC", "GT", "CU", "BO", "HT", "DO",
    "HN", "PY", "NI", "SV", "CR", "PA", "UY", "PR", "JM", "TT", "GY", "SR", "GF", "BZ",
    "BS", "BB", "GD", "LC", "VC", "AG", "DM", "KN", "AW", "CW", "SX", "BQ", "TC", "VG",
    "KY", "BM", "AI", "MS", "FK", "GS", "PM",
    // Europe
    "GB", "DE", "FR", "IT", "ES", "NL", "BE", "SE", "NO", "DK", "FI", "PL", "GR", "PT",
    "CZ", "RO", "HU", "AT", "CH", "IE", "SK", "BG", "HR", "LT", "SI", "LV", "EE", "CY",
    "LU", "MT", "IS", "AL", "RS", "BA", "MK", "ME", "XK", "MD", "BY", "UA", "GE", "AM",
    "AZ", "LI", "MC", "SM", "VA", "AD", "GI", "JE", "GG", "IM", "FO", "AX",
    // Asia-Pacific
    "AU", "NZ", "JP", "KR", "SG", "HK", "MY", "TH", "PH", "ID", "VN", "TW", "IN", "PK",
    "BD", "LK", "NP", "MM", "KH", "LA", "BN", "MO", "MV", "BT", "TL", "MN", "FJ", "PG",
    "NC", "PF", "GU", "MP", "AS", "WS", "PW", "FM", "MH", "KI", "NR", "TV", "TO", "VU",
    "SB", "CK", "NU", "TK", "WF",
    // Middle East
    "IL", "TR", "SA", "QA", "KW", "BH", "OM", "JO", "LB", "PS", "YE", "IQ", "SY", "CY",
    // Africa
    "ZA", "NG", "EG", "KE", "GH", "TZ", "UG", "ZW", "MU", "RW", "ET", "MA", "TN", "DZ",
    "SN", "CI", "CM", "MG", "BW", "NA", "MW", "ZM", "MZ", "AO", "SD", "SL", "LR", "BJ",
    "TG", "BF", "ML", "NE", "TD", "SS", "CF", "CG", "GA", "GQ", "ST", "CV", "GM", "GW",
    "GN", "BI", "DJ", "ER", "SO", "SC", "KM", "YT", "RE", "MU", "LS", "SZ",
];

/// Check if a country supports local phone numbers by querying Twilio API
async fn check_local_numbers_available(
    country_code: &str,
    account_sid: &str,
    auth_token: &str,
) -> Result<bool, String> {
    let client = reqwest::Client::new();

    // Check both Mobile and Local numbers
    for number_type in &["Mobile", "Local"] {
        let url = format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/AvailablePhoneNumbers/{}/{}.json",
            account_sid,
            country_code.to_uppercase(),
            number_type
        );

        match client
            .get(&url)
            .basic_auth(account_sid, Some(auth_token))
            .query(&[("PageSize", "1")])
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<AvailablePhoneNumbersResponse>().await {
                    Ok(data) => {
                        if !data.available_phone_numbers.is_empty() {
                            return Ok(true);
                        }
                    }
                    Err(_) => continue,
                }
            }
            Ok(resp) if resp.status().as_u16() == 404 => {
                // Country not supported for this number type
                continue;
            }
            _ => continue,
        }
    }

    Ok(false)
}

/// Fetch messaging and voice pricing for a country
async fn fetch_pricing(
    country_code: &str,
    account_sid: &str,
    auth_token: &str,
) -> Result<(Option<f32>, Option<f32>, Option<f32>, Option<f32>), String> {
    let client = reqwest::Client::new();

    // Fetch messaging prices
    let messaging_url = format!(
        "https://pricing.twilio.com/v1/Messaging/Countries/{}",
        country_code.to_uppercase()
    );

    let (outbound_sms_price, inbound_sms_price) = match client
        .get(&messaging_url)
        .basic_auth(account_sid, Some(auth_token))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<MessagingPricing>().await {
                Ok(pricing) => {
                    let outbound = pricing
                        .outbound_sms_prices
                        .first()
                        .and_then(|p| p.prices.first())
                        .and_then(|p| p.current_price.parse::<f32>().ok());

                    let inbound = pricing
                        .inbound_sms_prices
                        .first()
                        .and_then(|p| p.prices.first())
                        .and_then(|p| p.current_price.parse::<f32>().ok());

                    (outbound, inbound)
                }
                Err(_) => (None, None),
            }
        }
        _ => (None, None),
    };

    // Fetch voice prices
    let voice_url = format!(
        "https://pricing.twilio.com/v1/Voice/Countries/{}",
        country_code.to_uppercase()
    );

    let (outbound_voice_price, inbound_voice_price) = match client
        .get(&voice_url)
        .basic_auth(account_sid, Some(auth_token))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<VoicePricing>().await {
                Ok(pricing) => {
                    let outbound = pricing
                        .outbound_prefix_prices
                        .first()
                        .and_then(|p| p.current_price.parse::<f32>().ok());

                    let inbound = pricing
                        .inbound_call_prices
                        .first()
                        .and_then(|p| p.current_price.parse::<f32>().ok());

                    (outbound, inbound)
                }
                Err(_) => (None, None),
            }
        }
        _ => (None, None),
    };

    Ok((outbound_sms_price, inbound_sms_price, outbound_voice_price, inbound_voice_price))
}

/// Fetch domestic (same-country) voice pricing using Twilio v2 API
/// This is for BYOT users who call from their country's number to the same country
/// Returns (outbound_per_min, inbound_per_min)
async fn fetch_domestic_voice_pricing(
    country_code: &str,
    account_sid: &str,
    auth_token: &str,
) -> Result<(Option<f32>, Option<f32>), String> {
    let client = reqwest::Client::new();
    let country_upper = country_code.to_uppercase();

    // Twilio v2 Voice Pricing API with origination country
    // https://pricing.twilio.com/v2/Voice/Countries/{country}?OriginationNumber=+{country_prefix}
    // But simpler: /v2/Voice/Numbers/{number} gives us origin-based pricing
    // For now, use the country endpoint which gives us domestic rates
    let url = format!(
        "https://pricing.twilio.com/v2/Voice/Countries/{}",
        country_upper
    );

    match client
        .get(&url)
        .basic_auth(account_sid, Some(auth_token))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<OriginBasedVoicePricing>().await {
                Ok(pricing) => {
                    // Outbound: terminating_prefix_prices (calling TO the country FROM the country)
                    let outbound = pricing
                        .terminating_prefix_prices
                        .first()
                        .and_then(|p| p.current_price.parse::<f32>().ok());

                    // Inbound: originating_call_prices (receiving calls on a number IN the country)
                    let inbound = pricing
                        .originating_call_prices
                        .first()
                        .and_then(|p| p.current_price.parse::<f32>().ok());

                    Ok((outbound, inbound))
                }
                Err(e) => {
                    tracing::warn!("Failed to parse v2 voice pricing for {}: {}", country_upper, e);
                    Ok((None, None))
                }
            }
        }
        Ok(resp) => {
            tracing::warn!("v2 voice pricing API returned {} for {}", resp.status(), country_upper);
            Ok((None, None))
        }
        Err(e) => {
            tracing::warn!("Failed to fetch v2 voice pricing for {}: {}", country_upper, e);
            Ok((None, None))
        }
    }
}

/// Fetch phone number monthly pricing for a country
/// Returns (local_price, mobile_price) - both are monthly costs
pub async fn fetch_phone_number_pricing(
    country_code: &str,
    account_sid: &str,
    auth_token: &str,
) -> Result<(Option<f32>, Option<f32>), String> {
    let client = reqwest::Client::new();

    let url = format!(
        "https://pricing.twilio.com/v1/PhoneNumbers/Countries/{}",
        country_code.to_uppercase()
    );

    match client
        .get(&url)
        .basic_auth(account_sid, Some(auth_token))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<PhoneNumberPricing>().await {
                Ok(pricing) => {
                    let mut local_price = None;
                    let mut mobile_price = None;

                    for price in pricing.phone_number_prices {
                        let p = price.current_price.parse::<f32>().ok();
                        match price.number_type.to_lowercase().as_str() {
                            "local" => local_price = p,
                            "mobile" => mobile_price = p,
                            _ => {}
                        }
                    }

                    Ok((local_price, mobile_price))
                }
                Err(_) => Ok((None, None)),
            }
        }
        _ => Ok((None, None)),
    }
}

/// Full BYOT pricing info for a country
#[derive(Debug, Clone, Serialize)]
pub struct ByotCountryPricing {
    pub country_code: String,
    pub has_local_numbers: bool,
    /// Monthly cost for local phone number
    pub local_number_monthly: Option<f32>,
    /// Monthly cost for mobile phone number
    pub mobile_number_monthly: Option<f32>,
    /// Raw SMS price per segment (outbound)
    pub sms_price_per_segment: Option<f32>,
    /// Voice price per minute (outbound)
    pub voice_price_per_minute: Option<f32>,
    /// Inbound SMS price (if local number)
    pub inbound_sms_price: Option<f32>,
    /// Inbound voice price per minute (if local number)
    pub inbound_voice_price_per_minute: Option<f32>,
}

/// Get full BYOT pricing for a country (fetches from Twilio or cache)
/// BYOT = Bring Your Own Twilio - users buy numbers in their own country
/// Voice pricing uses DOMESTIC rates (country→country) since users call from their own country's number
pub async fn get_byot_pricing(
    state: &Arc<AppState>,
    country_code: &str,
) -> Result<ByotCountryPricing, String> {
    let country_upper = country_code.to_uppercase();

    // Get capability info (this handles caching for SMS pricing)
    let capability = get_country_capability(state, &country_upper).await?;

    let account_sid = std::env::var("TWILIO_ACCOUNT_SID")
        .map_err(|_| "TWILIO_ACCOUNT_SID not set".to_string())?;
    let auth_token = std::env::var("TWILIO_AUTH_TOKEN")
        .map_err(|_| "TWILIO_AUTH_TOKEN not set".to_string())?;

    // Fetch phone number pricing
    let (local_price, mobile_price) = fetch_phone_number_pricing(&country_upper, &account_sid, &auth_token).await?;

    // Fetch DOMESTIC voice pricing using v2 API
    // This is crucial for BYOT - users call FROM their country TO their country
    let (domestic_outbound_voice, domestic_inbound_voice) =
        fetch_domestic_voice_pricing(&country_upper, &account_sid, &auth_token).await?;

    Ok(ByotCountryPricing {
        country_code: country_upper,
        has_local_numbers: capability.can_receive_sms,
        local_number_monthly: local_price,
        mobile_number_monthly: mobile_price,
        sms_price_per_segment: capability.outbound_sms_price,
        // Use domestic voice rates for BYOT (country→country calls)
        voice_price_per_minute: domestic_outbound_voice.or(capability.outbound_voice_price_per_min),
        inbound_sms_price: capability.inbound_sms_price,
        // Use domestic inbound voice rates for BYOT
        inbound_voice_price_per_minute: domestic_inbound_voice.or(capability.inbound_voice_price_per_min),
    })
}

/// Determine the tier for a given country
pub async fn check_country_capability(
    country_code: &str,
    account_sid: &str,
    auth_token: &str,
) -> Result<(CountryTier, Option<f32>, Option<f32>, Option<f32>, Option<f32>), String> {
    let country_upper = country_code.to_uppercase();

    // Check if US/CA
    if country_upper == "US" || country_upper == "CA" {
        let (outbound_sms, inbound_sms, outbound_voice, inbound_voice) =
            fetch_pricing(&country_upper, account_sid, auth_token).await?;
        return Ok((CountryTier::UsCanada, outbound_sms, inbound_sms, outbound_voice, inbound_voice));
    }

    // Check if local numbers are available
    let has_local = check_local_numbers_available(&country_upper, account_sid, auth_token).await?;
    let (outbound_sms, inbound_sms, outbound_voice, inbound_voice) =
        fetch_pricing(&country_upper, account_sid, auth_token).await?;

    if has_local {
        return Ok((CountryTier::FullService, outbound_sms, inbound_sms, outbound_voice, inbound_voice));
    }

    // Check if country is in the notification-supported whitelist
    if NOTIFICATION_SUPPORTED_COUNTRIES.contains(&country_upper.as_str()) {
        return Ok((CountryTier::NotificationOnly, outbound_sms, inbound_sms, outbound_voice, inbound_voice));
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
        let mut conn = state.db_pool.get().map_err(|e| format!("DB connection error: {}", e))?;

        country_availability::table
            .filter(country_availability::country_code.eq(&country_upper))
            .filter(country_availability::last_checked.gt(now - cache_duration))
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
    let account_sid = std::env::var("TWILIO_ACCOUNT_SID")
        .map_err(|_| "TWILIO_ACCOUNT_SID not set".to_string())?;
    let auth_token = std::env::var("TWILIO_AUTH_TOKEN")
        .map_err(|_| "TWILIO_AUTH_TOKEN not set".to_string())?;

    let (tier, outbound_sms, inbound_sms, outbound_voice, inbound_voice) =
        check_country_capability(&country_upper, &account_sid, &auth_token).await?;

    if tier == CountryTier::NotSupported {
        return Err("Country not supported for Lightfriend tier 3".to_string());
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

    let mut conn = state.db_pool.get().map_err(|e| format!("DB connection error: {}", e))?;

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

/// Refresh all cached country availability data (for cron job)
pub async fn refresh_all_country_availability(
    db_pool: &Pool<ConnectionManager<SqliteConnection>>,
) -> Result<(), String> {
    let account_sid = std::env::var("TWILIO_ACCOUNT_SID")
        .map_err(|_| "TWILIO_ACCOUNT_SID not set".to_string())?;
    let auth_token = std::env::var("TWILIO_AUTH_TOKEN")
        .map_err(|_| "TWILIO_AUTH_TOKEN not set".to_string())?;

    // Get all cached countries
    let countries: Vec<String> = {
        let mut conn = db_pool.get().map_err(|e| format!("DB connection error: {}", e))?;
        country_availability::table
            .select(country_availability::country_code)
            .load::<String>(&mut conn)
            .map_err(|e| format!("Failed to load countries: {}", e))?
    };

    let now = Utc::now().timestamp() as i32;

    for country in countries {
        let (tier, outbound_sms, inbound_sms, outbound_voice, inbound_voice) =
            match check_country_capability(&country, &account_sid, &auth_token).await {
                Ok(result) => result,
                Err(e) => {
                    eprintln!("Error checking {}: {}", country, e);
                    continue;
                }
            };

        let has_local = matches!(tier, CountryTier::UsCanada | CountryTier::FullService);

        let mut conn = db_pool.get().map_err(|e| format!("DB connection error: {}", e))?;

        diesel::update(country_availability::table)
            .filter(country_availability::country_code.eq(&country))
            .set((
                country_availability::has_local_numbers.eq(has_local),
                country_availability::outbound_sms_price.eq(outbound_sms),
                country_availability::inbound_sms_price.eq(inbound_sms),
                country_availability::outbound_voice_price_per_min.eq(outbound_voice),
                country_availability::inbound_voice_price_per_min.eq(inbound_voice),
                country_availability::last_checked.eq(now),
            ))
            .execute(&mut conn)
            .map_err(|e| format!("Failed to update {}: {}", country, e))?;
    }

    Ok(())
}
