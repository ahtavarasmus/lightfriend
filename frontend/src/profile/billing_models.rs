use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone, PartialEq)]
pub struct AutoTopupSettings {
    pub active: bool,
    pub amount: Option<f32>,
}

#[derive(Serialize, Clone, PartialEq)]
pub struct BuyCreditsRequest {
    pub amount_dollars: f32, // Amount in dollars
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct ApiResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct UserProfile {
    pub id: i32,
    pub email: String,
    pub phone_number: String,
    pub nickname: Option<String>,
    pub verified: bool,
    pub time_to_live: Option<i32>,
    pub time_to_delete: bool,
    pub credits: f32,
    pub info: Option<String>,
    pub charge_when_under: bool,
    pub charge_back_to: Option<f32>,
    pub stripe_payment_method_id: Option<String>,
    pub timezone: Option<String>,
    pub timezone_auto: Option<bool>,
    pub sub_tier: Option<String>,
    pub credits_left: f32,
    pub discount: bool,
    pub notify: bool,
    pub preferred_number: Option<String>,
    pub agent_language: String,
    pub notification_type: Option<String>,
    pub sub_country: Option<String>,
    pub save_context: Option<i32>,
    pub days_until_billing: Option<i32>,
    pub digests_reserved: i32,
    pub pairing_code: Option<String>,
    pub server_ip: Option<String>,
    pub twilio_sid: Option<String>,
    pub twilio_token: Option<String>,
    pub openrouter_api_key: Option<String>,
    pub textbee_device_id: Option<String>,
    pub textbee_api_key: Option<String>,
    pub estimated_monitoring_cost: f32,
    pub location: Option<String>,
    pub nearby_places: Option<String>,
    pub phone_number_country: Option<String>,
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct StripeSetupIntentResponse {
    pub client_secret: String, // Client secret for the SetupIntent
}

pub const MIN_TOPUP_AMOUNT_CREDITS: f32 = 3.00;
pub const VOICE_SECOND_COST: f32 = 0.0033;
pub const MESSAGE_COST: f32 = 0.20;

pub fn format_timestamp(timestamp: i32) -> String {
    match Utc.timestamp_opt(timestamp as i64, 0) {
        chrono::offset::LocalResult::Single(dt) => {
            dt.format("%B %d, %Y").to_string()
        },
        _ => "Unknown date".to_string(),
    }
}
