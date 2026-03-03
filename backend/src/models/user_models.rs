use crate::schema::bridge_disconnection_events;
use crate::schema::bridges;
use crate::schema::contact_profile_exceptions;
use crate::schema::contact_profiles;
use crate::schema::country_availability;
use crate::schema::imap_connection;
use crate::schema::items;
use crate::schema::message_history;
use crate::schema::message_status_log;
use crate::schema::processed_emails;
use crate::schema::totp_backup_codes;
use crate::schema::totp_secrets;
use crate::schema::usage_logs;
use crate::schema::user_info;
use crate::schema::user_settings;
use crate::schema::users;
use crate::schema::waitlist;
use crate::schema::webauthn_challenges;
use crate::schema::webauthn_credentials;
use crate::schema::youtube;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Queryable, Selectable, Insertable, Clone)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct User {
    pub id: i32,
    pub email: String,
    pub password_hash: String,
    pub phone_number: String,
    pub nickname: Option<String>,  // what user wants the ai to call them
    pub time_to_live: Option<i32>, // if user has not verified their account in some time it will be deleted
    pub verified: bool,
    pub credits: f32, // user purchased credits, will not expire, can be bought if has subscribtion or is a early user(discount = true)
    pub preferred_number: Option<String>, // number the user prefers lightfriend texting/calling them from
    pub charge_when_under: bool, // flag for if user wants to automatically buy more overage credits
    pub charge_back_to: Option<f32>, // the credit amount to charge
    pub stripe_customer_id: Option<String>,
    pub stripe_payment_method_id: Option<String>,
    pub stripe_checkout_session_id: Option<String>,
    pub matrix_username: Option<String>,
    pub encrypted_matrix_access_token: Option<String>,
    pub sub_tier: Option<String>, // tier 2 only, differentiated by plan_type
    pub matrix_device_id: Option<String>,
    pub credits_left: f32, // free credits that reset every month while in the monthly sub. will always be consumed before one time credits
    pub encrypted_matrix_password: Option<String>,
    pub encrypted_matrix_secret_storage_recovery_key: Option<String>,
    pub last_credits_notification: Option<i32>, // Unix timestamp of last insufficient credits notification to prevent spam
    pub discount: bool, // if user can get buy overage credits without subscription(for early adopters)
    pub discount_tier: Option<String>, // could be None, "msg", "voice" or "full"
    pub next_billing_date_timestamp: Option<i32>, // when is user next billed for their subscription
    pub magic_token: Option<String>, // token for magic link login/password setup
    pub plan_type: Option<String>, // "assistant", "autopilot", or "byot"
    pub matrix_e2ee_enabled: bool, // whether E2EE is enabled for Matrix messaging
    pub migrated_to_new_server: bool, // whether user has migrated to new AWS server
    pub last_backup_at: Option<i32>, // Unix timestamp of last backup
    pub backup_session_active: bool, // whether a backup session is currently active
}

#[derive(Queryable, Selectable, Insertable, Clone)]
#[diesel(table_name = user_info)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UserInfo {
    pub id: Option<i32>,
    pub user_id: i32,
    pub location: Option<String>,
    pub info: Option<String>,
    pub timezone: Option<String>,
    pub nearby_places: Option<String>,
    pub latitude: Option<f32>,
    pub longitude: Option<f32>,
}

#[derive(Insertable)]
#[diesel(table_name = user_info)]
pub struct NewUserInfo {
    pub user_id: i32,
    pub location: Option<String>,
    pub info: Option<String>,
    pub timezone: Option<String>,
    pub nearby_places: Option<String>,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = imap_connection)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ImapConnection {
    pub id: Option<i32>,
    pub user_id: i32,
    pub method: String,
    pub encrypted_password: String,
    pub status: String,
    pub last_update: i32,
    pub created_on: i32,
    pub description: String,
    pub imap_server: Option<String>,
    pub imap_port: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = imap_connection)]
pub struct NewImapConnection {
    pub user_id: i32,
    pub method: String,
    pub encrypted_password: String,
    pub status: String,
    pub last_update: i32,
    pub created_on: i32,
    pub description: String,
    pub imap_server: Option<String>,
    pub imap_port: Option<i32>,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = processed_emails)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ProcessedEmail {
    pub id: Option<i32>,
    pub user_id: i32,
    pub email_uid: String,
    pub processed_at: i32,
}

