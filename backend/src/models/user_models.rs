
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
use crate::schema::temp_variables;
use crate::schema::message_history;



#[derive(Queryable, Selectable, Insertable, Clone)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct User {
    pub id: i32,
    pub email: String,
    pub password_hash: String,
    pub phone_number: String,
    pub nickname: Option<String>, // what user wants the ai to call them
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
    pub sub_tier: Option<String>, // tier 1(basic), tier 1.5(oracle), tier 2(sentinel), tier 3(self hosted)
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
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = temp_variables)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct TempVariable {
    pub id: i32,
    pub user_id: i32,
    pub confirm_send_event_type: String, // 'whatsapp', 'calendar' or 'email'
    pub confirm_send_event_recipient: Option<String>, 
    pub confirm_send_event_subject: Option<String>,
    pub confirm_send_event_content: Option<String>,
    pub confirm_send_event_start_time: Option<String>,
    pub confirm_send_event_duration: Option<String>,
    pub confirm_send_event_id: Option<String>,
    pub confirm_send_event_image_url: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = temp_variables)]
pub struct NewTempVariable {
    pub user_id: i32,
    pub confirm_send_event_type: String,
    pub confirm_send_event_recipient: Option<String>,
    pub confirm_send_event_subject: Option<String>,
    pub confirm_send_event_content: Option<String>,
    pub confirm_send_event_start_time: Option<String>,
    pub confirm_send_event_duration: Option<String>,
    pub confirm_send_event_id: Option<String>,
    pub confirm_send_event_image_url: Option<String>,
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


#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = bridges)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Bridge {
    pub id: Option<i32>, // Assuming auto-incrementing primary key
    pub user_id: i32,
    pub bridge_type: String, // whatsapp, telegram
    pub status: String, // connected, disconnected
    pub room_id: Option<String>,
    pub data: Option<String>,
    pub created_at: Option<i32>,
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


#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = waiting_checks)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct WaitingCheck {
    pub id: Option<i32>,
    pub user_id: i32,
    pub content: String,
    pub service_type: String,// like email, whatsapp, .. 
}

#[derive(Insertable)]
#[diesel(table_name = waiting_checks)]
pub struct NewWaitingCheck {
    pub user_id: i32,
    pub content: String,
    pub service_type: String,// like email, whatsapp, .. 
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = priority_senders)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PrioritySender {
    pub id: Option<i32>,
    pub user_id: i32,
    pub sender: String,
    pub service_type: String, // like email, whatsapp, .. 
}

#[derive(Insertable)]
#[diesel(table_name = priority_senders)]
pub struct NewPrioritySender {
    pub user_id: i32,
    pub sender: String,
    pub service_type: String, // like email, whatsapp, .. 
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
    pub timezone: Option<String>,
    pub timezone_auto: Option<bool>,
    pub agent_language: String, // language the agent will use to answer, default 'en'. 
    pub sub_country: Option<String>, // "US", "FI", "UK", "AU"
    pub save_context: Option<i32>, // how many messages to save in context or None if nothing is saved
    pub info: Option<String>, // extra info about the user for the ai
    pub morning_digest: Option<String>, // whether and when to send user morning digest noti, time is in UTC as rfc
    pub day_digest: Option<String>, // whether and when to send day digest, time is in UTC as rfc
    pub evening_digest: Option<String>, // whether and when to send user evening digest noti, time is in UTC rfc
    pub number_of_digests_locked: i32, // if user wants to change some of the digests for base messages we can lock some digests
    pub require_confirmation: bool, // whether to ask confirmation before sending a message or creating a calendar event
    pub critical_enabled: Option<String>, // whether to inform users about their critical messages immediately and by which way ("sms" or "call")
    pub encrypted_twilio_account_sid: Option<String>, // for self hosted instance
    pub encrypted_twilio_auth_token: Option<String>, // for self hosted instance
    pub encrypted_openrouter_api_key: Option<String>, // for self hosted instance
    pub server_url: Option<String>, // for self hosted instance
    pub encrypted_geoapify_key: Option<String>, // for self hosted instance
    pub encrypted_pirate_weather_key: Option<String>, // for self hosted instance
    pub server_instance_id: Option<String>, // for self hosted instance (field for pairing code and instance id)
    pub server_instance_last_ping_timestamp: Option<i32>, // for self hosted instance (used to allow only a single instance ping per day)
    pub server_ip: Option<String>, // for self hosted instance
    pub encrypted_textbee_device_id: Option<String>,
    pub encrypted_textbee_api_key: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = user_settings)]
pub struct NewUserSettings {
    pub user_id: i32,
    pub notify: bool,
    pub notification_type: Option<String>,
    pub timezone: Option<String>,
    pub timezone_auto: Option<bool>,
    pub agent_language: String,
    pub sub_country: Option<String>,
    pub save_context: Option<i32>,
    pub info: Option<String>,
    pub number_of_digests_locked: i32,
    pub critical_enabled: Option<String>,
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
}
