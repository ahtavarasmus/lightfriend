use crate::api::twilio_availability::get_country_capability;
use crate::api::twilio_pricing::{get_euro_country_pricing, get_notification_only_pricing};
use crate::handlers::auth_middleware::AuthUser;
use crate::schema::usage_logs;
use crate::utils::country::is_notification_only_country_code;
use crate::UserCoreOps;

/// ALL countries supported worldwide via Twilio
/// Comprehensive list for pricing display - pricing fetched on-demand when selected
const ALL_NOTIFICATION_COUNTRIES: &[&str] = &[
    // Europe
    "AL", "AD", "AT", "BY", "BE", "BA", "BG", "HR", "CY", "CZ", "DK", "EE", "FO", "FI", "FR", "DE",
    "GI", "GR", "HU", "IS", "IE", "IT", "XK", "LV", "LI", "LT", "LU", "MT", "MD", "MC", "ME", "NL",
    "MK", "NO", "PL", "PT", "RO", "RU", "SM", "RS", "SK", "SI", "ES", "SE", "CH", "UA", "GB", "VA",
    // Asia
    "AF", "AM", "AZ", "BH", "BD", "BT", "BN", "KH", "CN", "GE", "HK", "IN", "ID", "IR", "IQ", "IL",
    "JP", "JO", "KZ", "KW", "KG", "LA", "LB", "MO", "MY", "MV", "MN", "MM", "NP", "OM", "PK", "PS",
    "PH", "QA", "SA", "SG", "KR", "LK", "SY", "TW", "TJ", "TH", "TL", "TR", "TM", "AE", "UZ", "VN",
    "YE", // Africa
    "DZ", "AO", "BJ", "BW", "BF", "BI", "CV", "CM", "CF", "TD", "KM", "CD", "DJ", "EG", "GQ", "ER",
    "SZ", "ET", "GA", "GM", "GH", "GN", "GW", "CI", "KE", "LS", "LR", "LY", "MG", "MW", "ML", "MR",
    "MU", "MA", "MZ", "NA", "NE", "NG", "CG", "RW", "ST", "SN", "SC", "SL", "SO", "ZA", "SS", "SD",
    "TZ", "TG", "TN", "UG", "ZM", "ZW",
    // North America (excluding US/CA which are local-number)
    "AG", "BS", "BB", "BZ", "CR", "CU", "DM", "DO", "SV", "GD", "GT", "HT", "HN", "JM", "MX", "NI",
    "PA", "KN", "LC", "VC", "TT", // South America
    "AR", "BO", "BR", "CL", "CO", "EC", "GY", "PY", "PE", "SR", "UY", "VE",
    // Oceania (excluding AU which is local-number)
    "FJ", "KI", "MH", "FM", "NR", "NZ", "PW", "PG", "WS", "SB", "TO", "TV", "VU",
];
use crate::AppState;
use axum::{extract::State, Json};
use diesel::dsl::sql;
use diesel::prelude::*;
use diesel::sql_types::BigInt;
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
pub struct CountryPricing {
    pub country_code: String,
    pub country_name: String,
    pub sms_price: f32,   // Final price after formula
    pub voice_price: f32, // Final price per minute
}

/// Simple country info for listing (no pricing)
#[derive(Serialize)]
pub struct CountryInfo {
    pub country_code: String,
    pub country_name: String,
    pub is_local_number: bool,
}

/// Response for all countries list endpoint
#[derive(Serialize)]
pub struct AllCountriesResponse {
    pub local_number_countries: Vec<CountryInfo>,
    pub notification_only_countries: Vec<CountryInfo>,
}

/// GET /api/pricing/all-countries
/// Returns list of ALL supported countries (no pricing - use /api/pricing/country/{code} for pricing)
pub async fn get_all_countries() -> Json<AllCountriesResponse> {
    // Local-number countries (US, CA, FI, NL, GB, AU)
    let local_number_countries: Vec<CountryInfo> = ["US", "CA", "FI", "NL", "GB", "AU"]
        .iter()
        .map(|&code| CountryInfo {
            country_code: code.to_string(),
            country_name: get_country_name(code),
            is_local_number: true,
        })
        .collect();

    // All notification-only countries (worldwide)
    let mut notification_only_countries: Vec<CountryInfo> = ALL_NOTIFICATION_COUNTRIES
        .iter()
        .filter(|&&code| !["US", "CA", "FI", "NL", "GB", "AU"].contains(&code))
        .map(|&code| CountryInfo {
            country_code: code.to_string(),
            country_name: get_country_name(code),
            is_local_number: false,
        })
        .collect();

    // Sort by country name
    notification_only_countries.sort_by(|a, b| a.country_name.cmp(&b.country_name));

    Json(AllCountriesResponse {
        local_number_countries,
        notification_only_countries,
    })
}

/// GET /api/pricing/country/{country_code}
/// Returns pricing for a specific country (on-demand pricing fetch)
pub async fn get_single_country_pricing(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(country_code): axum::extract::Path<String>,
) -> Result<Json<CountryPricing>, (axum::http::StatusCode, String)> {
    let code = country_code.to_uppercase();

    // Check if it's a local-number country or notification-only
    let is_local = ["US", "CA", "FI", "NL", "GB", "AU"].contains(&code.as_str());

    let pricing = if is_local && !["US", "CA"].contains(&code.as_str()) {
        // Euro local-number countries
        get_euro_country_pricing(&state, &code)
            .await
            .map_err(|e| (axum::http::StatusCode::NOT_FOUND, e))?
    } else if ["US", "CA"].contains(&code.as_str()) {
        // US/CA have fixed pricing
        return Ok(Json(CountryPricing {
            country_code: code.clone(),
            country_name: get_country_name(&code),
            sms_price: 0.075,   // Fixed US/CA SMS price
            voice_price: 0.185, // Twilio $0.075 + ElevenLabs $0.11
        }));
    } else {
        // Notification-only countries
        get_notification_only_pricing(&state, &code)
            .await
            .map_err(|e| (axum::http::StatusCode::NOT_FOUND, e))?
    };

    Ok(Json(CountryPricing {
        country_code: code.clone(),
        country_name: get_country_name(&code),
        sms_price: pricing.calculated_sms_price,
        voice_price: pricing.calculated_voice_price,
    }))
}

