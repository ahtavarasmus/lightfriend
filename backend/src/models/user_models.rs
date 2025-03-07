
use diesel::prelude::*;
use crate::schema::users;
use crate::schema::conversations;
use crate::schema::usage_logs;
use crate::schema::subscriptions;
use crate::schema::calendar_connection;


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

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = calendar_connection)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct CalendarConnection {
    pub id: Option<i32>,
    pub user_id: i32,
    pub name: String,
    pub description: String,
    pub provider: String,
    pub encrypted_access_token: String,
}

#[derive(Insertable)]
#[diesel(table_name = calendar_connection)]
pub struct NewCalendarConnection {
    pub user_id: i32,
    pub name: String,
    pub description: String,
    pub provider: String,
    pub encrypted_access_token: String,
}


