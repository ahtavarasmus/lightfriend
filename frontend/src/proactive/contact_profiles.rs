use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ProfileException {
    pub platform: String,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ContactProfile {
    pub id: i32,
    pub nickname: String,
    pub whatsapp_chat: Option<String>,
    pub telegram_chat: Option<String>,
    pub signal_chat: Option<String>,
    pub email_addresses: Option<String>,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: bool,
    #[serde(default)]
    pub exceptions: Vec<ProfileException>,
    #[serde(default)]
    pub whatsapp_room_id: Option<String>,
    #[serde(default)]
    pub telegram_room_id: Option<String>,
    #[serde(default)]
    pub signal_room_id: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContactProfilesResponse {
    pub profiles: Vec<ContactProfile>,
    pub default_mode: String,
    #[serde(default = "default_noti_type")]
    pub default_noti_type: String,
    #[serde(default = "default_notify_call")]
    pub default_notify_on_call: bool,
    #[serde(default = "default_phone_contact_mode")]
    pub phone_contact_mode: String,
    #[serde(default = "default_phone_contact_noti_type")]
    pub phone_contact_noti_type: String,
    #[serde(default = "default_phone_contact_notify_call")]
    pub phone_contact_notify_on_call: bool,
}

fn default_noti_type() -> String {
    "sms".to_string()
}

fn default_notify_call() -> bool {
    true
}

fn default_phone_contact_mode() -> String {
    "critical".to_string()
}

fn default_phone_contact_noti_type() -> String {
    "sms".to_string()
}

fn default_phone_contact_notify_call() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Room {
    pub display_name: String,
    pub last_activity_formatted: String,
    #[serde(default)]
    pub room_id: String,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub attached_to: Option<String>,
    #[serde(default)]
    pub is_phone_contact: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchResponse {
    pub results: Vec<Room>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExceptionRequest {
    pub platform: String,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateProfileRequest {
    pub nickname: String,
    pub whatsapp_chat: Option<String>,
    pub telegram_chat: Option<String>,
    pub signal_chat: Option<String>,
    pub email_addresses: Option<String>,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exceptions: Option<Vec<ExceptionRequest>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whatsapp_room_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telegram_room_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_room_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateDefaultModeRequest {
    pub mode: Option<String>,
    pub noti_type: Option<String>,
    pub notify_on_call: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePhoneContactModeRequest {
    pub mode: Option<String>,
    pub noti_type: Option<String>,
    pub notify_on_call: Option<bool>,
}