#[derive(Insertable)]
#[diesel(table_name = processed_emails)]
pub struct NewProcessedEmail {
    pub user_id: i32,
    pub email_uid: String,
    pub processed_at: i32,
}

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = bridges)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Bridge {
    pub id: Option<i32>, // Assuming auto-incrementing primary key
    pub user_id: i32,
    pub bridge_type: String, // whatsapp, telegram
    pub status: String,      // connected, disconnected
    pub room_id: Option<String>,
    pub data: Option<String>,
    pub created_at: Option<i32>,
    pub last_seen_online: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = bridges)]
pub struct NewBridge {
    pub user_id: i32,
    pub bridge_type: String, // whatsapp, telegram
    pub status: String,      // connected, disconnected
    pub room_id: Option<String>,
    pub data: Option<String>,
    pub created_at: Option<i32>,
}

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = bridge_disconnection_events)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct BridgeDisconnectionEvent {
    pub id: Option<i32>,
    pub user_id: i32,
    pub bridge_type: String,
    pub detected_at: i32,
}

#[derive(Insertable)]
#[diesel(table_name = bridge_disconnection_events)]
pub struct NewBridgeDisconnectionEvent {
    pub user_id: i32,
    pub bridge_type: String,
    pub detected_at: i32,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = usage_logs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UsageLog {
    pub id: Option<i32>,
    pub user_id: i32,
    pub sid: Option<String>,        // elevenlabs call id or twilio message id
    pub activity_type: String, // sms, call, email_priority, email_critical, whatsapp_critical, whatsapp_priority
    pub credits: Option<f32>,  // the amount of credits used in euros
    pub created_at: i32,       // int timestamp utc epoch
    pub time_consumed: Option<i32>, // messsage response time or call duration in seconds
    pub success: Option<bool>, // if call/message was successful judged by ai
    pub reason: Option<String>, // if call/message was not successful, store the reason why (no sensitive content)
    pub status: Option<String>, // call specific: 'ongoing' or 'done' OR message specific: 'charged' or 'correction'
    pub recharge_threshold_timestamp: Option<i32>, // call specific: timestamp when credits go below recharge threshold
    pub zero_credits_timestamp: Option<i32>, // call specific: timestamp when credits reach zero
    pub call_duration: Option<i32>,          // call specific: timestamp when credits reach zero
}

#[derive(Insertable)]
#[diesel(table_name = usage_logs)]
pub struct NewUsageLog {
    pub user_id: i32,
    pub sid: Option<String>,
    pub activity_type: String,
    pub credits: Option<f32>,
    pub created_at: i32,
    pub time_consumed: Option<i32>,
    pub success: Option<bool>,
    pub reason: Option<String>,
    pub status: Option<String>,
    pub recharge_threshold_timestamp: Option<i32>,
    pub zero_credits_timestamp: Option<i32>,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = youtube)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct YouTube {
    pub id: Option<i32>,
    pub user_id: i32,
    pub encrypted_access_token: String,
    pub encrypted_refresh_token: String,
    pub status: String,
    pub expires_in: i32,
    pub last_update: i32,
    pub created_on: i32,
    pub description: String,
}

#[derive(Insertable)]
#[diesel(table_name = youtube)]
pub struct NewYouTube {
    pub user_id: i32,
    pub encrypted_access_token: String,
    pub encrypted_refresh_token: String,
    pub status: String,
    pub expires_in: i32,
    pub last_update: i32,
    pub created_on: i32,
    pub description: String,
}

// Contact Profiles - unified notification settings per person/group
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = contact_profiles)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ContactProfile {
    pub id: Option<i32>,
    pub user_id: i32,
    pub nickname: String,
    pub whatsapp_chat: Option<String>,
    pub telegram_chat: Option<String>,
    pub signal_chat: Option<String>,
    pub email_addresses: Option<String>,
    pub notification_mode: String, // "all", "critical", "digest"
    pub notification_type: String, // "sms", "call"
    pub notify_on_call: i32,       // 1 = true, 0 = false
    pub created_at: i32,
    pub whatsapp_room_id: Option<String>,
    pub telegram_room_id: Option<String>,
    pub signal_room_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = contact_profiles)]
