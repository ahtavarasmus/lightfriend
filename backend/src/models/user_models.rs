
use diesel::prelude::*;
use crate::schema::users;
use crate::schema::conversations;
use crate::schema::usage_logs;
use crate::schema::subscriptions;
use crate::schema::unipile_connection;
use crate::schema::google_calendar;
use crate::schema::gmail;
use crate::schema::bridges;
use crate::schema::imap_connection;

#[derive(Queryable, Selectable, Insertable)]
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
    pub credits: f32, // user credits
    pub notify: bool, // notify when new features
    pub info: Option<String>, // extra info about the user for the ai
    pub preferred_number: Option<String>, // number the user prefers lightfriend texting/calling them from
    pub debug_logging_permission: bool,
    pub charge_when_under: bool,
    pub charge_back_to: Option<f32>,
    pub stripe_customer_id: Option<String>,
    pub stripe_payment_method_id: Option<String>,
    pub stripe_checkout_session_id: Option<String>,
    pub matrix_username: Option<String>,
    pub encrypted_matrix_access_token: Option<String>,
    pub timezone: Option<String>,
    pub timezone_auto: Option<bool>,
    pub sub_tier: Option<String>,
    pub msgs_left: i32,
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
#[diesel(table_name = bridges)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Bridge {
    pub id: Option<i32>, // Assuming auto-incrementing primary key
    user_id: i32,
    bridge_type: String, // whatsapp, telegram
    status: String, // connected, disconnected
    room_id: Option<String>,
    data: Option<String>,
    created_at: Option<i32>,
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
    pub conversation_id: Option<String>, // elevenlabs call id
    pub status: Option<String>, // call is either 'ongoing' or 'done'
    pub activity_type: String, // sms or call
    pub credits: Option<f32>, // the amount of credits used
    pub created_at: i32, // int timestamp utc epoch
    pub success: Option<bool>, // if call was successful judged by ai
    pub summary: Option<String>, // if call was not successful store it if user gives permission
    pub recharge_threshold_timestamp: Option<i32>, // timestamp when credits go below recharge threshold
    pub zero_credits_timestamp: Option<i32>, // timestamp when credits reach zero
}

#[derive(Insertable)]
#[diesel(table_name = usage_logs)]
pub struct NewUsageLog {
    pub user_id: i32,
    pub conversation_id: Option<String>,
    pub status: Option<String>,
    pub activity_type: String,
    pub credits: Option<f32>,
    pub created_at: i32,
    pub success: Option<bool>,
    pub summary: Option<String>,
    pub recharge_threshold_timestamp: Option<i32>,
    pub zero_credits_timestamp: Option<i32>,
}

#[derive(Queryable, Selectable, Insertable)]
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
