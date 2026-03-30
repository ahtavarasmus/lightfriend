use crate::pg_schema::{
    admin_alerts, country_availability, disabled_alert_types, message_status_log, site_metrics,
    user_settings, users, waitlist,
};
use diesel::prelude::*;
use serde::Serialize;

#[derive(Queryable, Selectable, Insertable, Clone)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: i32,
    pub email: String,
    pub password_hash: String,
    pub phone_number: String,
    pub nickname: Option<String>,  // what user wants the ai to call them
    pub time_to_live: Option<i32>, // if user has not verified their account in some time it will be deleted
    pub credits: f32,              // user purchased credits, will not expire
    pub preferred_number: Option<String>, // number the user prefers lightfriend texting/calling them from
    pub charge_when_under: bool, // flag for if user wants to automatically buy more overage credits
    pub charge_back_to: Option<f32>, // the credit amount to charge
    pub stripe_customer_id: Option<String>,
    pub stripe_payment_method_id: Option<String>,
    pub stripe_checkout_session_id: Option<String>,
    pub sub_tier: Option<String>, // tier 2 only, differentiated by plan_type
    pub credits_left: f32, // free credits that reset every month while in the monthly sub. will always be consumed before one time credits
    pub last_credits_notification: Option<i32>, // Unix timestamp of last insufficient credits notification to prevent spam
    pub next_billing_date_timestamp: Option<i32>, // when is user next billed for their subscription
    pub magic_token: Option<String>,            // token for magic link login/password setup
    pub refresh_token_hash: Option<String>,     // hash of the currently active refresh token
    pub refresh_token_compromised: bool,        // set if an invalidated refresh token is reused
    pub magic_token_expires_at: Option<i32>,    // expiry for magic link token
    pub plan_type: Option<String>,              // "assistant", "autopilot", or "byot"
    pub matrix_e2ee_enabled: bool,              // whether E2EE is enabled for Matrix messaging
}

#[derive(Queryable, Selectable, Insertable, Clone)]
#[diesel(table_name = user_settings)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserSettings {
    pub id: i32,
    pub user_id: i32,
    pub notify: bool,
    pub notification_type: Option<String>,
    pub timezone_auto: Option<bool>,
    pub agent_language: String,
    pub sub_country: Option<String>,
    pub save_context: Option<i32>,
    pub critical_enabled: Option<String>,
    pub elevenlabs_phone_number_id: Option<String>,
    pub notify_about_calls: bool,
    pub action_on_critical_message: Option<String>,
    pub phone_service_active: bool,
    pub default_notification_mode: Option<String>,
    pub default_notification_type: Option<String>,
    pub default_notify_on_call: i32,
    pub llm_provider: Option<String>,
    pub phone_contact_notification_mode: Option<String>,
    pub phone_contact_notification_type: Option<String>,
    pub phone_contact_notify_on_call: i32,
    pub auto_create_items: bool,
    pub system_important_notify: bool,
    pub digest_enabled: bool,
    pub digest_time: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = user_settings)]
pub struct NewUserSettings {
    pub user_id: i32,
    pub notify: bool,
    pub notification_type: Option<String>,
    pub timezone_auto: Option<bool>,
    pub agent_language: String,
    pub sub_country: Option<String>,
    pub save_context: Option<i32>,
    pub critical_enabled: Option<String>,
    pub notify_about_calls: bool,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = country_availability)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CountryAvailability {
    pub has_local_numbers: bool,
    pub outbound_sms_price: Option<f32>,
    pub inbound_sms_price: Option<f32>,
    pub outbound_voice_price_per_min: Option<f32>,
    pub inbound_voice_price_per_min: Option<f32>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = country_availability)]
pub struct NewCountryAvailability {
    pub country_code: String,
    pub has_local_numbers: bool,
    pub outbound_sms_price: Option<f32>,
    pub inbound_sms_price: Option<f32>,
    pub outbound_voice_price_per_min: Option<f32>,
    pub inbound_voice_price_per_min: Option<f32>,
    pub last_checked: i32,
    pub created_at: i32,
}

// Waitlist models for users who want updates but haven't subscribed
#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = waitlist)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WaitlistEntry {
    pub email: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = waitlist)]
pub struct NewWaitlistEntry {
    pub email: String,
    pub created_at: i32,
}

// Refund tracking models
#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = crate::pg_schema::refund_info)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct RefundInfo {
    pub has_refunded: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::pg_schema::refund_info)]
pub struct NewRefundInfo {
    pub user_id: i32,
    pub has_refunded: i32,
}

// Message Status Log models for tracking SMS delivery metadata (no message content)
#[derive(Queryable, Selectable, Clone, Debug, Serialize)]
#[diesel(table_name = message_status_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct MessageStatusLog {
    pub id: i32,
    pub message_sid: String,
    pub user_id: i32,
    pub direction: String,
    pub to_number: String,
    pub from_number: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: i32,
    pub updated_at: i32,
    pub price: Option<f32>,
    pub price_unit: Option<String>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = message_status_log)]
pub struct NewMessageStatusLog {
    pub message_sid: String,
    pub user_id: i32,
    pub direction: String,
    pub to_number: String,
    pub from_number: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: i32,
    pub updated_at: i32,
    pub price: Option<f32>,
    pub price_unit: Option<String>,
}

// Admin Alert models for tracking system alerts
#[derive(Queryable, Selectable, Clone, Debug, Serialize)]
#[diesel(table_name = admin_alerts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AdminAlert {
    pub id: i32,
    pub alert_type: String,
    pub severity: String,
    pub message: String,
    pub location: String,
    pub module: String,
    pub acknowledged: i32,
    pub created_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = admin_alerts)]
pub struct NewAdminAlert {
    pub alert_type: String,
    pub severity: String,
    pub message: String,
    pub location: String,
    pub module: String,
    pub acknowledged: i32,
    pub created_at: i32,
}

// Disabled Alert Types models for tracking which alerts are silenced
#[derive(Queryable, Selectable, Clone, Debug, Serialize)]
#[diesel(table_name = disabled_alert_types)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DisabledAlertType {
    pub id: i32,
    pub alert_type: String,
    pub disabled_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = disabled_alert_types)]
pub struct NewDisabledAlertType {
    pub alert_type: String,
    pub disabled_at: i32,
}

// Site Metrics models for tracking site-wide statistics
#[derive(Queryable, Selectable, Clone, Debug, Serialize)]
#[diesel(table_name = site_metrics)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SiteMetric {
    pub id: i32,
    pub metric_key: String,
    pub metric_value: String,
    pub updated_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = site_metrics)]
pub struct NewSiteMetric {
    pub metric_key: String,
    pub metric_value: String,
    pub updated_at: i32,
}
