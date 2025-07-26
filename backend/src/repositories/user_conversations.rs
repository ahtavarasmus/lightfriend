use diesel::prelude::*;
use diesel::result::Error as DieselError;
use std::sync::Arc;
use crate::AppState;
use std::error::Error;
use chrono::Local;
use crate::{
    models::user_models::{User, Conversation, NewConversation},
    schema::conversations,
    DbPool,
    api::twilio_utils::create_twilio_conversation_for_participant,
};


pub struct UserConversations {
    pool: DbPool
}

impl UserConversations {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