pub struct NewContactProfile {
    pub user_id: i32,
    pub nickname: String,
    pub whatsapp_chat: Option<String>,
    pub telegram_chat: Option<String>,
    pub signal_chat: Option<String>,
    pub email_addresses: Option<String>,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: i32,
    pub created_at: i32,
    pub whatsapp_room_id: Option<String>,
    pub telegram_room_id: Option<String>,
    pub signal_room_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = contact_profile_exceptions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ContactProfileException {
    pub platform: String,          // "whatsapp", "telegram", "signal", "email"
    pub notification_mode: String, // "all", "critical", "digest"
    pub notification_type: String, // "sms", "call"
    pub notify_on_call: i32,       // 1 = true, 0 = false
}

#[derive(Insertable)]
#[diesel(table_name = contact_profile_exceptions)]
pub struct NewContactProfileException {
    pub profile_id: i32,
    pub platform: String,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: i32,
}

#[derive(Queryable, Selectable, Insertable, Clone)]
#[diesel(table_name = user_settings)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UserSettings {
    pub id: Option<i32>,
    pub user_id: i32,
    pub notify: bool, // if user wants to be notified about new features to lightfriend (not related to notifications)
    pub notification_type: Option<String>, // "call" or "sms"(sms is default when none) // "call" also sends notification as sms
    pub timezone_auto: Option<bool>,
    pub agent_language: String, // language the agent will use to answer, default 'en'.
    pub sub_country: Option<String>, // "US", "FI", "UK", "AU"
    pub save_context: Option<i32>, // how many messages to save in context or None if nothing is saved
    pub morning_digest: Option<String>, // whether and when to send user morning digest noti, time is in UTC as rfc
    pub day_digest: Option<String>, // whether and when to send day digest, time is in UTC as rfc
    pub evening_digest: Option<String>, // whether and when to send user evening digest noti, time is in UTC rfc
    pub critical_enabled: Option<String>, // whether to inform users about their critical messages immediately and by which way ("sms" or "call")
    pub encrypted_twilio_account_sid: Option<String>, // for self hosted instance
    pub encrypted_twilio_auth_token: Option<String>, // for self hosted instance
    pub encrypted_openrouter_api_key: Option<String>, // for self hosted instance
    pub encrypted_textbee_device_id: Option<String>,
    pub encrypted_textbee_api_key: Option<String>,
    pub elevenlabs_phone_number_id: Option<String>, // used to make outbound calls(we get this from elevenlabs api call when adding the phone number)
    pub proactive_agent_on: bool, // whether the user wants to receive any kinds of notifications
    pub notify_about_calls: bool, // if call comes in to any chat networks should we notify the user about it?
    pub action_on_critical_message: Option<String>, // "notify_family" or None (notify all). Only applies to messaging platforms (WhatsApp, Telegram, Signal), not email.
    pub last_instant_digest_time: Option<i32>,      // timestamp of last on-demand digest fetch
    pub phone_service_active: bool, // whether phone service (SMS and calls) is active - can be disabled for security (e.g., stolen phone)
    pub default_notification_mode: Option<String>, // "critical", "digest", or "ignore" - default behavior for unknown senders
    pub default_notification_type: Option<String>, // "sms" or "call" - default notification type for unknown senders
    pub default_notify_on_call: i32,               // 1 = notify on incoming calls, 0 = don't notify
    pub llm_provider: Option<String>, // "openai" (default) or "tinfoil" - which LLM provider to use for SMS/chat
    pub quiet_mode_until: Option<i32>, // NULL = active, 0 = indefinite quiet, >0 = quiet until that unix timestamp
    pub phone_contact_notification_mode: Option<String>, // "critical", "digest", or "ignore" - for phone contacts without a profile
    pub phone_contact_notification_type: Option<String>, // "sms" or "call" - notification type for phone contacts
    pub phone_contact_notify_on_call: i32, // 1 = notify on incoming calls from phone contacts, 0 = don't
    pub auto_create_items: bool, // whether to auto-detect and create trackable items from emails/messages
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
    pub proactive_agent_on: bool,
    pub notify_about_calls: bool,
}

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = message_history)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct MessageHistory {
    pub id: Option<i32>,
    pub user_id: i32,
    pub role: String, // 'user', 'assistant', 'tool', or 'system'
    pub encrypted_content: String,
    pub tool_name: Option<String>, // Name of the tool if it's a tool response
    pub tool_call_id: Option<String>, // ID of the tool call if it's a tool response
    pub created_at: i32,           // Unix timestamp
    pub conversation_id: String,   // To group messages in the same conversation
    pub tool_calls_json: Option<String>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = message_history)]
