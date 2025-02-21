use diesel::prelude::*;
use crate::schema::users;
use crate::schema::conversations;
use crate::schema::calls;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ElevenLabsResponse {
    pub status: String,
    pub call_duration_secs: i32,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct User {
    pub id: i32,
    pub email: String,
    pub password_hash: String,
    pub phone_number: String,
    pub nickname: Option<String>,
    pub time_to_live: Option<i32>,
    pub verified: bool,
    pub iq: i32,
    pub notify_credits: bool,
    pub locality: String,
    pub info: Option<String>,
    pub preferred_number: Option<String>,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = conversations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Conversation {
    pub id: i32,
    pub user_id: i32,
    pub conversation_sid: String,
    pub service_sid: String,
    pub created_at: i32,
    pub active: bool,
}

#[derive(Insertable)]
#[diesel(table_name = conversations)]
pub struct NewConversation {
    pub user_id: i32,
    pub conversation_sid: String,
    pub service_sid: String,
    pub created_at: i32,
    pub active: bool,
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = calls)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Call {
    pub id: i32,
    pub user_id: i32,
    pub conversation_id: String,
    pub status: String,
    pub analysis: Option<String>,
    pub call_duration_secs: i32,
    pub created_at: i32,
}

#[derive(Insertable, Clone)]
#[diesel(table_name = calls)]
pub struct NewCall {
    pub user_id: i32,
    pub conversation_id: String,
    pub status: String,
    pub analysis: Option<String>,
    pub call_duration_secs: i32,
    pub created_at: i32,
}

