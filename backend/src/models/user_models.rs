use diesel::prelude::*;
use crate::schema::users;
use crate::schema::conversations;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ElevenLabsResponse {
    pub status: String,
    pub metadata: CallMetaData,
    pub conversation_initiation_client_data: CallInitiationData,
}

#[derive(Deserialize)]
pub struct CallMetaData {
    pub call_duration_secs: i32,
}

#[derive(Deserialize)]
pub struct CallInitiationData {
    pub dynamic_variables: DynVariables,
}

#[derive(Deserialize)]
pub struct DynVariables {
    pub user_id: Option<String>,
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
    pub twilio_number: String,
    pub user_number: String,
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

