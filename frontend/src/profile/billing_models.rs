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
    pub phone_service_active: Option<bool>, // whether phone service is active - can be disabled for security
    pub llm_provider: Option<String>, // "openai" (default) or "tinfoil" - user's LLM provider preference
    pub auto_create_items: Option<bool>, // whether to auto-detect and create trackable items from emails/messages
    pub has_any_connection: bool, // whether user has connected any service (for onboarding modal)
}

pub const MIN_TOPUP_AMOUNT_CREDITS: f32 = 3.00;

/// Usage projection response - all values in NOTIFICATION UNITS (not currency)
#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct UsageProjection {
    /// User's plan type: "assistant", "autopilot", or "byot"
    pub plan_type: Option<String>,
    /// Plan capacity in notifications per month
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

    // Actual usage this billing period (not projected)
    /// Actual notifications used this billing period
    pub actual_notifications_used: i32,
    /// Actual voice minutes used this billing period
    pub actual_voice_mins_used: i32,
    /// Actual messages sent this billing period
    pub actual_messages_used: i32,
    /// Actual digests sent this billing period
    pub actual_digests_used: i32,
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

// ============================================
// BYOT Usage Types
// ============================================

/// Activity count and cost for BYOT users
#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct ByotActivityCost {
    pub count: i32,
    pub cost_eur: f32,
}

/// Breakdown of BYOT user's usage
#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct ByotUsageBreakdown {
    pub digests: ByotActivityCost,
    pub sms_notifications: ByotActivityCost,
    pub call_notifications: ByotActivityCost,
    pub messages: ByotActivityCost,
    pub voice_minutes: f32,
    pub voice_cost_eur: f32,
}

/// Percentages for segmented bar display
#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct ByotUsagePercentages {
    pub digests: f32,
    pub sms_notifications: f32,
    pub call_notifications: f32,
    pub messages: f32,
    pub voice: f32,
}

/// Response for BYOT usage endpoint
#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct ByotUsageResponse {
    pub total_cost_eur: f32,
    pub country_code: String,
    pub country_name: String,
    pub days_until_billing: Option<i32>,
    pub breakdown: ByotUsageBreakdown,
    pub percentages: ByotUsagePercentages,
}
