use crate::schema::bridge_disconnection_events;
use crate::schema::bridges;
use crate::schema::calendar_notifications;
use crate::schema::contact_profile_exceptions;
use crate::schema::contact_profiles;
use crate::schema::conversations;
use crate::schema::country_availability;
use crate::schema::email_judgments;
use crate::schema::google_calendar;
use crate::schema::imap_connection;
use crate::schema::keywords;
use crate::schema::message_history;
use crate::schema::message_status_log;
use crate::schema::priority_senders;
use crate::schema::processed_emails;
use crate::schema::subaccounts;
use crate::schema::tasks;
use crate::schema::totp_backup_codes;
use crate::schema::totp_secrets;
use crate::schema::uber;
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
    pub free_reply: bool, // flag that gets set when previous message needs more information to finish the reply
    pub confirm_send_event: Option<String>, // flag for if sending event needs confirmation. can be "whatsapp", "email" or "calendar"
    pub waiting_checks_count: i32, // how many waiting checks the user currently has(max 5 is possible)
    pub next_billing_date_timestamp: Option<i32>, // when is user next billed for their subscription
    pub magic_token: Option<String>, // token for magic link login/password setup
    pub plan_type: Option<String>, // "monitor" or "digest" for euro plan users, NULL for US/CA
}

#[derive(Queryable, Selectable, Insertable, Clone)]
#[diesel(table_name = user_info)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UserInfo {
    pub id: Option<i32>,
    pub user_id: i32,
    pub location: Option<String>,
    pub dictionary: Option<String>,
    pub info: Option<String>,
    pub timezone: Option<String>,
    pub nearby_places: Option<String>,
    pub recent_contacts: Option<String>,
    pub blocker_password_vault: Option<String>,
    pub lockbox_password_vault: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = user_info)]
pub struct NewUserInfo {
    pub user_id: i32,
    pub location: Option<String>,
    pub dictionary: Option<String>,
    pub info: Option<String>,
    pub timezone: Option<String>,
    pub nearby_places: Option<String>,
    pub recent_contacts: Option<String>,
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
    pub expires_in: i32,
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
    pub expires_in: i32,
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

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = email_judgments)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct EmailJudgment {
    pub id: Option<i32>,
    pub user_id: i32,
    pub email_timestamp: i32,
    pub processed_at: i32,
    pub should_notify: bool,
    pub score: i32,
    pub reason: String,
}

#[derive(Insertable)]
#[diesel(table_name = email_judgments)]
pub struct NewEmailJudgment {
    pub user_id: i32,
    pub email_timestamp: i32,
    pub processed_at: i32,
    pub should_notify: bool,
    pub score: i32,
    pub reason: String,
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
    pub activity_type: String, // sms, call, calendar_notification, email_priority, email_waiting_check, email_critical, whatsapp_critical, whatsapp_priority, whatsapp_waiting_check
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

#[derive(Queryable, Selectable, Insertable, Clone)]
#[diesel(table_name = conversations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Conversation {
    pub id: i32,
    pub user_id: i32,
    pub conversation_sid: String, // twilio conversation sid
    pub service_sid: String,      // twilio service sid where all the conversations fall under
    pub created_at: i32,          // epoch timestamp
    pub active: bool,             // should default to active for now
    pub twilio_number: String,    // where user was texting in this conversation
    pub user_number: String,      // user's number for this conversation
}

#[derive(Insertable)]
#[diesel(table_name = conversations)]
pub struct NewConversation {
    pub user_id: i32,
    pub conversation_sid: String,
    pub service_sid: String,
    pub created_at: i32,
    pub active: bool,
    pub twilio_number: String,
    pub user_number: String,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = uber)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Uber {
    pub id: Option<i32>,
    pub user_id: i32,
    pub encrypted_access_token: String,
    pub encrypted_refresh_token: String,
    pub status: String,
    pub last_update: i32,
    pub created_on: i32,
    pub description: String,
    pub expires_in: i32, // for access token
}

#[derive(Insertable)]
#[diesel(table_name = uber)]
pub struct NewUber {
    pub user_id: i32,
    pub encrypted_access_token: String,
    pub encrypted_refresh_token: String,
    pub status: String,
    pub last_update: i32,
    pub created_on: i32,
    pub description: String,
    pub expires_in: i32,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = google_calendar)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct GoogleCalendar {
    pub id: Option<i32>,
    pub user_id: i32,
    pub encrypted_access_token: String,
    pub encrypted_refresh_token: String,
    pub status: String,
    pub last_update: i32,
    pub created_on: i32,
    pub description: String,
    pub expires_in: i32, // for access token
}

#[derive(Insertable)]
#[diesel(table_name = google_calendar)]
pub struct NewGoogleCalendar {
    pub user_id: i32,
    pub encrypted_access_token: String,
    pub encrypted_refresh_token: String,
    pub status: String,
    pub last_update: i32,
    pub created_on: i32,
    pub description: String,
    pub expires_in: i32,
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

#[derive(Debug, Queryable, Insertable)]
#[diesel(table_name = calendar_notifications)]
pub struct CalendarNotification {
    pub id: Option<i32>,
    pub user_id: i32,
    pub event_id: String,
    pub notification_time: i32,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = calendar_notifications)]
pub struct NewCalendarNotification {
    pub user_id: i32,
    pub event_id: String,
    pub notification_time: i32,
}

