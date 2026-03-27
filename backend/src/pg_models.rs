//! PostgreSQL-backed models for sensitive/user-content data.
//!
//! All structs use `check_for_backend(diesel::pg::Pg)` and reference `pg_schema::`.
//! If any code tries to use these with a SQLite connection, it won't compile.

use crate::pg_schema::{
    admin_alerts, bridge_bandwidth_logs, bridge_disconnection_events, bridges,
    country_availability, disabled_alert_types, imap_connection, llm_usage_logs, mcp_servers,
    message_history, message_status_log, processed_emails, refund_info, site_metrics, tesla,
    totp_backup_codes, totp_secrets, usage_logs, user_info, user_secrets, waitlist,
    webauthn_challenges, webauthn_credentials, youtube,
};
use diesel::prelude::*;
use serde::Serialize;

// -- user_secrets (NEW table, consolidates matrix/twilio secrets) --

#[derive(Queryable, Selectable, Insertable, Clone, Debug)]
#[diesel(table_name = user_secrets)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserSecrets {
    pub id: i32,
    pub user_id: i32,
    pub matrix_username: Option<String>,
    pub matrix_device_id: Option<String>,
    pub encrypted_matrix_access_token: Option<String>,
    pub encrypted_matrix_password: Option<String>,
    pub encrypted_matrix_secret_storage_recovery_key: Option<String>,
    pub encrypted_twilio_account_sid: Option<String>,
    pub encrypted_twilio_auth_token: Option<String>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = user_secrets)]
pub struct NewUserSecrets {
    pub user_id: i32,
    pub matrix_username: Option<String>,
    pub matrix_device_id: Option<String>,
    pub encrypted_matrix_access_token: Option<String>,
    pub encrypted_matrix_password: Option<String>,
    pub encrypted_matrix_secret_storage_recovery_key: Option<String>,
    pub encrypted_twilio_account_sid: Option<String>,
    pub encrypted_twilio_auth_token: Option<String>,
}

// -- user_info --

#[derive(Queryable, Selectable, Insertable, Clone)]
#[diesel(table_name = user_info)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgUserInfo {
    pub id: i32,
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
pub struct NewPgUserInfo {
    pub user_id: i32,
    pub location: Option<String>,
    pub info: Option<String>,
    pub timezone: Option<String>,
    pub nearby_places: Option<String>,
}

// -- imap_connection --

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = imap_connection)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgImapConnection {
    pub id: i32,
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
pub struct NewPgImapConnection {
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

// -- message_history --

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = message_history)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgMessageHistory {
    pub id: i32,
    pub user_id: i32,
    pub role: String,
    pub encrypted_content: String,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub created_at: i32,
    pub conversation_id: String,
    pub tool_calls_json: Option<String>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = message_history)]
pub struct NewPgMessageHistory {
    pub user_id: i32,
    pub role: String,
    pub encrypted_content: String,
    pub tool_name: Option<String>,
    pub tool_call_id: Option<String>,
    pub created_at: i32,
    pub conversation_id: String,
    pub tool_calls_json: Option<String>,
}

// -- tesla --

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = tesla)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgTesla {
    pub id: i32,
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
    pub granted_scopes: Option<String>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = tesla)]
pub struct NewPgTesla {
    pub user_id: i32,
    pub encrypted_access_token: String,
    pub encrypted_refresh_token: String,
    pub status: String,
    pub last_update: i32,
    pub created_on: i32,
    pub expires_in: i32,
    pub region: String,
    pub granted_scopes: Option<String>,
}

// -- youtube --

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = youtube)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgYouTube {
    pub id: i32,
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
pub struct NewPgYouTube {
    pub user_id: i32,
    pub encrypted_access_token: String,
    pub encrypted_refresh_token: String,
    pub status: String,
    pub expires_in: i32,
    pub last_update: i32,
    pub created_on: i32,
    pub description: String,
}

// -- mcp_servers --

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = mcp_servers)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgMcpServer {
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    pub url_encrypted: String,
    pub auth_token_encrypted: Option<String>,
    pub is_enabled: i32,
    pub created_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = mcp_servers)]
pub struct NewPgMcpServer {
    pub user_id: i32,
    pub name: String,
    pub url_encrypted: String,
    pub auth_token_encrypted: Option<String>,
    pub is_enabled: i32,
    pub created_at: i32,
}

// -- totp_secrets --

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = totp_secrets)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgTotpSecret {
    pub encrypted_secret: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = totp_secrets)]
pub struct NewPgTotpSecret {
    pub user_id: i32,
    pub encrypted_secret: String,
    pub enabled: i32,
    pub created_at: i32,
}

// -- totp_backup_codes --

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = totp_backup_codes)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgTotpBackupCode {
    pub id: i32,
    pub code_hash: String,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = totp_backup_codes)]
pub struct NewPgTotpBackupCode {
    pub user_id: i32,
    pub code_hash: String,
    pub used: i32,
}

// -- webauthn_credentials --

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = webauthn_credentials)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgWebauthnCredential {
    pub credential_id: String,
    pub encrypted_public_key: String,
    pub device_name: String,
    pub created_at: i32,
    pub last_used_at: Option<i32>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = webauthn_credentials)]
pub struct NewPgWebauthnCredential {
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

// -- webauthn_challenges --

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = webauthn_challenges)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgWebauthnChallenge {
    pub challenge: String,
    pub context: Option<String>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = webauthn_challenges)]
