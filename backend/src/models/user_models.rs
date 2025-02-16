use diesel::prelude::*;
use crate::schema::users;

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

