use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone, PartialEq)]
pub struct AutoTopupSettings {
    pub active: bool,
    pub amount: Option<i32>,
}

#[derive(Serialize, Clone, PartialEq)]
pub struct BuyCreditsRequest {
    pub amount_dollars: f64, // Amount in dollars
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
    pub time_to_live: i32,
    pub time_to_delete: bool,
    pub iq: i32,
    pub info: Option<String>,
    pub charge_when_under: bool,
    pub charge_back_to: Option<i32>,
    pub stripe_payment_method_id: Option<String>,
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct StripeSetupIntentResponse {
    pub client_secret: String, // Client secret for the SetupIntent
}

pub const IQ_TO_EURO_RATE: f64 = 60.0; // 60 IQ = 1 Euro
pub const MIN_TOPUP_AMOUNT_DOLLARS: f64 = 3.0;
pub const MIN_TOPUP_AMOUNT_IQ: i32 = (MIN_TOPUP_AMOUNT_DOLLARS * IQ_TO_EURO_RATE) as i32;

pub fn format_timestamp(timestamp: i32) -> String {
    match Utc.timestamp_opt(timestamp as i64, 0) {
        chrono::offset::LocalResult::Single(dt) => {
            dt.format("%B %d, %Y").to_string()
        },
        _ => "Unknown date".to_string(),
    }
}
