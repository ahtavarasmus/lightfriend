use serde::{Deserialize, Serialize};
use crate::schema::users;  
use diesel::prelude::*;
use std::fmt::Debug;

#[derive(Insertable)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub username: String,
    pub password_hash: String,
    pub phone_number: String,
    pub time_to_live: i32,
    pub verified: bool,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Deserialize, Clone)]
pub struct RegisterRequest {
    pub username: String,
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
    pub username: String,
    pub phone_number: String,
}

#[derive(Debug, Deserialize)]
pub struct Claims {
    pub sub: i32,
    pub exp: i64,
}