pub struct NewMessageHistory {
    pub user_id: i32,
    pub role: String,
    pub encrypted_content: String,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub created_at: i32,
    pub conversation_id: String,
    pub tool_calls_json: Option<String>,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = country_availability)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
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

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = crate::schema::tesla)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Tesla {
    pub id: Option<i32>,
    pub user_id: i32,
    pub encrypted_access_token: String,
    pub encrypted_refresh_token: String,
    pub status: String,
    pub last_update: i32,
    pub created_on: i32,
    pub expires_in: i32,
    pub region: String,
    pub selected_vehicle_vin: Option<String>,
    pub selected_vehicle_name: Option<String>,
    pub selected_vehicle_id: Option<String>,
    pub virtual_key_paired: i32,
    pub granted_scopes: Option<String>, // Space-separated scopes granted by user
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::tesla)]
pub struct NewTesla {
    pub user_id: i32,
    pub encrypted_access_token: String,
    pub encrypted_refresh_token: String,
    pub status: String,
    pub last_update: i32,
    pub created_on: i32,
    pub expires_in: i32,
    pub region: String,
    pub granted_scopes: Option<String>, // Space-separated scopes granted by user
}

// TOTP 2FA models
#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = totp_secrets)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct TotpSecret {
    pub encrypted_secret: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = totp_secrets)]
pub struct NewTotpSecret {
    pub user_id: i32,
    pub encrypted_secret: String,
    pub enabled: i32,
    pub created_at: i32,
}

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = totp_backup_codes)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct TotpBackupCode {
    pub id: Option<i32>,
    pub code_hash: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = totp_backup_codes)]
pub struct NewTotpBackupCode {
    pub user_id: i32,
    pub code_hash: String,
    pub used: i32,
}

// WebAuthn models for passkeys (Touch ID, Face ID, etc.)
#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = webauthn_credentials)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct WebauthnCredential {
    pub credential_id: String,
    pub encrypted_public_key: String,
    pub device_name: String,
    pub created_at: i32,
    pub last_used_at: Option<i32>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = webauthn_credentials)]
pub struct NewWebauthnCredential {
    pub user_id: i32,
    pub credential_id: String,
    pub encrypted_public_key: String,
    pub device_name: String,
    pub counter: i32,
    pub transports: Option<String>,
    pub aaguid: Option<String>,
    pub created_at: i32,
    pub enabled: i32,
}

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = webauthn_challenges)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct WebauthnChallenge {
    pub challenge: String,
    pub context: Option<String>, // e.g., "login", "tesla_unlock"
}

#[derive(Insertable, Debug)]
#[diesel(table_name = webauthn_challenges)]
pub struct NewWebauthnChallenge {
    pub user_id: i32,
    pub challenge: String,
    pub challenge_type: String,
    pub context: Option<String>,
    pub created_at: i32,
    pub expires_at: i32,
}

// Waitlist models for users who want updates but haven't subscribed
#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = waitlist)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
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
#[diesel(table_name = crate::schema::refund_info)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct RefundInfo {
    pub has_refunded: i32, // 0 = no, 1 = yes (SQLite doesn't have bool)
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::refund_info)]
pub struct NewRefundInfo {
    pub user_id: i32,
    pub has_refunded: i32,
}

// Message Status Log models for tracking SMS delivery metadata (no message content)
#[derive(Queryable, Selectable, Clone, Debug, Serialize)]
#[diesel(table_name = message_status_log)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct MessageStatusLog {
    pub id: Option<i32>,
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
#[diesel(table_name = crate::schema::admin_alerts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct AdminAlert {
    pub id: Option<i32>,
    pub alert_type: String,
    pub severity: String,
    pub message: String,
    pub location: String,
    pub module: String,
    pub acknowledged: i32,
    pub created_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::admin_alerts)]
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
#[diesel(table_name = crate::schema::disabled_alert_types)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DisabledAlertType {
    pub id: Option<i32>,
    pub alert_type: String,
    pub disabled_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::disabled_alert_types)]
pub struct NewDisabledAlertType {
    pub alert_type: String,
    pub disabled_at: i32,
}

// Site Metrics models for tracking site-wide statistics
#[derive(Queryable, Selectable, Clone, Debug, Serialize)]
#[diesel(table_name = crate::schema::site_metrics)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct SiteMetric {
    pub id: Option<i32>,
    pub metric_key: String,
    pub metric_value: String,
    pub updated_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = crate::schema::site_metrics)]
pub struct NewSiteMetric {
    pub metric_key: String,
    pub metric_value: String,
    pub updated_at: i32,
}

// Unified items
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = items)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Item {
    pub id: Option<i32>,
    pub user_id: i32,
    pub summary: String,
    pub due_at: Option<i32>,
    pub priority: i32,
    pub source_id: Option<String>,
    pub created_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = items)]
pub struct NewItem {
    pub user_id: i32,
    pub summary: String,
    pub due_at: Option<i32>,
    pub priority: i32,
    pub source_id: Option<String>,
    pub created_at: i32,
}
