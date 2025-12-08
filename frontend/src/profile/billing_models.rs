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
    pub phone_number_country: Option<String>,
    pub server_ip: Option<String>,
    pub plan_type: Option<String>,
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct StripeSetupIntentResponse {
    pub client_secret: String, // Client secret for the SetupIntent
}

pub const MIN_TOPUP_AMOUNT_CREDITS: f32 = 3.00;
pub const VOICE_SECOND_COST: f32 = 0.0033;
pub const MESSAGE_COST: f32 = 0.20;

/// Usage projection response - all values in NOTIFICATION UNITS (not currency)
#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct UsageProjection {
    /// User's plan type: "monitor" or "digest"
    pub plan_type: Option<String>,
    /// Plan capacity in notifications per month (40 for monitor, 120 for digest)
    pub plan_capacity: i32,
    /// Whether auto top-up is enabled
    pub has_auto_topup: bool,
    /// Days until billing cycle resets
    pub days_until_billing: Option<i32>,
    /// True if using example data (< 3 days of usage history)
    pub is_example_data: bool,

    // Digest usage
    /// Number of active digests per day (0-3)
    pub digest_count: i32,
    /// Digests per month (digest_count * 30)
    pub digests_per_month: i32,

    // Detailed breakdown (all averages from last 30 days)
    /// SMS notifications per day (_critical, _priority_sms) - cheaper
    pub avg_sms_notifications_per_day: f32,
    /// Call notifications per day (_priority_call, noti_call) - more expensive
    pub avg_call_notifications_per_day: f32,
    /// Regular SMS messages per day
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

    // Segmented bar fields
    /// Whether this is a notification-only country
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

    // Overage credits info
    /// User's overage credits balance
    pub overage_credits: f32,
    /// Days overage credits will last at current usage rate
    pub overage_days_remaining: Option<i32>,
}

/// Overage information - this is where we show euro amounts
#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct OverageInfo {
    /// How many notifications over the plan limit
    pub notifications_over: i32,
    /// Estimated euro cost for the overage
    pub estimated_cost_euros: f32,
    /// Whether auto top-up will cover it
    pub covered_by_auto_topup: bool,
}

pub fn format_timestamp(timestamp: i32) -> String {
    match Utc.timestamp_opt(timestamp as i64, 0) {
        chrono::offset::LocalResult::Single(dt) => {
            dt.format("%B %d, %Y").to_string()
        },
        _ => "Unknown date".to_string(),
    }
}
