use diesel::prelude::*;
use diesel::result::Error as DieselError;
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

    pub async fn create_conversation_for_user(
        &self, 
        user: &User,
        twilio_number: String
    ) -> Result<Conversation, Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let user_number = user.phone_number.clone();
        
        let (conv_sid, service_sid) = create_twilio_conversation_for_participant(user, twilio_number.clone()).await?;
        
        let new_conversation = NewConversation {
            user_id: user.id,
            conversation_sid: conv_sid.clone(),
            service_sid: service_sid,
            created_at: Local::now().timestamp() as i32,
            active: true,
            user_number: user_number.clone(),
            twilio_number: twilio_number.clone(),
        };

        diesel::insert_into(conversations::table)
            .values(&new_conversation)
            .execute(&mut conn)?;

        // Fetch and return the created conversation
        let created_conversation = conversations::table
            .filter(conversations::user_id.eq(user.id))
            .filter(conversations::conversation_sid.eq(conv_sid))
            .filter(conversations::user_number.eq(user_number))
            .filter(conversations::twilio_number.eq(twilio_number))
            .order(conversations::id.desc())
            .first(&mut conn)?;

        Ok(created_conversation)
    }

    pub fn find_active_conversation(
        &self,
        user: &User,
        twilio_number_param: String
    ) -> Result<Option<Conversation>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        use crate::schema::conversations::dsl::*;

        conversations
            .filter(user_id.eq(user.id))
            .filter(user_number.eq(user.phone_number.clone()))
            .filter(twilio_number.eq(twilio_number_param))
            .filter(active.eq(true))
            .first(&mut conn)
            .optional()
    }

    pub async fn delete_conversation(&self, conversation_sid: &str) -> Result<(), Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Then delete the conversation record from the database
        use crate::schema::conversations::dsl::*;
        
        diesel::delete(conversations)
            .filter(conversation_sid.eq(conversation_sid))
            .execute(&mut conn)?;
        Ok(())
    }

    pub async fn get_conversation(
        &self,
        user: &User,
        twilio_number: String,
    ) -> Result<Conversation, Box<dyn Error>> {
        
        // First check if an active conversation exists for this user and Twilio number
        match self.find_active_conversation(user, twilio_number.clone())? {
            Some(conversation) => {
                println!("Found active conversation for user {} with number {}", user.phone_number, twilio_number);

                // used to just reset the conversation if problems
                if false {
                    // Delete the Twilio conversation first
                    crate::api::twilio_utils::delete_twilio_conversation(&conversation.conversation_sid).await?;
                    // Then delete the conversation from our database
                    self.delete_conversation(&conversation.conversation_sid).await?;
                    // Create a new conversation
                    return self.create_conversation_for_user(user, twilio_number).await;

                }
                
                
                // Fetch and log participants
                match crate::api::twilio_utils::fetch_conversation_participants(&conversation.conversation_sid).await {
                    Ok(participants) => {
                        println!("Conversation participants:");
                        for participant in participants {
                            if let Some(binding) = participant.messaging_binding {
                                println!("  Participant SID: {}", participant.sid);
                                println!("    Address: {:?}", binding.address);
                                println!("    Proxy Address: {:?}", binding.proxy_address);
                            }
                        }
                    },
                    Err(e) => println!("Failed to fetch participants: {}", e),
                }
                
                Ok(conversation)
            },
            None => {
                println!("No active conversation found for user {} with number {}", user.phone_number, twilio_number);
                println!("Creating a new one");
                // Create a new conversation since none exists
                self.create_conversation_for_user(user, twilio_number).await
            }
        }

    }
}

