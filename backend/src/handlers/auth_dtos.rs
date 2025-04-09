use serde::{Deserialize, Serialize};
use crate::schema::users;  
use diesel::prelude::*;
use std::fmt::Debug;

#[derive(Insertable)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub email: String,
    pub password_hash: String,
    pub phone_number: String,
    pub time_to_live: i32,
    pub notify: bool,
    pub debug_logging_permission: bool,
    pub verified: bool,
    pub credits: f32,
    pub charge_when_under: bool,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Deserialize, Clone)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub phone_number: String,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: i32,
    pub email: String,
    pub phone_number: String,
    pub nickname: Option<String>,
    pub time_to_live: Option<i32>,
    pub verified: bool,
    pub credits: f32,
    pub notify: bool,
    pub preferred_number: Option<String>,
    pub sub_tier: Option<String>,
    pub msgs_left: i32,
}

#[derive(Debug, Deserialize)]
pub struct Claims {
    pub sub: i32,
    pub exp: i64,
}