fn get_country_name(code: &str) -> String {
    match code {
        // Local-number countries (US, CA, FI, NL, GB, AU)
        "US" => "United States",
        "CA" => "Canada",
        "FI" => "Finland",
        "NL" => "Netherlands",
        "GB" | "UK" => "United Kingdom",
        "AU" => "Australia",
        // Europe
        "AL" => "Albania",
        "AD" => "Andorra",
        "AT" => "Austria",
        "BY" => "Belarus",
        "BE" => "Belgium",
        "BA" => "Bosnia and Herzegovina",
        "BG" => "Bulgaria",
        "HR" => "Croatia",
        "CY" => "Cyprus",
        "CZ" => "Czech Republic",
        "DK" => "Denmark",
        "EE" => "Estonia",
        "FO" => "Faroe Islands",
        "FR" => "France",
        "DE" => "Germany",
        "GI" => "Gibraltar",
        "GR" => "Greece",
        "HU" => "Hungary",
        "IS" => "Iceland",
        "IE" => "Ireland",
        "IT" => "Italy",
        "XK" => "Kosovo",
        "LV" => "Latvia",
        "LI" => "Liechtenstein",
        "LT" => "Lithuania",
        "LU" => "Luxembourg",
        "MT" => "Malta",
        "MD" => "Moldova",
        "MC" => "Monaco",
        "ME" => "Montenegro",
        "MK" => "North Macedonia",
        "NO" => "Norway",
        "PL" => "Poland",
        "PT" => "Portugal",
        "RO" => "Romania",
        "RU" => "Russia",
        "SM" => "San Marino",
        "RS" => "Serbia",
        "SK" => "Slovakia",
        "SI" => "Slovenia",
        "ES" => "Spain",
        "SE" => "Sweden",
        "CH" => "Switzerland",
        "UA" => "Ukraine",
        "VA" => "Vatican City",
        // Asia
        "AF" => "Afghanistan",
        "AM" => "Armenia",
        "AZ" => "Azerbaijan",
        "BH" => "Bahrain",
        "BD" => "Bangladesh",
        "BT" => "Bhutan",
        "BN" => "Brunei",
        "KH" => "Cambodia",
        "CN" => "China",
        "GE" => "Georgia",
        "HK" => "Hong Kong",
        "IN" => "India",
        "ID" => "Indonesia",
        "IR" => "Iran",
        "IQ" => "Iraq",
        "IL" => "Israel",
        "JP" => "Japan",
        "JO" => "Jordan",
        "KZ" => "Kazakhstan",
        "KW" => "Kuwait",
        "KG" => "Kyrgyzstan",
        "LA" => "Laos",
        "LB" => "Lebanon",
        "MO" => "Macao",
        "MY" => "Malaysia",
        "MV" => "Maldives",
        "MN" => "Mongolia",
        "MM" => "Myanmar",
        "NP" => "Nepal",
        "OM" => "Oman",
        "PK" => "Pakistan",
        "PS" => "Palestine",
        "PH" => "Philippines",
        "QA" => "Qatar",
        "SA" => "Saudi Arabia",
        "SG" => "Singapore",
        "KR" => "South Korea",
        "LK" => "Sri Lanka",
        "SY" => "Syria",
        "TW" => "Taiwan",
        "TJ" => "Tajikistan",
        "TH" => "Thailand",
        "TL" => "Timor-Leste",
        "TR" => "Turkey",
        "TM" => "Turkmenistan",
        "AE" => "United Arab Emirates",
        "UZ" => "Uzbekistan",
        "VN" => "Vietnam",
        "YE" => "Yemen",
        // Africa
        "DZ" => "Algeria",
        "AO" => "Angola",
        "BJ" => "Benin",
        "BW" => "Botswana",
        "BF" => "Burkina Faso",
        "BI" => "Burundi",
        "CV" => "Cape Verde",
        "CM" => "Cameroon",
        "CF" => "Central African Republic",
        "TD" => "Chad",
        "KM" => "Comoros",
        "CD" => "DR Congo",
        "DJ" => "Djibouti",
        "EG" => "Egypt",
        "GQ" => "Equatorial Guinea",
        "ER" => "Eritrea",
        "SZ" => "Eswatini",
        "ET" => "Ethiopia",
        "GA" => "Gabon",
        "GM" => "Gambia",
        "GH" => "Ghana",
        "GN" => "Guinea",
        "GW" => "Guinea-Bissau",
        "CI" => "Ivory Coast",
        "KE" => "Kenya",
        "LS" => "Lesotho",
        "LR" => "Liberia",
        "LY" => "Libya",
        "MG" => "Madagascar",
        "MW" => "Malawi",
        "ML" => "Mali",
        "MR" => "Mauritania",
        "MU" => "Mauritius",
        "MA" => "Morocco",
        "MZ" => "Mozambique",
        "NA" => "Namibia",
        "NE" => "Niger",
        "NG" => "Nigeria",
        "CG" => "Republic of the Congo",
        "RW" => "Rwanda",
        "ST" => "Sao Tome and Principe",
        "SN" => "Senegal",
        "SC" => "Seychelles",
        "SL" => "Sierra Leone",
        "SO" => "Somalia",
        "ZA" => "South Africa",
        "SS" => "South Sudan",
        "SD" => "Sudan",
        "TZ" => "Tanzania",
        "TG" => "Togo",
        "TN" => "Tunisia",
        "UG" => "Uganda",
        "ZM" => "Zambia",
        "ZW" => "Zimbabwe",
        // North America and Caribbean
        "AG" => "Antigua and Barbuda",
        "BS" => "Bahamas",
        "BB" => "Barbados",
        "BZ" => "Belize",
        "CR" => "Costa Rica",
        "CU" => "Cuba",
        "DM" => "Dominica",
        "DO" => "Dominican Republic",
        "SV" => "El Salvador",
        "GD" => "Grenada",
        "GT" => "Guatemala",
        "HT" => "Haiti",
        "HN" => "Honduras",
        "JM" => "Jamaica",
        "MX" => "Mexico",
        "NI" => "Nicaragua",
        "PA" => "Panama",
        "KN" => "Saint Kitts and Nevis",
        "LC" => "Saint Lucia",
        "VC" => "Saint Vincent and the Grenadines",
        "TT" => "Trinidad and Tobago",
        // South America
        "AR" => "Argentina",
        "BO" => "Bolivia",
        "BR" => "Brazil",
        "CL" => "Chile",
        "CO" => "Colombia",
        "EC" => "Ecuador",
        "GY" => "Guyana",
        "PY" => "Paraguay",
        "PE" => "Peru",
        "SR" => "Suriname",
        "UY" => "Uruguay",
        "VE" => "Venezuela",
        // Oceania
        "FJ" => "Fiji",
        "KI" => "Kiribati",
        "MH" => "Marshall Islands",
        "FM" => "Micronesia",
        "NR" => "Nauru",
        "NZ" => "New Zealand",
        "PW" => "Palau",
        "PG" => "Papua New Guinea",
        "WS" => "Samoa",
        "SB" => "Solomon Islands",
        "TO" => "Tonga",
        "TV" => "Tuvalu",
        "VU" => "Vanuatu",
        _ => code,
    }
    .to_string()
}