pub struct NewPgWebauthnChallenge {
    pub user_id: i32,
    pub challenge: String,
    pub challenge_type: String,
    pub context: Option<String>,
    pub created_at: i32,
    pub expires_at: i32,
}

// -- bridges --

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = bridges)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgBridge {
    pub id: i32,
    pub user_id: i32,
    pub bridge_type: String,
    pub status: String,
    pub room_id: Option<String>,
    pub data: Option<String>,
    pub created_at: Option<i32>,
    pub last_seen_online: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = bridges)]
pub struct NewPgBridge {
    pub user_id: i32,
    pub bridge_type: String,
    pub status: String,
    pub room_id: Option<String>,
    pub data: Option<String>,
    pub created_at: Option<i32>,
}

// -- bridge_disconnection_events --

#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = bridge_disconnection_events)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgBridgeDisconnectionEvent {
    pub id: i32,
    pub user_id: i32,
    pub bridge_type: String,
    pub detected_at: i32,
}

#[derive(Insertable)]
#[diesel(table_name = bridge_disconnection_events)]
pub struct NewPgBridgeDisconnectionEvent {
    pub user_id: i32,
    pub bridge_type: String,
    pub detected_at: i32,
}

// -- usage_logs --

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = usage_logs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgUsageLog {
    pub id: i32,
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
    pub call_duration: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = usage_logs)]
pub struct NewPgUsageLog {
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

// -- processed_emails --

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = processed_emails)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgProcessedEmail {
    pub id: i32,
    pub user_id: i32,
    pub email_uid: String,
    pub processed_at: i32,
}

#[derive(Insertable)]
#[diesel(table_name = processed_emails)]
pub struct NewPgProcessedEmail {
    pub user_id: i32,
    pub email_uid: String,
    pub processed_at: i32,
}

// -- refund_info --

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = refund_info)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgRefundInfo {
    pub id: i32,
    pub user_id: i32,
    pub has_refunded: i32,
    pub last_credit_pack_amount: Option<f32>,
    pub last_credit_pack_purchase_timestamp: Option<i32>,
    pub refunded_at: Option<i32>,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = refund_info)]
pub struct NewPgRefundInfo {
    pub user_id: i32,
    pub has_refunded: i32,
}

// -- country_availability --

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = country_availability)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgCountryAvailability {
    pub id: i32,
    pub country_code: String,
    pub has_local_numbers: bool,
    pub outbound_sms_price: Option<f32>,
    pub inbound_sms_price: Option<f32>,
    pub outbound_voice_price_per_min: Option<f32>,
    pub inbound_voice_price_per_min: Option<f32>,
    pub last_checked: i32,
    pub created_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = country_availability)]
pub struct NewPgCountryAvailability {
    pub country_code: String,
    pub has_local_numbers: bool,
    pub outbound_sms_price: Option<f32>,
    pub inbound_sms_price: Option<f32>,
    pub outbound_voice_price_per_min: Option<f32>,
    pub inbound_voice_price_per_min: Option<f32>,
    pub last_checked: i32,
    pub created_at: i32,
}

// -- message_status_log --

#[derive(Queryable, Selectable, Clone, Debug, Serialize)]
#[diesel(table_name = message_status_log)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgMessageStatusLog {
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
pub struct NewPgMessageStatusLog {
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

// -- admin_alerts --

#[derive(Queryable, Selectable, Clone, Debug, Serialize)]
#[diesel(table_name = admin_alerts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgAdminAlert {
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
pub struct NewPgAdminAlert {
    pub alert_type: String,
    pub severity: String,
    pub message: String,
    pub location: String,
    pub module: String,
    pub acknowledged: i32,
    pub created_at: i32,
}

// -- disabled_alert_types --

#[derive(Queryable, Selectable, Clone, Debug, Serialize)]
#[diesel(table_name = disabled_alert_types)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgDisabledAlertType {
    pub id: i32,
    pub alert_type: String,
    pub disabled_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = disabled_alert_types)]
pub struct NewPgDisabledAlertType {
    pub alert_type: String,
    pub disabled_at: i32,
}

// -- site_metrics --

#[derive(Queryable, Selectable, Clone, Debug, Serialize)]
#[diesel(table_name = site_metrics)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgSiteMetric {
    pub id: i32,
    pub metric_key: String,
    pub metric_value: String,
    pub updated_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = site_metrics)]
pub struct NewPgSiteMetric {
    pub metric_key: String,
    pub metric_value: String,
    pub updated_at: i32,
}

// -- waitlist --

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = waitlist)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgWaitlistEntry {
    pub id: i32,
    pub email: String,
    pub created_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = waitlist)]
pub struct NewPgWaitlistEntry {
    pub email: String,
    pub created_at: i32,
}

// -- llm_usage_logs --

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = llm_usage_logs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PgLlmUsageLog {
    pub id: i32,
    pub user_id: i32,
    pub provider: String,
    pub model: String,
    pub callsite: String,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub created_at: i32,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = llm_usage_logs)]
pub struct NewPgLlmUsageLog {
    pub user_id: i32,
    pub provider: String,
    pub model: String,
    pub callsite: String,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub created_at: i32,
}

#[derive(Insertable)]
#[diesel(table_name = bridge_bandwidth_logs)]
pub struct NewBridgeBandwidthLog {
    pub user_id: i32,
    pub bridge_type: String,
    pub direction: String,
    pub bytes_estimate: i32,
    pub created_at: i32,
}
