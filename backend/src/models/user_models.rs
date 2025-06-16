
use diesel::prelude::*;
use crate::schema::users;
use crate::schema::conversations;
use crate::schema::waiting_checks;
use crate::schema::priority_senders;
use crate::schema::keywords;
use crate::schema::importance_priorities;
use crate::schema::usage_logs;
use crate::schema::subscriptions;
use crate::schema::unipile_connection;
use crate::schema::google_calendar;
use crate::schema::gmail;
use crate::schema::bridges;
use crate::schema::imap_connection;
use crate::schema::processed_emails;
use crate::schema::email_judgments;
use crate::schema::google_tasks;
use crate::schema::task_notifications;
use crate::schema::proactive_settings;
use crate::schema::calendar_notifications;
use crate::schema::user_settings;
use crate::schema::ideas;
use crate::schema::idea_upvotes;
use crate::schema::idea_email_subscriptions;



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
    pub info: Option<String>, // extra info about the user for the ai
    pub preferred_number: Option<String>, // number the user prefers lightfriend texting/calling them from
    pub charge_when_under: bool, // flag for if user wants to automatically buy more overage credits
    pub charge_back_to: Option<f32>, // the credit amount to charge 
    pub stripe_customer_id: Option<String>,
    pub stripe_payment_method_id: Option<String>,
    pub stripe_checkout_session_id: Option<String>,
    pub matrix_username: Option<String>,
    pub encrypted_matrix_access_token: Option<String>,
    pub sub_tier: Option<String>,
    pub msgs_left: i32, // proactive messages for the monthly sub, resets every month to bought amount
    pub matrix_device_id: Option<String>,
    pub credits_left: f32, // free credits that reset every month while in the monthly sub. will always be consumed before one time credits
    pub encrypted_matrix_password: Option<String>,
    pub encrypted_matrix_secret_storage_recovery_key: Option<String>,
    pub last_credits_notification: Option<i32>, // Unix timestamp of last insufficient credits notification to prevent spam
    pub confirm_send_event: bool, // flag that gets set when user wants to send something and it needs to be confirmed before sending 
    pub discount: bool, // if user can get buy overage credits without subscription(for early adopters)
    pub discount_tier: Option<String>, // could be None, "msg", "voice" or "full"
    pub free_reply: bool, // flag that gets set when previous message needs more information to finish the reply
}


#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = proactive_settings)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ProactiveSettings {
    pub id: Option<i32>,
    pub user_id: i32,
    pub imap_proactive: bool,
    pub imap_general_checks: Option<String>,
    pub proactive_calendar: bool,
    pub created_at: i32,
    pub updated_at: i32,
    pub proactive_calendar_last_activated: i32,
    pub proactive_email_last_activated: i32,
    pub proactive_whatsapp: bool,
    pub whatsapp_general_checks: Option<String>,
    pub whatsapp_keywords_active: bool,
    pub whatsapp_priority_senders_active: bool,
    pub whatsapp_waiting_checks_active: bool,
    pub whatsapp_general_importance_active: bool,
    pub email_keywords_active: bool,
    pub email_priority_senders_active: bool,
    pub email_waiting_checks_active: bool,
    pub email_general_importance_active: bool,
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
pub struct NewProcessedEmails {
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
#[diesel(table_name = subscriptions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Subscription {
    pub id: Option<i32>, // Assuming auto-incrementing primary key
    pub user_id: i32, // App's user ID
    pub paddle_subscription_id: String,
    pub paddle_customer_id: String,
    pub stage: String,
    pub status: String,
    pub next_bill_date: i32,
    pub is_scheduled_to_cancel: Option<bool>,
}


#[derive(Insertable)]
#[diesel(table_name = subscriptions)]
pub struct NewSubscription {
    pub user_id: i32, 
    pub paddle_subscription_id: String,
    pub paddle_customer_id: String,
    pub stage: String,
    pub status: String,
    pub next_bill_date: i32,
    pub is_scheduled_to_cancel: Option<bool>,
}


#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = usage_logs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UsageLog {
    pub id: Option<i32>,
    pub user_id: i32,
    pub sid: Option<String>, // elevenlabs call id or twilio message id
    pub activity_type: String, // sms or call
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

// unipile connection, turned out to be disgustingly expensive wtf 5e/month FOR ONE CONNECTION WTF FOR ONE USER
#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = unipile_connection)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UnipileConnection {
    pub id: Option<i32>,
    pub user_id: i32,
    pub account_type: String, // LINKEDIN, GMAIL, WHATSAPP,..
    pub account_id: String,
    pub status: String, // OK, CREDENTIALS, 
    pub last_update: i32,
    pub created_on: i32,
    pub description: String,
}

#[derive(Insertable)]
#[diesel(table_name = unipile_connection)]
pub struct NewUnipileConnection {
    pub user_id: i32,
    pub account_type: String,
    pub account_id: String,
    pub status: String,
    pub last_update: i32,
    pub created_on: i32,
    pub description: String,
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
#[diesel(table_name = gmail)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Gmail {
    pub id: Option<i32>,
    pub user_id: i32,
    pub encrypted_access_token: String,
    pub encrypted_refresh_token: String,
    pub status: String,
    pub last_update: i32,
    pub created_on: i32,
    pub description: String,
    pub expires_in: i32,
}

#[derive(Insertable)]
#[diesel(table_name = gmail)]
pub struct NewGmail {
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
#[diesel(table_name = waiting_checks)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct WaitingCheck {
    pub id: Option<i32>,
    pub user_id: i32,
    pub due_date: i32,
    pub content: String,
    pub remove_when_found: bool,
    pub service_type: String,// like email, whatsapp, .. 
}

#[derive(Insertable)]
#[diesel(table_name = waiting_checks)]
pub struct NewWaitingCheck {
    pub user_id: i32,
    pub due_date: i32,
    pub content: String,
    pub remove_when_found: bool,
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

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = importance_priorities)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ImportancePriority {
    pub id: Option<i32>,
    pub user_id: i32,
    pub threshold: i32,
    pub service_type: String,// like email, whatsapp, .. 
}

#[derive(Insertable)]
#[diesel(table_name = importance_priorities)]
pub struct NewImportancePriority {
    pub user_id: i32,
    pub threshold: i32,
    pub service_type: String,// like email, whatsapp, .. 
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
    pub sub_country: Option<String>, // "US", "FI", "UK", "AU", "IL"
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
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = ideas)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Idea {
    pub id: Option<i32>,
    pub creator_id: String,
    pub text: String,
    pub created_at: i32,
}

#[derive(Insertable)]
#[diesel(table_name = ideas)]
pub struct NewIdea {
    pub creator_id: String,
    pub text: String,
    pub created_at: i32,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = idea_upvotes)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct IdeaUpvote {
    pub id: Option<i32>,
    pub idea_id: i32,
    pub voter_id: String,
    pub created_at: i32,
}

#[derive(Insertable)]
#[diesel(table_name = idea_upvotes)]
pub struct NewIdeaUpvote {
    pub idea_id: i32,
    pub voter_id: String,
    pub created_at: i32,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = idea_email_subscriptions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct IdeaEmailSubscription {
    pub id: Option<i32>,
    pub idea_id: i32,
    pub email: String,
    pub created_at: i32,
}

#[derive(Insertable)]
#[diesel(table_name = idea_email_subscriptions)]
pub struct NewIdeaEmailSubscription {
    pub idea_id: i32,
    pub email: String,
    pub created_at: i32,
}
