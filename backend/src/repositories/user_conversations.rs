use diesel::prelude::*;
use diesel::result::Error as DieselError;
use std::error::Error;
use chrono::Local;
use crate::{
    models::user_models::{User, Conversation, NewConversation},
    schema::conversations,
    DbPool,
    api::twilio_utils::setup_conversation,
};

pub struct UserConversations {
    pool: DbPool
}

impl UserConversations {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create_conversation_for_user(
        &self, 
        user: &User,
        twilio_number: Option<String>
    ) -> Result<Conversation, Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let (conv_sid, service_sid) = setup_conversation(user, twilio_number).await?;
        
        let new_conversation = NewConversation {
            user_id: user.id,
            conversation_sid: conv_sid,
            service_sid: service_sid,
            created_at: Local::now().timestamp() as i32,
            active: true,
        };

        diesel::insert_into(conversations::table)
            .values(&new_conversation)
            .execute(&mut conn)?;

        // Fetch and return the created conversation
        let created_conversation = conversations::table
            .filter(conversations::user_id.eq(user.id))
            .order(conversations::id.desc())
            .first(&mut conn)?;

        Ok(created_conversation)
    }

    pub fn find_active_conversation(&self, user_id: i32) -> Result<Option<Conversation>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        use crate::schema::conversations::dsl::*;

        conversations
            .filter(user_id.eq(user_id))
            .filter(active.eq(true))
            .first(&mut conn)
            .optional()
    }
}