/// Response for BYOT pricing endpoint (simplified - no monthly number cost)
#[derive(Serialize)]
pub struct ByotPricingResponse {
    pub country_code: String,
    pub country_name: String,
    pub has_local_numbers: bool,
    /// Monthly cost for a phone number - always None (users check Twilio directly)
    pub monthly_number_cost: Option<f32>,
    /// Cost breakdown for different message types
    pub costs: ByotMessageCosts,
}

#[derive(Serialize)]
pub struct ByotMessageCosts {
    /// SMS price per segment (raw Twilio price)
    pub sms_per_segment: Option<f32>,
    /// Notification cost (1.5 segments)
    pub notification: Option<f32>,
    /// Normal response cost (~3 segments)
    pub normal_response: Option<f32>,
    /// Digest cost (~3 segments)
    pub digest: Option<f32>,
    /// Voice outbound per minute
    pub voice_outbound_per_min: Option<f32>,
    /// Voice inbound per minute (if local number available)
    pub voice_inbound_per_min: Option<f32>,
}

/// GET /api/pricing/byot/{country_code}
/// Returns pricing for a specific country (for frontend message equivalent calculations)
/// Note: monthly_number_cost is always None - BYOT users should check Twilio directly for number prices
pub async fn get_byot_country_pricing(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(country_code): axum::extract::Path<String>,
) -> Result<Json<ByotPricingResponse>, (axum::http::StatusCode, String)> {
    let capability = get_country_capability(&state, &country_code)
        .await
        .map_err(|e| (axum::http::StatusCode::NOT_FOUND, e))?;

    let sms_price = capability.outbound_sms_price;

    // ElevenLabs voice AI cost: $0.11 per minute (added to all voice calls)
    const ELEVENLABS_COST_PER_MIN: f32 = 0.11;

    Ok(Json(ByotPricingResponse {
        country_code: country_code.to_uppercase(),
        country_name: get_country_name(&country_code.to_uppercase()),
        has_local_numbers: capability.can_receive_sms,
        monthly_number_cost: None, // BYOT users should check Twilio website directly
        costs: ByotMessageCosts {
            sms_per_segment: sms_price,
            notification: sms_price.map(|p| p * 1.5),
            normal_response: sms_price.map(|p| p * 3.0),
            digest: sms_price.map(|p| p * 3.0),
            // Add ElevenLabs cost to voice (AI voice generation)
            voice_outbound_per_min: capability
                .outbound_voice_price_per_min
                .map(|p| p + ELEVENLABS_COST_PER_MIN),
            voice_inbound_per_min: capability
                .inbound_voice_price_per_min
                .map(|p| p + ELEVENLABS_COST_PER_MIN),
        },
    }))
}

/// Dashboard credits response - shows user's credits with equivalents
#[derive(Serialize)]
pub struct DashboardCreditsResponse {
    /// User's country code (from phone_number_country)
    pub country_code: String,
    /// Whether this is a US/CA user (message-based credits)
    pub is_us_ca: bool,
    /// Whether this country is notification-only (no responses)
    pub is_notification_only: bool,
    /// Whether this country has local numbers (can receive inbound calls)
    pub has_local_numbers: bool,
    /// User's plan type: "monitor" or "digest"
    pub plan_type: Option<String>,
    /// Monthly credits info (if user has subscription)
    pub monthly: Option<CreditEquivalents>,
    /// Overage credits info (if user has overage credits)
    pub overage: Option<CreditEquivalents>,
    /// Days until billing resets
    pub days_until_billing: Option<i32>,
}

#[derive(Serialize)]
pub struct CreditEquivalents {
    /// Raw credit value (message count for US/CA, € for euro)
    pub raw_value: f32,
    /// Display string for the value (e.g., "15.50€" or "200 messages")
    pub display_value: String,
    /// Approximate notifications possible
    pub notifications: i32,
    /// Approximate digests possible
    pub digests: i32,
    /// Approximate responses possible (None for notification-only countries)
    pub responses: Option<i32>,
    /// Approximate voice minutes outbound
    pub voice_mins_out: Option<i32>,
    /// Approximate voice minutes inbound (if local number available)
    pub voice_mins_in: Option<i32>,
}

/// Check if country is US or CA
fn is_us_or_ca(country: &str) -> bool {
    matches!(country, "US" | "CA")
}

/// Check if country code is notification-only (all non-local countries)
fn is_notification_only_code(country: &str) -> bool {
    is_notification_only_country_code(country)
}

/// Check if country code has local numbers (FI, NL, GB, AU, US, CA)
fn has_local_numbers_code(country: &str) -> bool {
    matches!(country, "US" | "CA" | "FI" | "NL" | "GB" | "AU")
}