/// Unified task model for scheduled and recurring tasks
/// trigger: "once_<timestamp>" or "recurring_email" or "recurring_messaging"
/// condition: optional natural language condition checked at runtime
/// action: natural language description of what to do
#[derive(Queryable, Selectable, Insertable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = tasks)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Task {
    pub id: Option<i32>,
    pub user_id: i32,
    pub trigger: String, // "once_<timestamp>", "recurring_email", "recurring_messaging"
    pub condition: Option<String>, // natural language condition (optional)
    pub action: String,  // natural language action (required)
    pub notification_type: Option<String>, // "sms" or "call"
    pub status: Option<String>, // "active", "completed", "cancelled"
    pub created_at: i32,
    pub completed_at: Option<i32>,
    pub is_permanent: Option<i32>, // 0 or 1, set via dashboard only
    pub recurrence_rule: Option<String>, // "daily", "weekly:1,3,5", "monthly:15"
    pub recurrence_time: Option<String>, // "09:00" (HH:MM in user timezone)
}

#[derive(Insertable)]
#[diesel(table_name = tasks)]
pub struct NewTask {
    pub user_id: i32,
    pub trigger: String,
    pub condition: Option<String>,
    pub action: String,
    pub notification_type: Option<String>,
    pub status: String,
    pub created_at: i32,
}

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = priority_senders)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PrioritySender {
    pub id: Option<i32>,
    pub user_id: i32,
    pub sender: String,
    pub service_type: String,      // like email, whatsapp, ..
    pub noti_type: Option<String>, // "sms", "call"
    pub noti_mode: String, // "all" to notify about every msg, "focus" to pay extra attention on digests and eligible for family on different monitoring settings
}

#[derive(Insertable)]
#[diesel(table_name = priority_senders)]
pub struct NewPrioritySender {
    pub user_id: i32,
    pub sender: String,
    pub service_type: String,
    pub noti_type: Option<String>,
    pub noti_mode: String,
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

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = keywords)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Keyword {
    pub id: Option<i32>,
    pub user_id: i32,
    pub keyword: String,
    pub service_type: String, // like email, whatsapp, ..
}

#[derive(Insertable)]
#[diesel(table_name = keywords)]
pub struct NewKeyword {
    pub user_id: i32,
    pub keyword: String,
    pub service_type: String, // like email, whatsapp, ..
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
    pub number_of_digests_locked: i32, // if user wants to change some of the digests for base messages we can lock some digests
    pub critical_enabled: Option<String>, // whether to inform users about their critical messages immediately and by which way ("sms" or "call")
    pub encrypted_twilio_account_sid: Option<String>, // for self hosted instance
    pub encrypted_twilio_auth_token: Option<String>, // for self hosted instance
    pub encrypted_openrouter_api_key: Option<String>, // for self hosted instance
    pub server_url: Option<String>,       // for self hosted instance
    pub encrypted_geoapify_key: Option<String>, // for self hosted instance
    pub encrypted_pirate_weather_key: Option<String>, // for self hosted instance
    pub server_ip: Option<String>,        // for self hosted instance
    pub encrypted_textbee_device_id: Option<String>,
    pub encrypted_textbee_api_key: Option<String>,
    pub elevenlabs_phone_number_id: Option<String>, // used to make outbound calls(we get this from elevenlabs api call when adding the phone number)
    pub proactive_agent_on: bool, // whether the user wants to receive any kinds of notifications
    pub notify_about_calls: bool, // if call comes in to any chat networks should we notify the user about it?
    pub action_on_critical_message: Option<String>, // "notify_family" or None (notify all). Only applies to messaging platforms (WhatsApp, Telegram, Signal), not email.
    pub magic_login_token: Option<String>,          // guest checkout magic link token
    pub magic_login_token_expiration_timestamp: Option<i32>, // guest checkout magic token expiration timestamp
    pub monthly_message_count: i32,                          // monthly message count tracking
    pub outbound_message_pricing: Option<f32>, // cached Twilio outbound SMS price for user's country
    pub last_instant_digest_time: Option<i32>, // timestamp of last on-demand digest fetch
    pub phone_service_active: bool, // whether phone service (SMS and calls) is active - can be disabled for security (e.g., stolen phone)
    pub default_notification_mode: Option<String>, // "critical", "digest", or "ignore" - default behavior for unknown senders
    pub default_notification_type: Option<String>, // "sms" or "call" - default notification type for unknown senders
    pub default_notify_on_call: i32,               // 1 = notify on incoming calls, 0 = don't notify
    pub llm_provider: Option<String>, // "openai" (default) or "tinfoil" - which LLM provider to use for SMS/chat
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
    pub number_of_digests_locked: i32,
    pub critical_enabled: Option<String>,
    pub proactive_agent_on: bool,
    pub notify_about_calls: bool,
}

#[derive(Queryable, Insertable)]
#[diesel(table_name = subaccounts)]
pub struct Subaccount {
    pub id: i32,
    pub user_id: String,
    pub subaccount_sid: String,
    pub auth_token: String,
    pub country: Option<String>,
    pub number: Option<String>,
    pub cost_this_month: Option<f32>,
    pub created_at: Option<i32>,
    pub status: Option<String>,
    pub tinfoil_key: Option<String>,
    pub messaging_service_sid: Option<String>,
    pub subaccount_type: String, // "us_ca", "full_service", "notification_only"
    pub country_code: Option<String>, // ISO country code (US, CA, FI, IL, etc.)
}

#[derive(Insertable)]
#[diesel(table_name = subaccounts)]
pub struct NewSubaccount {
    pub user_id: String,
    pub subaccount_sid: String,
    pub auth_token: String,
    pub country: Option<String>,
    pub number: Option<String>,
    pub cost_this_month: Option<f32>,
    pub created_at: Option<i32>,
    pub status: Option<String>,
    pub tinfoil_key: Option<String>,
    pub messaging_service_sid: Option<String>,
    pub subaccount_type: String, // "us_ca", "full_service", "notification_only"
    pub country_code: Option<String>, // ISO country code
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
