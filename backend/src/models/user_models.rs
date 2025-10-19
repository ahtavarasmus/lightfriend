use diesel::prelude::*;
use crate::schema::users;
use crate::schema::conversations;
use crate::schema::waiting_checks;
use crate::schema::priority_senders;
use crate::schema::keywords;
use crate::schema::usage_logs;
use crate::schema::google_calendar;
use crate::schema::bridges;
use crate::schema::imap_connection;
use crate::schema::processed_emails;
use crate::schema::email_judgments;
use crate::schema::google_tasks;
use crate::schema::task_notifications;
use crate::schema::calendar_notifications;
use crate::schema::user_settings;
use crate::schema::message_history;
use crate::schema::user_info;
use crate::schema::uber;



#[derive(Queryable, Selectable, Insertable, Clone)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct User {
    pub id: i32,
    pub phone_number: String,
    pub nickname: Option<String>, // what user wants the ai to call them
    pub credits: f32, // user purchased credits, will not expire, can be bought if has subscribtion or is a early user(discount = true)
    pub preferred_number: Option<String>, // number the user prefers lightfriend texting/calling them from
    pub matrix_username: Option<String>,
    pub encrypted_matrix_access_token: Option<String>,
    pub matrix_device_id: Option<String>,
    pub credits_left: f32, // free credits that reset every month while in the monthly sub. will always be consumed before one time credits
    pub encrypted_matrix_password: Option<String>,
    pub phone_number_country: Option<String>, // "US", "CA", .. diff between us and ca phone numbers so we don't have to use api to look up each time
    pub twilio_account_sid: Option<String>,
    pub twilio_auth_token: Option<String>,
    pub server_url: Option<String>,
    pub twilio_messaging_service_sid: Option<String>,
    pub tinfoil_api_key: Option<String>,
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
#[diesel(table_name = task_notifications)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct TaskNotification {
    pub id: Option<i32>,
    pub user_id: i32,
    pub task_id: String, // google task id
    pub notified_at: i32, // due timestamp
}

#[derive(Insertable)]
#[diesel(table_name = task_notifications)]
pub struct NewTaskNotification {
    pub user_id: i32,
    pub task_id: String,
    pub notified_at: i32,
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
    pub id: Option<i32>, 
    pub user_id: i32,
    pub bridge_type: String, // whatsapp, telegram
    pub status: String, // connected, disconnected
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
    pub status: String, // connected, disconnected
    pub room_id: Option<String>,
    pub data: Option<String>,
    pub created_at: Option<i32>,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = usage_logs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UsageLog {
    pub id: Option<i32>,
    pub user_id: i32,
    pub sid: Option<String>, // elevenlabs call id or twilio message id
    pub activity_type: String, // sms, call, calendar_notification, email_priority, email_waiting_check, email_critical, whatsapp_critical, whatsapp_priority, whatsapp_waiting_check
    pub credits: Option<f32>, // the amount of credits used in euros
    pub created_at: i32, // int timestamp utc epoch
    pub time_consumed: Option<i32>, // messsage response time or call duration in seconds
    pub success: Option<bool>, // if call/message was successful judged by ai
    pub reason: Option<String>, // if call/message was not successful, store the reason why (no sensitive content)
    pub status: Option<String>, // call specific: 'ongoing' or 'done' OR message specific: 'charged' or 'correction'
    pub recharge_threshold_timestamp: Option<i32>, // call specific: timestamp when credits go below recharge threshold
    pub zero_credits_timestamp: Option<i32>, // call specific: timestamp when credits reach zero
    pub call_duration: Option<i32>, // call specific: timestamp when credits reach zero
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
    pub service_sid: String, // twilio service sid where all the conversations fall under
    pub created_at: i32, // epoch timestamp
    pub active: bool, // should default to active for now
    pub twilio_number: String, // where user was texting in this conversation
    pub user_number: String, // user's number for this conversation
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
#[diesel(table_name = google_tasks)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct GoogleTasks {
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
#[diesel(table_name = google_tasks)]
pub struct NewGoogleTasks{
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
pub struct GoogleCalendar{
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
pub struct NewGoogleCalendar{
    pub user_id: i32,
    pub encrypted_access_token: String,
    pub encrypted_refresh_token: String,
    pub status: String,
    pub last_update: i32,
    pub created_on: i32,
    pub description: String,
    pub expires_in: i32,
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


#[derive(Queryable, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = waiting_checks)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct WaitingCheck {
    pub id: Option<i32>,
    pub user_id: i32,
    pub content: String,
    pub service_type: String,// like email, whatsapp, .. 
    pub noti_type: Option<String>, // "sms", "call"
}

#[derive(Insertable)]
#[diesel(table_name = waiting_checks)]
pub struct NewWaitingCheck {
    pub user_id: i32,
    pub content: String,
    pub service_type: String,// like email, whatsapp, .. 
    pub noti_type: Option<String>,
}

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = priority_senders)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PrioritySender {
    pub id: Option<i32>,
    pub user_id: i32,
    pub sender: String,
    pub service_type: String, // like email, whatsapp, .. 
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
    pub notification_type: Option<String>, // "call" or "sms"(sms is default when none) // "call" also sends notification as sms
    pub timezone_auto: Option<bool>,
    pub agent_language: String, // language the agent will use to answer, default 'en'. 
    pub save_context: Option<i32>, // how many messages to save in context or None if nothing is saved
    pub morning_digest: Option<String>, // whether and when to send user morning digest noti, time is in UTC as rfc
    pub day_digest: Option<String>, // whether and when to send day digest, time is in UTC as rfc
    pub evening_digest: Option<String>, // whether and when to send user evening digest noti, time is in UTC rfc
    pub critical_enabled: Option<String>, // whether to inform users about their critical messages immediately and by which way ("sms" or "call")
    pub server_url: Option<String>, // for self hosted instance
    pub encrypted_textbee_device_id: Option<String>,
    pub encrypted_textbee_api_key: Option<String>,
    pub elevenlabs_phone_number_id: Option<String>, // used to make outbound calls(we get this from elevenlabs api call when adding the phone number)
    pub proactive_agent_on: bool, // whether the user wants to receive any kinds of notifications
    pub notify_about_calls: bool, // if call comes in to any chat networks should we notify the user about it?
    pub action_on_critical_message: Option<String>, // "ask_sender", "ask_sender_exclude_family", "notify_family", or None and "notify_all" are the same
}

#[derive(Insertable)]
#[diesel(table_name = user_settings)]
pub struct NewUserSettings {
    pub user_id: i32,
    pub notification_type: Option<String>,
    pub timezone_auto: Option<bool>,
    pub agent_language: String,
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
    pub role: String,  // 'user', 'assistant', 'tool', or 'system'
    pub encrypted_content: String,
    pub tool_name: Option<String>,  // Name of the tool if it's a tool response
    pub tool_call_id: Option<String>,  // ID of the tool call if it's a tool response
    pub created_at: i32,  // Unix timestamp
    pub conversation_id: String,  // To group messages in the same conversation
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
