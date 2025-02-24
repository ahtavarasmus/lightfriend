
use diesel::prelude::*;
use crate::schema::users;
use crate::schema::conversations;
use crate::schema::usage_logs;


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
    pub iq: i32, // user's iq balance. can be either positive(if given free credits) or negative when on usage based sub
    pub notify_credits: bool, // not used currently
    pub locality: String, // not used currently
    pub info: Option<String>, // extra info about the user for the ai
    pub preferred_number: Option<String>, // number the user prefers lightfriend texting/calling them from
    pub iq_cost_per_euro: i32, // the cost for iq in iq/€ (e.g. 300iq -> 1€)
    pub debug_logging_permission: bool,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = usage_logs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UsageLog {
    pub id: Option<i32>,
    pub user_id: i32,
    pub activity_type: String, // text or sms
    pub iq_used: i32, // the amount of iq used
    pub iq_cost_per_euro: i32, // the cost for iq in iq/€ (e.g. 300iq -> 1€)
    pub created_at: i32, // int timestamp utc epoch
    pub success: bool, // if call was successful judged by ai
    pub summary: Option<String>, // if call was not successful store it if user gives permission
}

#[derive(Insertable)]
#[diesel(table_name = usage_logs)]
pub struct NewUsageLog {
    pub user_id: i32,
    pub activity_type: String,
    pub iq_used: i32,
    pub iq_cost_per_euro: i32,
    pub created_at: i32,
    pub success: bool,
    pub summary: Option<String>,
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