/// GET /api/pricing/dashboard-credits
/// Returns authenticated user's credits with message equivalents
pub async fn get_dashboard_credits(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<DashboardCreditsResponse>, (axum::http::StatusCode, String)> {
    // Get user from database
    let user = state
        .user_core
        .find_by_id(auth_user.user_id)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "User not found".to_string(),
        ))?;

    // Determine country
    let country_code = crate::utils::country::get_country_code_from_phone(&user.phone_number)
        .unwrap_or_else(|| "US".to_string());
    let is_us_ca = is_us_or_ca(&country_code);
    let is_notification_only = is_notification_only_code(&country_code);
    let has_local_numbers = has_local_numbers_code(&country_code);

    // Get pricing for this country using get_country_capability
    let capability = if !is_us_ca {
        get_country_capability(&state, &country_code).await.ok()
    } else {
        None
    };

    // ElevenLabs voice AI cost
    const ELEVENLABS_COST_PER_MIN: f32 = 0.11;

    // Calculate equivalents helper
    let calculate_equivalents = |credit_value: f32, is_euro: bool| -> CreditEquivalents {
        if is_us_ca {
            // US/CA: credit_value is message count, each message = 1 notification or 0.5 response
            let notifications = (credit_value * 2.0).floor() as i32; // 2 notifications per message
            let responses = (credit_value).floor() as i32; // 1 response per message
            let digests = (credit_value).floor() as i32; // 1 digest per message
                                                         // Voice: ~$0.185 per min total (Twilio $0.075 + ElevenLabs $0.11)
                                                         // Each message credit is worth ~$0.075, so voice_mins = (credits * $0.075) / $0.185
            let voice_mins = (credit_value * 0.075 / 0.185).floor() as i32;

            CreditEquivalents {
                raw_value: credit_value,
                display_value: format!("{} messages", credit_value as i32),
                notifications,
                digests,
                responses: Some(responses),
                voice_mins_out: Some(voice_mins),
                voice_mins_in: if has_local_numbers {
                    Some(voice_mins)
                } else {
                    None
                },
            }
        } else if let Some(ref cap) = capability {
            // Euro: credit_value is € amount
            let sms_price = cap.outbound_sms_price.unwrap_or(0.10);

            // Notification = 1.5 segments
            let notifications = (credit_value / (1.5 * sms_price)).floor() as i32;
            // Response = 3 segments
            let responses = (credit_value / (3.0 * sms_price)).floor() as i32;
            // Digest = 3 segments
            let digests = (credit_value / (3.0 * sms_price)).floor() as i32;
            // Voice outbound
            let voice_out = cap
                .outbound_voice_price_per_min
                .map(|v| (credit_value / (v + ELEVENLABS_COST_PER_MIN)).floor() as i32);
            // Voice inbound (only for local number countries)
            let voice_in = if has_local_numbers {
                cap.inbound_voice_price_per_min
                    .map(|v| (credit_value / (v + ELEVENLABS_COST_PER_MIN)).floor() as i32)
            } else {
                None
            };

            CreditEquivalents {
                raw_value: credit_value,
                display_value: format!("{:.2}€", credit_value),
                notifications,
                digests,
                responses: if is_notification_only {
                    None
                } else {
                    Some(responses)
                },
                voice_mins_out: voice_out,
                voice_mins_in: voice_in,
            }
        } else {
            // Fallback if no pricing available
            CreditEquivalents {
                raw_value: credit_value,
                display_value: if is_euro {
                    format!("{:.2}€", credit_value)
                } else {
                    format!("{} messages", credit_value as i32)
                },
                notifications: 0,
                digests: 0,
                responses: None,
                voice_mins_out: None,
                voice_mins_in: None,
            }
        }
    };

    // Monthly credits (credits_left)
    let monthly = if user.credits_left > 0.0 || user.sub_tier.is_some() {
        Some(calculate_equivalents(user.credits_left, !is_us_ca))
    } else {
        None
    };

    // Overage credits (only for digest plan users or anyone with overage > 0)
    let show_overage = user.plan_type.as_deref() == Some("digest") || user.credits > 0.0;
    let overage = if show_overage {
        Some(calculate_equivalents(user.credits, true)) // Overage is always in €
    } else {
        None
    };

    // Calculate days until billing
    let days_until_billing = if let Some(next_billing) = user.next_billing_date_timestamp {
        let now = chrono::Utc::now().timestamp() as i32;
        let days = (next_billing - now) / 86400;
        Some(days.max(0))
    } else {
        None
    };

    Ok(Json(DashboardCreditsResponse {
        country_code,
        is_us_ca,
        is_notification_only,
        has_local_numbers,
        plan_type: user.plan_type,
        monthly,
        overage,
        days_until_billing,
    }))
}

// ============================================================================
// Usage Projection Endpoint - Shows usage in notification units, not euros
// ============================================================================

/// Response for usage projection - all values in NOTIFICATION UNITS (not currency)
#[derive(Serialize)]
pub struct UsageProjectionResponse {
    /// User's plan type: "monitor" or "digest"
    pub plan_type: Option<String>,
    /// Plan capacity in notifications per month (30 for monitor, 120 for digest)
    pub plan_capacity: i32,
    /// Whether auto top-up is enabled
    pub has_auto_topup: bool,
    /// Days until billing cycle resets
    pub days_until_billing: Option<i32>,
    /// True if using example data (< 3 days of usage history)
    pub is_example_data: bool,

    // === NEW SIMPLIFIED STATUS FIELDS ===
    /// Simple status: "on_track", "warning", "over_quota"
    pub status: String,
    /// Display string for percentage: "65% of monthly quota used"
    pub usage_percentage_display: String,
    /// Days overage credits will last at current usage (if no auto top-up and has credits)
    pub overage_days_remaining: Option<i32>,
    /// Estimated extra cost per month if over quota (if auto top-up enabled)
    pub estimated_monthly_extra_cost: Option<f32>,
    /// Recommendation for user action (if warning or over)
    pub recommendation: Option<UsageRecommendation>,
    /// User's current overage credits balance
    pub overage_credits: f32,

    // Digest usage
    /// Number of active digests per day (0-3)
    pub digest_count: i32,
    /// Digests per month (digest_count * 30)
    pub digests_per_month: i32,

    // Detailed breakdown (all averages from last 30 days)
    /// SMS notifications per day (_critical, _priority_sms)
    pub avg_sms_notifications_per_day: f32,
    /// Call notifications per day (_priority_call, noti_call) - more expensive
    pub avg_call_notifications_per_day: f32,
    /// Regular SMS messages per day (activity_type = "sms")
    pub avg_messages_per_day: f32,
    /// Voice call minutes per day
    pub avg_voice_mins_per_day: f32,

    // Combined for simple display
    /// Average notifications per day (sms + call combined)
    pub avg_notifications_per_day: f32,
    /// Projected notifications per month (avg * 30)
    pub notifications_per_month: i32,

    // Totals
    /// Total projected usage per month (digests + notifications + messages)
    pub total_usage_per_month: i32,
    /// Usage as percentage of plan capacity
    pub usage_percentage: f32,
    /// Remaining capacity (can be negative if over)
    pub remaining_capacity: i32,

    // Overage info (only if usage > capacity)
    pub overage: Option<OverageInfo>,

    // === SEGMENTED BAR FIELDS ===
    /// Whether this is a notification-only country (no messages/responses)
    pub is_notification_only: bool,
    /// Digests as percentage of plan capacity
    pub digest_percentage: f32,
    /// SMS notifications as percentage of plan capacity
    pub sms_noti_percentage: f32,
    /// Call notifications as percentage of plan capacity
    pub call_noti_percentage: f32,
    /// Messages as percentage of plan capacity
    pub messages_percentage: f32,
    /// Voice as percentage of plan capacity
    pub voice_percentage: f32,

    // === ACTUAL USAGE THIS BILLING PERIOD ===
    /// Actual notifications used this billing period (not projected)
    pub actual_notifications_used: i32,
    /// Actual voice minutes used this billing period
    pub actual_voice_mins_used: i32,
    /// Actual messages sent this billing period
    pub actual_messages_used: i32,
    /// Actual digests sent this billing period
    pub actual_digests_used: i32,
}

