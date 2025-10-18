use chrono::{TimeZone, Utc};
use serde::Deserialize;

#[derive(Deserialize, Clone, PartialEq)]
pub struct UserProfile {
    pub id: i32,
    pub phone_number: String,
    pub nickname: Option<String>,
    pub credits: f32,
    pub info: Option<String>,
    pub preferred_number: Option<String>,
    pub timezone: Option<String>,
    pub timezone_auto: Option<bool>,
    pub credits_left: f32,
    pub agent_language: String,
    pub notification_type: Option<String>,
    pub save_context: Option<i32>,
    pub twilio_sid: Option<String>,
    pub twilio_token: Option<String>,
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
