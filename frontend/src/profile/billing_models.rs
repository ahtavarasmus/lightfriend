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

#[derive(Deserialize, Clone, PartialEq, Default)]
#[serde(default)]
pub struct UserProfile {
    pub id: i32,
    pub email: String,
    pub phone_number: String,
    pub nickname: Option<String>,
    pub verified: bool,
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
    pub twilio_sid: Option<String>,
    pub twilio_token: Option<String>,
    pub openrouter_api_key: Option<String>,
    pub textbee_device_id: Option<String>,
    pub textbee_api_key: Option<String>,
    pub estimated_monitoring_cost: f32,
    pub location: Option<String>,
    pub nearby_places: Option<String>,
    pub server_ip: Option<String>,
    pub plan_type: Option<String>,
    pub phone_service_active: Option<bool>,
    pub llm_provider: Option<String>,
    pub auto_create_items: Option<bool>,
    pub system_important_notify: Option<bool>,
    pub has_any_connection: bool,
    pub digest_enabled: Option<bool>,
    pub digest_time: Option<String>,
    pub auto_track_items_system: Option<bool>,
    pub auto_confirm_tracked_items: Option<bool>,
}

pub const MIN_TOPUP_AMOUNT_CREDITS: f32 = 3.00;

#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct UsageLogEntry {
    pub activity_type: String,
    pub credits: Option<f32>,
    pub created_at: i32,
    pub call_duration: Option<i32>,
}