/// Overage information - this is where we show euro amounts
#[derive(Serialize)]
pub struct OverageInfo {
    /// How many notifications over the plan limit
    pub notifications_over: i32,
    /// Estimated euro cost for the overage
    pub estimated_cost_euros: f32,
    /// Whether auto top-up will cover it
    pub covered_by_auto_topup: bool,
}

/// Recommendation for what the user should do
#[derive(Serialize)]
pub struct UsageRecommendation {
    /// The recommendation message
    pub message: String,
    /// Type of action: "reduce_digests", "upgrade_plan", "enable_topup", "buy_credits"
    pub action_type: String,
    /// Optional link to the relevant section
    pub action_link: Option<String>,
}

/// GET /api/pricing/usage-projection
/// Returns user's projected usage in notification units (not euros)
/// Euro amounts only shown for overage
pub async fn get_usage_projection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<UsageProjectionResponse>, (axum::http::StatusCode, String)> {
    use diesel::sql_types::Float;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Get user from database
    let user = state
        .user_core
        .find_by_id(auth_user.user_id)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "User not found".to_string(),
        ))?;

    // Get user settings for digest info
    let (morning_digest, day_digest, evening_digest) = state
        .user_core
        .get_digests(auth_user.user_id)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Count active digests
    let digest_count = [
        morning_digest.as_ref(),
        day_digest.as_ref(),
        evening_digest.as_ref(),
    ]
    .iter()
    .filter(|&&x| x.is_some())
    .count() as i32;

    // Get plan capacity based on country and plan type
    // US/CA: always 400 messages (hosted plan)
    // Other countries: monitor=40, digest=120
    let plan_type = user.plan_type.clone();
    let detected_country = crate::utils::country::get_country_code_from_phone(&user.phone_number);
    let is_us_ca = matches!(detected_country.as_deref(), Some("US") | Some("CA"));
    let plan_capacity = if is_us_ca {
        400 // US/CA hosted plan
    } else {
        match plan_type.as_deref() {
            Some("digest") => 120,
            _ => 40, // monitor or default
        }
    };

    // Calculate digests per month
    let digests_per_month = digest_count * 30;

    // Get detailed usage breakdown from usage_logs
    let (
        is_example_data,
        avg_sms_notifications_per_day,
        avg_call_notifications_per_day,
        avg_messages_per_day,
        avg_voice_mins_per_day,
    ) = {
        let mut conn = state.db_pool.get().expect("Failed to get DB connection");
        let now: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let thirty_days_ago: i64 = now - 2_592_000; // 30 * 86_400

        // Count active days
        let active_days_count: i64 = usage_logs::table
            .select(sql::<BigInt>("COUNT(DISTINCT created_at / 86400)"))
            .filter(usage_logs::user_id.eq(auth_user.user_id))
            .filter(usage_logs::created_at.ge(thirty_days_ago as i32))
            .first(&mut conn)
            .unwrap_or(0);

        if active_days_count < 3 {
            // Not enough data - use example data based on plan type
            // Monitor plan: typically no digests, ~0.5 critical notis/day
            // Digest plan: typically 2 digests (already counted), ~0.5 critical notis/day, ~0.3 messages/day
            let example_sms_notis = 0.5_f32;
            let example_call_notis = 0.0_f32;
            let example_messages = if plan_type.as_deref() == Some("digest") {
                0.3_f32
            } else {
                0.0_f32
            };
            let example_voice_mins = 0.0_f32;

            (
                true,
                example_sms_notis,
                example_call_notis,
                example_messages,
                example_voice_mins,
            )
        } else {
            // Get oldest day in the period
            let oldest_day: i64 = usage_logs::table
                .select(sql::<BigInt>("MIN(created_at / 86400)"))
                .filter(usage_logs::user_id.eq(auth_user.user_id))
                .filter(usage_logs::created_at.ge(thirty_days_ago as i32))
                .first(&mut conn)
                .unwrap_or(0);

            let current_day: i64 = now / 86_400;
            let days_span = (current_day - oldest_day + 1).max(1) as f32;

            let start_timestamp: i64 = oldest_day * 86_400;
            let end_timestamp: i64 = (current_day + 1) * 86_400;

            // SMS notifications: _critical (not call) and _priority_sms
            let sms_notifications: i64 = usage_logs::table
                .filter(usage_logs::user_id.eq(auth_user.user_id))
                .filter(
                    usage_logs::activity_type
                        .like("%_critical")
                        .or(usage_logs::activity_type.like("%_priority_sms"))
                        .or(usage_logs::activity_type.eq("noti_msg")),
                )
                .filter(usage_logs::activity_type.not_like("%_priority_call"))
                .filter(usage_logs::created_at.ge(start_timestamp as i32))
                .filter(usage_logs::created_at.lt(end_timestamp as i32))
                .count()
                .get_result(&mut conn)
                .unwrap_or(0);

            // Call notifications: _priority_call, noti_call
            let call_notifications: i64 = usage_logs::table
                .filter(usage_logs::user_id.eq(auth_user.user_id))
                .filter(
                    usage_logs::activity_type
                        .like("%_priority_call")
                        .or(usage_logs::activity_type.eq("noti_call")),
                )
                .filter(usage_logs::created_at.ge(start_timestamp as i32))
                .filter(usage_logs::created_at.lt(end_timestamp as i32))
                .count()
                .get_result(&mut conn)
                .unwrap_or(0);

            // Regular SMS messages (activity_type = "sms" or "message")
            let messages: i64 = usage_logs::table
                .filter(usage_logs::user_id.eq(auth_user.user_id))
                .filter(
                    usage_logs::activity_type
                        .eq("sms")
                        .or(usage_logs::activity_type.eq("message")),
                )
                .filter(usage_logs::created_at.ge(start_timestamp as i32))
                .filter(usage_logs::created_at.lt(end_timestamp as i32))
                .count()
                .get_result(&mut conn)
                .unwrap_or(0);

            // Voice minutes: sum of call_duration for voice/call activities
            let voice_seconds: Option<f32> = usage_logs::table
                .select(sql::<diesel::sql_types::Nullable<Float>>(
                    "SUM(COALESCE(call_duration, 0))",
                ))
                .filter(usage_logs::user_id.eq(auth_user.user_id))
                .filter(
                    usage_logs::activity_type
                        .like("%voice%")
                        .or(usage_logs::activity_type.like("%call%")),
                )
                .filter(usage_logs::created_at.ge(start_timestamp as i32))
                .filter(usage_logs::created_at.lt(end_timestamp as i32))
                .first(&mut conn)
                .unwrap_or(None);

            let voice_mins = voice_seconds.unwrap_or(0.0) / 60.0;

            (
                false,
                sms_notifications as f32 / days_span,
                call_notifications as f32 / days_span,
                messages as f32 / days_span,
                voice_mins / days_span,
            )
        }
    };

    // Combined notifications for simple display
    let avg_notifications_per_day = avg_sms_notifications_per_day + avg_call_notifications_per_day;
    let notifications_per_month = (avg_notifications_per_day * 30.0).round() as i32;
    let messages_per_month = (avg_messages_per_day * 30.0).round() as i32;

    // === CALCULATE ACTUAL USAGE THIS BILLING PERIOD ===
    let (
        actual_notifications_used,
        actual_voice_mins_used,
        actual_messages_used,
        actual_digests_used,
    ) = {
        let mut conn = state.db_pool.get().expect("Failed to get DB connection");
        let now: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Calculate billing period start
        let billing_period_start: i64 = if let Some(next_billing) = user.next_billing_date_timestamp
        {
            // Billing period started 30 days before next billing date
            (next_billing as i64) - 2_592_000 // 30 days in seconds
        } else {
            // No billing date set, default to 30 days ago
            now - 2_592_000
        };

        // Actual SMS notifications this billing period
        let actual_sms_notis: i64 = usage_logs::table
            .filter(usage_logs::user_id.eq(auth_user.user_id))
            .filter(
                usage_logs::activity_type
                    .like("%_critical")
                    .or(usage_logs::activity_type.like("%_priority_sms"))
                    .or(usage_logs::activity_type.eq("noti_msg")),
            )
            .filter(usage_logs::activity_type.not_like("%_priority_call"))
            .filter(usage_logs::created_at.ge(billing_period_start as i32))
            .count()
            .get_result(&mut conn)
            .unwrap_or(0);

        // Actual call notifications this billing period
        let actual_call_notis: i64 = usage_logs::table
            .filter(usage_logs::user_id.eq(auth_user.user_id))
            .filter(
                usage_logs::activity_type
                    .like("%_priority_call")
                    .or(usage_logs::activity_type.eq("noti_call")),
            )
            .filter(usage_logs::created_at.ge(billing_period_start as i32))
            .count()
            .get_result(&mut conn)
            .unwrap_or(0);

        // Actual messages this billing period
        let actual_msgs: i64 = usage_logs::table
            .filter(usage_logs::user_id.eq(auth_user.user_id))
            .filter(
                usage_logs::activity_type
                    .eq("sms")
                    .or(usage_logs::activity_type.eq("message")),
            )
            .filter(usage_logs::created_at.ge(billing_period_start as i32))
            .count()
            .get_result(&mut conn)
            .unwrap_or(0);

        // Actual voice minutes this billing period
        let actual_voice_secs: Option<f32> = usage_logs::table
            .select(sql::<diesel::sql_types::Nullable<Float>>(
                "SUM(COALESCE(call_duration, 0))",
            ))
            .filter(usage_logs::user_id.eq(auth_user.user_id))
            .filter(
                usage_logs::activity_type
                    .like("%voice%")
                    .or(usage_logs::activity_type.like("%call%")),
            )
            .filter(usage_logs::created_at.ge(billing_period_start as i32))
            .first(&mut conn)
            .unwrap_or(None);

        // Actual digests this billing period
        let actual_digs: i64 = usage_logs::table
            .filter(usage_logs::user_id.eq(auth_user.user_id))
            .filter(usage_logs::activity_type.eq("digest"))
            .filter(usage_logs::created_at.ge(billing_period_start as i32))
            .count()
            .get_result(&mut conn)
            .unwrap_or(0);

        (
            (actual_sms_notis + actual_call_notis) as i32,
            (actual_voice_secs.unwrap_or(0.0) / 60.0).round() as i32,
            actual_msgs as i32,
            actual_digs as i32,
        )
    };

    // Total usage includes digests, notifications, and messages
    // Digests and messages cost ~3x a notification for display purposes
    // So we weight them: digests/messages = 1 unit, notifications = 1/3 unit
    let weighted_notifications = (notifications_per_month as f32 / 3.0).round() as i32;
    let total_usage_per_month = digests_per_month + weighted_notifications + messages_per_month;

    // Remaining capacity expressed in notification equivalents
    // Each remaining "unit" can buy ~3 notifications
    let remaining_units = plan_capacity - total_usage_per_month;
    let remaining_capacity = remaining_units * 3; // Convert to notification equivalents

    let usage_percentage = if plan_capacity > 0 {
        (total_usage_per_month as f32 / plan_capacity as f32) * 100.0
    } else {
        0.0
    };

    // Calculate days until billing
    let days_until_billing = if let Some(next_billing) = user.next_billing_date_timestamp {
        let now = chrono::Utc::now().timestamp() as i32;
        let days = (next_billing - now) / 86400;
        Some(days.max(0))
    } else {
        None
    };

    let has_auto_topup = user.charge_when_under;

    // Calculate overage if usage exceeds capacity
    let overage = if total_usage_per_month > plan_capacity {
        // Convert overage units to notification equivalents (units * 3)
        let notifications_over = (total_usage_per_month - plan_capacity) * 3;

        // Get country for pricing
        let country_code = crate::utils::country::get_country_code_from_phone(&user.phone_number)
            .unwrap_or_else(|| "US".to_string());

        // Calculate euro cost for overage (weighted by SMS vs call ratio)
        let estimated_cost_euros = if is_us_or_ca(&country_code) {
            // US/CA pricing
            let sms_cost = 0.075_f32; // per SMS notification
            let call_cost = 0.15_f32; // per call notification (more expensive)

            // Calculate weighted average based on user's SMS/call ratio
            let total_notis = avg_sms_notifications_per_day + avg_call_notifications_per_day;
            if total_notis > 0.0 {
                let sms_ratio = avg_sms_notifications_per_day / total_notis;
                let call_ratio = avg_call_notifications_per_day / total_notis;
                let weighted_cost = (sms_ratio * sms_cost + call_ratio * call_cost) * 0.92; // USD to EUR
                notifications_over as f32 * weighted_cost
            } else {
                notifications_over as f32 * sms_cost * 0.92
            }
        } else {
            // Euro countries: use notification price
            if let Ok(pricing) = get_notification_only_pricing(&state, &country_code).await {
                // SMS notifications use notification_price, call notifications are more expensive
                let sms_cost = pricing.notification_price;
                let call_cost = pricing.calculated_voice_price * 0.5 + 0.11; // ~30 sec call + ElevenLabs

                let total_notis = avg_sms_notifications_per_day + avg_call_notifications_per_day;
                if total_notis > 0.0 {
                    let sms_ratio = avg_sms_notifications_per_day / total_notis;
                    let call_ratio = avg_call_notifications_per_day / total_notis;
                    notifications_over as f32 * (sms_ratio * sms_cost + call_ratio * call_cost)
                } else {
                    notifications_over as f32 * sms_cost
                }
            } else {
                // Fallback
                notifications_over as f32 * 0.195
            }
        };

        Some(OverageInfo {
            notifications_over,
            estimated_cost_euros,
            covered_by_auto_topup: has_auto_topup && plan_type.as_deref() == Some("digest"),
        })
    } else {
        None
    };

    // === CALCULATE NEW SIMPLIFIED STATUS FIELDS ===

    // Status based on usage percentage (only warn at 95%+)
    let status = if usage_percentage <= 95.0 {
        "on_track".to_string()
    } else if usage_percentage <= 100.0 {
        "warning".to_string()
    } else {
        "over_quota".to_string()
    };

    // Display string for percentage
    let usage_percentage_display =
        format!("{}% of monthly quota used", usage_percentage.round() as i32);

    // Get overage credits from user
    let overage_credits = user.credits;

    // Calculate estimated daily overage cost (for users going over quota)
    let daily_overage_cost = if let Some(ref ov) = overage {
        // Monthly overage cost / 30 days
        ov.estimated_cost_euros / 30.0
    } else {
        0.0
    };

    // Overage days remaining (if no auto top-up and has credits and is over quota)
    let overage_days_remaining =
        if !has_auto_topup && overage_credits > 0.0 && daily_overage_cost > 0.0 {
            Some((overage_credits / daily_overage_cost).floor() as i32)
        } else {
            None
        };

    // Estimated monthly extra cost (if auto top-up enabled and over quota)
    let estimated_monthly_extra_cost = if has_auto_topup && overage.is_some() {
        overage.as_ref().map(|ov| ov.estimated_cost_euros)
    } else {
        None
    };

    // Generate recommendation based on plan type and status
    let recommendation = if status == "on_track" {
        None
    } else {
        match plan_type.as_deref() {
            Some("monitor") => {
                // Monitor plan user going over
                if digest_count > 0 {
                    Some(UsageRecommendation {
                        message: "Reduce digest frequency to stay within quota".to_string(),
                        action_type: "reduce_digests".to_string(),
                        action_link: Some("/dashboard?tab=settings".to_string()),
                    })
                } else {
                    Some(UsageRecommendation {
                        message: "Upgrade to Digest Plan for 3x more capacity".to_string(),
                        action_type: "upgrade_plan".to_string(),
                        action_link: Some("/pricing".to_string()),
                    })
                }
            }
            Some("digest") => {
                // Digest plan user going over
                if !has_auto_topup {
                    Some(UsageRecommendation {
                        message: "Enable auto top-up to cover overages automatically".to_string(),
                        action_type: "enable_topup".to_string(),
                        action_link: Some("/dashboard?tab=billing".to_string()),
                    })
                } else {
                    // They have auto top-up, no action needed - just show cost
                    None
                }
            }
            _ => None,
        }
    };

    // === CALCULATE SEGMENTED BAR FIELDS ===
    let country_code = crate::utils::country::get_country_code_from_phone(&user.phone_number)
        .unwrap_or_else(|| "US".to_string());
    let is_notification_only = is_notification_only_code(&country_code);

    // Calculate percentages as share of plan capacity
    let capacity_f = plan_capacity as f32;
    let digest_percentage = if capacity_f > 0.0 {
        (digests_per_month as f32 / capacity_f) * 100.0
    } else {
        0.0
    };

    // Notifications are weighted as 1/3 unit each
    let sms_noti_units = (avg_sms_notifications_per_day * 30.0) / 3.0;
    let call_noti_units = (avg_call_notifications_per_day * 30.0) / 3.0;

    let sms_noti_percentage = if capacity_f > 0.0 {
        (sms_noti_units / capacity_f) * 100.0
    } else {
        0.0
    };

    let call_noti_percentage = if capacity_f > 0.0 {
        (call_noti_units / capacity_f) * 100.0
    } else {
        0.0
    };

    let messages_percentage = if capacity_f > 0.0 {
        (messages_per_month as f32 / capacity_f) * 100.0
    } else {
        0.0
    };

    // Voice is not part of the quota system, so we don't include it in the bar
    // But we track it separately for display purposes
    let voice_percentage = 0.0_f32;

    Ok(Json(UsageProjectionResponse {
        plan_type,
        plan_capacity,
        has_auto_topup,
        days_until_billing,
        is_example_data,
        status,
        usage_percentage_display,
        overage_days_remaining,
        estimated_monthly_extra_cost,
        recommendation,
        overage_credits,
        digest_count,
        digests_per_month,
        avg_sms_notifications_per_day,
        avg_call_notifications_per_day,
        avg_messages_per_day,
        avg_voice_mins_per_day,
        avg_notifications_per_day,
        notifications_per_month,
        total_usage_per_month,
        usage_percentage,
        remaining_capacity,
        overage,
        is_notification_only,
        digest_percentage,
        sms_noti_percentage,
        call_noti_percentage,
        messages_percentage,
        voice_percentage,
        actual_notifications_used,
        actual_voice_mins_used,
        actual_messages_used,
        actual_digests_used,
    }))
}

