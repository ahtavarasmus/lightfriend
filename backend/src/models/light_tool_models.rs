use crate::pg_schema::{
    light_tool_devices, light_tool_pairing_sessions, light_tool_push_registrations, light_tool_runs,
};
use diesel::prelude::*;

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = light_tool_devices)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct LightToolDevice {
    pub id: i32,
    pub installation_id_hash: String,
    pub device_token_hash: String,
    pub user_id: Option<i32>,
    pub trial_started_at: i32,
    pub trial_expires_at: i32,
    pub trial_messages_used: i32,
    pub last_seen_at: i32,
    pub revoked_at: Option<i32>,
    pub created_at: i32,
    pub updated_at: i32,
}

#[derive(Insertable, Clone, Debug)]
#[diesel(table_name = light_tool_devices)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewLightToolDevice {
    pub installation_id_hash: String,
    pub device_token_hash: String,
    pub trial_started_at: i32,
    pub trial_expires_at: i32,
    pub last_seen_at: i32,
    pub created_at: i32,
    pub updated_at: i32,
}

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = light_tool_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct LightToolRun {
    pub id: String,
    pub device_id: i32,
    pub account_user_id: Option<i32>,
    pub client_message_id: String,
    pub encrypted_user_message: String,
    pub encrypted_activity_text: Option<String>,
    pub encrypted_assistant_message: Option<String>,
    pub encrypted_error_message: Option<String>,
    pub status: String,
    pub created_at: i32,
    pub updated_at: i32,
    pub completed_at: Option<i32>,
}

#[derive(Insertable, Clone, Debug)]
#[diesel(table_name = light_tool_runs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewLightToolRun {
    pub id: String,
    pub device_id: i32,
    pub account_user_id: Option<i32>,
    pub client_message_id: String,
    pub encrypted_user_message: String,
    pub created_at: i32,
    pub updated_at: i32,
}

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = light_tool_pairing_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct LightToolPairingSession {
    pub user_id: i32,
    pub token_hash: String,
    pub expires_at: i32,
    pub consumed_at: Option<i32>,
    pub consumed_by_device_id: Option<i32>,
    pub created_at: i32,
}

#[derive(Insertable, Clone, Debug)]
#[diesel(table_name = light_tool_pairing_sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewLightToolPairingSession {
    pub user_id: i32,
    pub token_hash: String,
    pub expires_at: i32,
    pub created_at: i32,
}

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = light_tool_push_registrations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct LightToolPushRegistration {
    pub device_id: i32,
    pub encrypted_endpoint: String,
    pub endpoint_hash: String,
    pub registered_at: i32,
    pub updated_at: i32,
}

#[derive(Insertable, Clone, Debug)]
#[diesel(table_name = light_tool_push_registrations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewLightToolPushRegistration {
    pub device_id: i32,
    pub encrypted_endpoint: String,
    pub endpoint_hash: String,
    pub registered_at: i32,
    pub updated_at: i32,
}