// ============================================
// BYOT Usage Tracking
// ============================================

/// Activity count and cost for BYOT users
#[derive(Serialize)]
pub struct ByotActivityCost {
    pub count: i32,
    pub cost_eur: f32,
}

/// Breakdown of BYOT user's usage
#[derive(Serialize)]
pub struct ByotUsageBreakdown {
    pub digests: ByotActivityCost,
    pub sms_notifications: ByotActivityCost,
    pub call_notifications: ByotActivityCost,
    pub messages: ByotActivityCost,
    pub voice_minutes: f32,
    pub voice_cost_eur: f32,
}

/// Percentages for segmented bar display
#[derive(Serialize)]
pub struct ByotUsagePercentages {
    pub digests: f32,
    pub sms_notifications: f32,
    pub call_notifications: f32,
    pub messages: f32,
    pub voice: f32,
}

/// Response for BYOT usage endpoint
#[derive(Serialize)]
pub struct ByotUsageResponse {
    pub total_cost_eur: f32,
    pub country_code: String,
    pub country_name: String,
    pub days_until_billing: Option<i32>,
    pub breakdown: ByotUsageBreakdown,
    pub percentages: ByotUsagePercentages,
}

/// GET /api/pricing/byot-usage
/// Returns BYOT user's usage and estimated costs for the current billing period
pub async fn get_byot_usage(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ByotUsageResponse>, (axum::http::StatusCode, String)> {
    use diesel::sql_types::Float;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Get user from database
    let user = state
        .user_core
        .find_by_id(auth_user.user_id)
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((
            axum::http::StatusCode::NOT_FOUND,
            "User not found".to_string(),
        ))?;

    // Verify this is a BYOT user
    if user.plan_type.as_deref() != Some("byot") {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "This endpoint is only for BYOT users".to_string(),
        ));
    }

    // Get user's country for pricing
    let country_code = crate::utils::country::get_country_code_from_phone(&user.phone_number)
        .ok_or((
            axum::http::StatusCode::BAD_REQUEST,
            "Could not detect country from phone number.".to_string(),
        ))?;

    // Fetch pricing for user's country using get_country_capability
    let capability = get_country_capability(&state, &country_code)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get pricing: {}", e),
            )
        })?;

    // Get pricing rates
    let sms_per_segment = capability.outbound_sms_price.unwrap_or(0.0);
    let notification_cost = sms_per_segment * 1.5; // 1.5 segments per notification
    let message_cost = sms_per_segment * 3.0; // 3 segments per message
    let digest_cost = sms_per_segment * 3.0; // 3 segments per digest

    // Voice cost includes ElevenLabs AI ($0.11/min)
    const ELEVENLABS_COST_PER_MIN: f32 = 0.11;
    let voice_per_min =
        capability.outbound_voice_price_per_min.unwrap_or(0.0) + ELEVENLABS_COST_PER_MIN;

    // Calculate billing period
    let now: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let billing_period_start: i64 = if let Some(next_billing) = user.next_billing_date_timestamp {
        (next_billing as i64) - 2_592_000 // 30 days before next billing
    } else {
        now - 2_592_000 // Default to 30 days ago
    };

    let days_until_billing = user
        .next_billing_date_timestamp
        .map(|ts| ((ts as i64 - now) / 86_400).max(0) as i32);

    // Query usage from database
    let mut conn = state.db_pool.get().expect("Failed to get DB connection");

    // Digests
    let digest_count: i64 = usage_logs::table
        .filter(usage_logs::user_id.eq(auth_user.user_id))
        .filter(usage_logs::activity_type.eq("digest"))
        .filter(usage_logs::created_at.ge(billing_period_start as i32))
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    // SMS notifications
    let sms_noti_count: i64 = usage_logs::table
        .filter(usage_logs::user_id.eq(auth_user.user_id))
        .filter(
            usage_logs::activity_type
                .like("%_critical")
                .or(usage_logs::activity_type.like("%_priority_sms"))
                .or(usage_logs::activity_type.eq("noti_msg")),
        )
        .filter(usage_logs::activity_type.not_like("%_priority_call"))
        .filter(usage_logs::created_at.ge(billing_period_start as i32))
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    // Call notifications
    let call_noti_count: i64 = usage_logs::table
        .filter(usage_logs::user_id.eq(auth_user.user_id))
        .filter(
            usage_logs::activity_type
                .like("%_priority_call")
                .or(usage_logs::activity_type.eq("noti_call")),
        )
        .filter(usage_logs::created_at.ge(billing_period_start as i32))
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    // Regular messages
    let message_count: i64 = usage_logs::table
        .filter(usage_logs::user_id.eq(auth_user.user_id))
        .filter(
            usage_logs::activity_type
                .eq("sms")
                .or(usage_logs::activity_type.eq("message")),
        )
        .filter(usage_logs::created_at.ge(billing_period_start as i32))
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    // Voice minutes
    let voice_seconds: Option<f32> = usage_logs::table
        .select(sql::<diesel::sql_types::Nullable<Float>>(
            "SUM(COALESCE(call_duration, 0))",
        ))
        .filter(usage_logs::user_id.eq(auth_user.user_id))
        .filter(
            usage_logs::activity_type
                .like("%voice%")
                .or(usage_logs::activity_type.like("%call%")),
        )
        .filter(usage_logs::created_at.ge(billing_period_start as i32))
        .first(&mut conn)
        .unwrap_or(None);

    let voice_minutes = voice_seconds.unwrap_or(0.0) / 60.0;

    // Calculate costs
    let digest_total = digest_count as f32 * digest_cost;
    let sms_noti_total = sms_noti_count as f32 * notification_cost;
    let call_noti_total = call_noti_count as f32 * voice_per_min * 0.5; // ~30 sec per call notification
    let message_total = message_count as f32 * message_cost;
    let voice_total = voice_minutes * voice_per_min;

    let total_cost = digest_total + sms_noti_total + call_noti_total + message_total + voice_total;

    // Calculate percentages for segmented bar
    let (digest_pct, sms_noti_pct, call_noti_pct, message_pct, voice_pct) = if total_cost > 0.0 {
        (
            (digest_total / total_cost) * 100.0,
            (sms_noti_total / total_cost) * 100.0,
            (call_noti_total / total_cost) * 100.0,
            (message_total / total_cost) * 100.0,
            (voice_total / total_cost) * 100.0,
        )
    } else {
        (0.0, 0.0, 0.0, 0.0, 0.0)
    };

    Ok(Json(ByotUsageResponse {
        total_cost_eur: (total_cost * 100.0).round() / 100.0, // Round to 2 decimals
        country_code: country_code.clone(),
        country_name: get_country_name(&country_code),
        days_until_billing,
        breakdown: ByotUsageBreakdown {
            digests: ByotActivityCost {
                count: digest_count as i32,
                cost_eur: (digest_total * 100.0).round() / 100.0,
            },
            sms_notifications: ByotActivityCost {
                count: sms_noti_count as i32,
                cost_eur: (sms_noti_total * 100.0).round() / 100.0,
            },
            call_notifications: ByotActivityCost {
                count: call_noti_count as i32,
                cost_eur: (call_noti_total * 100.0).round() / 100.0,
            },
            messages: ByotActivityCost {
                count: message_count as i32,
                cost_eur: (message_total * 100.0).round() / 100.0,
            },
            voice_minutes: (voice_minutes * 10.0).round() / 10.0, // Round to 1 decimal
            voice_cost_eur: (voice_total * 100.0).round() / 100.0,
        },
        percentages: ByotUsagePercentages {
            digests: digest_pct,
            sms_notifications: sms_noti_pct,
            call_notifications: call_noti_pct,
            messages: message_pct,
            voice: voice_pct,
        },
    }))
}
