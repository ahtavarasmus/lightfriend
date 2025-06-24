use diesel::prelude::*;
use diesel::sql_types::Text;
use serde::Serialize;
use diesel::result::Error as DieselError;
use std::error::Error;
use crate::utils::encryption::{encrypt, decrypt};
use rand;

// Define the lower SQL function
sql_function! {
    fn lower(x: Text) -> Text;
    
}
#[derive(Serialize, PartialEq)]
pub struct UsageDataPoint {
    pub timestamp: i32,
    pub credits: f32,
}

use crate::{
    models::user_models::{User, NewUsageLog, NewUnipileConnection, NewGoogleCalendar, 
        ImapConnection, NewImapConnection, Bridge, NewBridge, WaitingCheck, 
        NewWaitingCheck, PrioritySender, NewPrioritySender, Keyword, 
        NewKeyword, ImportancePriority, NewImportancePriority, NewGoogleTasks,
        TaskNotification, NewTaskNotification, ProactiveSettings
    },
    handlers::auth_dtos::NewUser,
    schema::{
        users, usage_logs, unipile_connection, imap_connection,
        waiting_checks, priority_senders, keywords, importance_priorities,
    },
    DbPool,
};

pub struct UserRepository {
    pub pool: DbPool
}

impl UserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn get_conversation_history(
        &self,
        user_id: i32,
        conversation_id: &str,
        limit: i64,
    ) -> Result<Vec<crate::models::user_models::MessageHistory>, diesel::result::Error> {
        use crate::schema::message_history;
        use diesel::prelude::*;
        use crate::utils::encryption;

        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // First, get the user messages to establish time boundaries
        let user_messages = message_history::table
            .filter(message_history::user_id.eq(user_id))
            .filter(message_history::conversation_id.eq(conversation_id))
            .filter(message_history::role.eq("user"))
            .order_by(message_history::created_at.desc())
            .limit(limit)
            .load::<crate::models::user_models::MessageHistory>(&mut conn)?;

        if user_messages.is_empty() {
            return Ok(Vec::new());
        }

        // Get the timestamp of the oldest user message
        let oldest_timestamp = user_messages.last().map(|msg| msg.created_at).unwrap_or(0);

        // Now get all messages from the oldest user message onwards
        let encrypted_messages = message_history::table
            .filter(message_history::user_id.eq(user_id))
            .filter(message_history::conversation_id.eq(conversation_id))
            .filter(message_history::created_at.ge(oldest_timestamp))
            .order_by(message_history::created_at.desc())

            .load::<crate::models::user_models::MessageHistory>(&mut conn)?;

        // Decrypt the content of each message
        let mut decrypted_messages = Vec::new();
        for mut msg in encrypted_messages {
            match encryption::decrypt(&msg.encrypted_content) {
                Ok(decrypted_content) => {
                    msg.encrypted_content = decrypted_content;
                    decrypted_messages.push(msg);
                }
                Err(e) => {
                    tracing::error!("Failed to decrypt message content: {:?}", e);
                    // Skip messages that fail to decrypt
                    continue;
                }
            }
        }

        Ok(decrypted_messages)
    }

    pub fn create_message_history(&self, new_message: &crate::models::user_models::NewMessageHistory) -> Result<(), DieselError> {
        use crate::schema::message_history;
        use crate::utils::encryption;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Create a new message with encrypted content
        let encrypted_content = encryption::encrypt(&new_message.encrypted_content)
            .map_err(|e| {
                tracing::error!("Failed to encrypt message content: {:?}", e);
                DieselError::RollbackTransaction
            })?;

        let encrypted_message = crate::models::user_models::NewMessageHistory {
            user_id: new_message.user_id,
            role: new_message.role.clone(),
            encrypted_content,
            tool_name: new_message.tool_name.clone(),
            tool_call_id: new_message.tool_call_id.clone(),
            created_at: new_message.created_at,
            conversation_id: new_message.conversation_id.clone(),
        };

        diesel::insert_into(message_history::table)
            .values(&encrypted_message)
            .execute(&mut conn)?;

        Ok(())
    }

    // Helper function to create default proactive settings
    fn create_default_proactive_settings(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_settings = ProactiveSettings {
            id: None,
            user_id,
            imap_proactive: false,
            imap_general_checks: None,
            proactive_calendar: false,
            created_at: current_time,
            updated_at: current_time,
            proactive_calendar_last_activated: current_time,
            proactive_email_last_activated: current_time,
            proactive_whatsapp: false,
            whatsapp_general_checks: None,
            whatsapp_keywords_active: true,
            whatsapp_priority_senders_active: true,
            whatsapp_waiting_checks_active: true,
            whatsapp_general_importance_active: true,
            email_keywords_active: true,
            email_priority_senders_active: true,
            email_waiting_checks_active: true,
            email_general_importance_active: true,
            proactive_telegram: false,
            telegram_general_checks: None,
            telegram_keywords_active: true,
            telegram_priority_senders_active: true,
            telegram_waiting_checks_active: true,
            telegram_general_importance_active: true,
        };

        diesel::insert_into(proactive_settings::table)
            .values(&new_settings)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_task_notification(&self, user_id: i32, task_id: &str) -> Result<Option<TaskNotification>, diesel::result::Error> {
        use crate::schema::task_notifications;
        
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let result = task_notifications::table
            .filter(task_notifications::user_id.eq(user_id))
            .filter(task_notifications::task_id.eq(task_id))
            .first::<TaskNotification>(&mut conn)
            .optional()?;
            
        Ok(result)
    }
    
    pub fn create_task_notification(&self, user_id: i32, task_id: &str, notified_at: i32) -> Result<(), diesel::result::Error> {
        use crate::schema::task_notifications;
        let mut conn = self.pool.get().expect("Failed to get DB connection");


        let new_notification = NewTaskNotification {
            user_id,
            task_id: task_id.to_string(),
            notified_at,
        };
        
        diesel::insert_into(task_notifications::table)
            .values(&new_notification)
            .execute(&mut conn)?;
            
        Ok(())
    }

    pub fn delete_old_task_notifications(&self, older_than_timestamp: i32) -> Result<usize, diesel::result::Error> {
        use crate::schema::task_notifications;
        
        diesel::delete(task_notifications::table)
            .filter(task_notifications::notified_at.lt(older_than_timestamp))
            .execute(&mut self.pool.get().unwrap())
    }

    pub fn delete_old_message_history(&self, user_id: i32, conversation_id: Option<&str>, save_context_limit: i64) -> Result<usize, diesel::result::Error> {
        use crate::schema::message_history;
        use diesel::prelude::*;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Start a transaction
        conn.transaction(|conn| {
            // Find the oldest timestamp to keep based on the most recent messages
            let oldest_keep_timestamp: Option<i32> = {
                let base_query = message_history::table
                    .filter(message_history::user_id.eq(user_id))
                    .filter(message_history::role.eq("user"));

                base_query
                    .order_by(message_history::created_at.desc())
                    .limit(save_context_limit)
                    .select(message_history::created_at)
                    .load::<i32>(conn)?
                    .last()
                    .cloned()
            };

            match oldest_keep_timestamp {
                Some(timestamp) => {
                    // Build delete query
                    let base_delete = diesel::delete(message_history::table)
                        .filter(message_history::user_id.eq(user_id))
                        .filter(message_history::created_at.lt(timestamp));

                    // Execute delete with optional conversation filter
                    match conversation_id {
                        Some(conv_id) => base_delete
                            .filter(message_history::conversation_id.eq(conv_id))
                            .execute(conn),
                        None => base_delete.execute(conn)
                    }
                },
                None => Ok(0)
            }
        })
    }

 
    pub fn set_imap_credentials(
        &self,
        user_id: i32,
        email: &str,
        password: &str,
        imap_server: Option<&str>,
        imap_port: Option<u16>,
    ) -> Result<(), diesel::result::Error> {
        use crate::schema::imap_connection;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Encrypt password
        let encrypted_password = encrypt(password)
            .map_err(|_| diesel::result::Error::RollbackTransaction)?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // First, delete any existing connections for this user
        diesel::delete(imap_connection::table)
            .filter(imap_connection::user_id.eq(user_id))
            .execute(&mut conn)?;

        // Create new connection
        let new_connection = NewImapConnection {
            user_id,
            method: imap_server.map(|s| s.to_string()).unwrap_or("gmail".to_string()),
            encrypted_password,
            status: "active".to_string(),
            last_update: current_time,
            created_on: current_time,
            description: email.to_string(),
            expires_in: 0,
            imap_server: imap_server.map(|s| s.to_string()),
            imap_port: imap_port.map(|p| p as i32),
        };

        // Insert the new connection
        diesel::insert_into(imap_connection::table)
            .values(&new_connection)
            .execute(&mut conn)?;

        Ok(())
    }
    

    pub fn get_imap_credentials(
        &self,
        user_id: i32,
    ) -> Result<Option<(String, String, Option<String>, Option<i32>)>, diesel::result::Error> {
        use crate::schema::imap_connection;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Get the active IMAP connection for the user
        let imap_conn = imap_connection::table
            .filter(imap_connection::user_id.eq(user_id))
            .filter(imap_connection::status.eq("active"))
            .first::<crate::models::user_models::ImapConnection>(&mut conn)
            .optional()?;

        if let Some(conn) = imap_conn {
            // Decrypt the password
            match decrypt(&conn.encrypted_password) {
                Ok(decrypted_password) => Ok(Some((conn.description, decrypted_password, conn.imap_server, conn.imap_port))),
                Err(_) => Err(diesel::result::Error::RollbackTransaction)
            }
        } else {
            Ok(None)
        }
    }

    pub fn delete_imap_credentials(
        &self,
        user_id: i32,
    ) -> Result<(), diesel::result::Error> {
        use crate::schema::imap_connection;
        let connection = &mut self.pool.get().unwrap();
        
        diesel::delete(imap_connection::table
            .filter(imap_connection::user_id.eq(user_id)))
            .execute(connection)?;
        
        Ok(())
    }

    // log the usage. activity_type either 'call' or 'sms'
    pub fn log_usage(&self, user_id: i32, sid: Option<String>, activity_type: String, credits: Option<f32>, time_consumed: Option<i32>, success: Option<bool>, reason: Option<String>, status: Option<String>, recharge_threshold_timestamp: Option<i32>, zero_credits_timestamp: Option<i32>) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let user = users::table
            .find(user_id)
            .first::<User>(&mut conn)?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_log = NewUsageLog {
            user_id,
            sid,
            activity_type,
            credits,
            created_at: current_time,
            time_consumed,
            success,
            reason,
            status,
            recharge_threshold_timestamp,
            zero_credits_timestamp,
        };

        diesel::insert_into(usage_logs::table)
            .values(&new_log)
            .execute(&mut conn)?;
        Ok(())
    }


    pub fn is_credits_under_threshold(&self, user_id: i32) -> Result<bool, DieselError> {

        let charge_back_threshold= std::env::var("CHARGE_BACK_THRESHOLD")
            .expect("CHARGE_BACK_THRESHOLD not set")
            .parse::<f32>()
            .unwrap_or(2.00);

        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table
            .find(user_id)
            .first::<User>(&mut conn)?;
        
        Ok(user.credits < charge_back_threshold)
    }

    pub fn get_usage_data(&self, user_id: i32, from_timestamp: i32) -> Result<Vec<UsageDataPoint>, DieselError> {
        // Check if we're in development mode
        if std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()) != "development" {
            // Generate example data for the last 30 days
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;
            
            let mut example_data = Vec::new();
            let day_in_seconds = 24 * 60 * 60;
            
            // Generate random usage data for each day
            for i in 0..30 {
                let timestamp = now - (i * day_in_seconds);
                if timestamp >= from_timestamp {
                    // Random usage between 50 and 500
                    let usage = rand::random::<f32>() % 451.00 + 50.00;
                    example_data.push(UsageDataPoint {
                        timestamp,
                        credits: usage,
                    });
                    
                    // Sometimes add multiple entries per day
                    if rand::random::<f32>() > 0.7 {
                        let credit_usage = rand::random::<f32>() % 301.00 + 20.00;
                        example_data.push(UsageDataPoint {
                            timestamp: timestamp + 3600, // 1 hour later
                            credits: credit_usage,
                        });
                    }
                }
            }
            
            example_data.sort_by_key(|point| point.timestamp);
            println!("returning example data");
            return Ok(example_data);
        }
        println!("getting real usage data");
        use crate::schema::usage_logs::dsl::*;
        
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Query usage logs for the user within the time range
        let usage_data = usage_logs
            .filter(user_id.eq(user_id))
            .filter(created_at.ge(from_timestamp))
            .select((created_at, credits))
            .order_by(created_at.asc())
            .load::<(i32, Option<f32>)>(&mut conn)?
            .into_iter()
            .filter_map(|(timestamp, credit_amount)| {
                credit_amount.map(|credit_value| UsageDataPoint {
                    timestamp,
                    credits: credit_value,
                })
            })
            .collect();

        Ok(usage_data)
    }

    // Fetch the ongoing usage log for a user
    pub fn get_ongoing_usage(&self, user_id: i32) -> Result<Option<crate::models::user_models::UsageLog>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let ongoing_log = usage_logs::table
            .filter(usage_logs::user_id.eq(user_id))
            .filter(usage_logs::status.eq("ongoing"))
            .first::<crate::models::user_models::UsageLog>(&mut conn)
            .optional()?;
        Ok(ongoing_log)
    }

    pub fn update_usage_log_fields(&self, user_id: i32, sid: &str, status: &str, success: bool, reason: &str, call_duration: Option<i32>) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(usage_logs::table)
            .filter(usage_logs::user_id.eq(user_id))
            .filter(usage_logs::sid.eq(sid))
            .set((
                usage_logs::success.eq(success),
                usage_logs::reason.eq(reason),
                usage_logs::status.eq(status),
                usage_logs::call_duration.eq(call_duration),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_all_ongoing_usage(&self) -> Result<Vec<crate::models::user_models::UsageLog>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let ongoing_logs = usage_logs::table
            .filter(usage_logs::status.eq("ongoing"))
            .load::<crate::models::user_models::UsageLog>(&mut conn)?;
        Ok(ongoing_logs)
    }

    pub fn get_all_usage_logs(&self) -> Result<Vec<crate::models::user_models::UsageLog>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Get all usage logs ordered by creation time (newest first)
        let logs = usage_logs::table
            .order_by(usage_logs::created_at.desc())
            .load::<crate::models::user_models::UsageLog>(&mut conn)?;
        
        Ok(logs)
    }

        pub fn update_usage_log_timestamps(&self, sid: &str, recharge_threshold_timestamp: Option<i32>, zero_credits_timestamp: Option<i32>) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(usage_logs::table)
            .filter(usage_logs::sid.eq(sid))
            .set((
                usage_logs::recharge_threshold_timestamp.eq(recharge_threshold_timestamp),
                usage_logs::zero_credits_timestamp.eq(zero_credits_timestamp),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    // Waiting Checks methods
    pub fn create_waiting_check(&self, new_check: &NewWaitingCheck) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(waiting_checks::table)
            .values(new_check)
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn delete_waiting_check(&self, user_id: i32, service_type: &str, content: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(waiting_checks::table)
            .filter(waiting_checks::user_id.eq(user_id))
            .filter(waiting_checks::service_type.eq(service_type))
            .filter(waiting_checks::content.eq(content))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_waiting_checks(&self, user_id: i32, service_type: &str) -> Result<Vec<WaitingCheck>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        waiting_checks::table
            .filter(waiting_checks::user_id.eq(user_id))
            .filter(waiting_checks::service_type.eq(service_type))
            .load::<WaitingCheck>(&mut conn)
    }

    // Priority Senders methods
    pub fn create_priority_sender(&self, new_sender: &NewPrioritySender) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(priority_senders::table)
            .values(new_sender)
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn delete_priority_sender(&self, user_id: i32, service_type: &str, sender: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(priority_senders::table)
            .filter(priority_senders::user_id.eq(user_id))
            .filter(priority_senders::service_type.eq(service_type))
            .filter(priority_senders::sender.eq(sender))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_priority_senders(&self, user_id: i32, service_type: &str) -> Result<Vec<PrioritySender>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        priority_senders::table
            .filter(priority_senders::user_id.eq(user_id))
            .filter(priority_senders::service_type.eq(service_type))
            .load::<PrioritySender>(&mut conn)
    }

    // Keywords methods
    pub fn create_keyword(&self, new_keyword: &NewKeyword) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(keywords::table)
            .values(new_keyword)
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn delete_keyword(&self, user_id: i32, service_type: &str, keyword: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(keywords::table)
            .filter(keywords::user_id.eq(user_id))
            .filter(keywords::service_type.eq(service_type))
            .filter(keywords::keyword.eq(keyword))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_keywords(&self, user_id: i32, service_type: &str) -> Result<Vec<Keyword>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        keywords::table
            .filter(keywords::user_id.eq(user_id))
            .filter(keywords::service_type.eq(service_type))
            .load::<Keyword>(&mut conn)
    }

    pub fn create_importance_priority(&self, new_priority: &NewImportancePriority) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // First delete any existing importance priority for this user and service type
        diesel::delete(importance_priorities::table)
            .filter(importance_priorities::user_id.eq(new_priority.user_id))
            .filter(importance_priorities::service_type.eq(&new_priority.service_type))
            .execute(&mut conn)?;

        // Then insert the new priority
        diesel::insert_into(importance_priorities::table)
            .values(new_priority)
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn delete_importance_priority(&self, user_id: i32, service_type: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(importance_priorities::table)
            .filter(importance_priorities::user_id.eq(user_id))
            .filter(importance_priorities::service_type.eq(service_type))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_importance_priority(&self, user_id: i32, service_type: &str) -> Result<Option<ImportancePriority>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let importance_priority = importance_priorities::table
            .filter(importance_priorities::user_id.eq(user_id))
            .filter(importance_priorities::service_type.eq(service_type))
            .first::<ImportancePriority>(&mut conn)
            .optional()?;
        Ok(importance_priority)
    }

    // Update user's imap_proactive setting
    pub fn update_imap_proactive(&self, user_id: i32, proactive: bool) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // Check if settings exist for this user
        let existing_settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        match existing_settings {
            Some(_) => {
                // Update existing settings

                let existing_settings = proactive_settings::table
                    .filter(proactive_settings::user_id.eq(user_id))
                    .first::<ProactiveSettings>(&mut conn)?;

                diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                    .set((
                        proactive_settings::imap_proactive.eq(proactive),
                        proactive_settings::updated_at.eq(current_time),
                        proactive_settings::proactive_email_last_activated.eq(if proactive { current_time } else { existing_settings.proactive_email_last_activated }),
                    ))
                    .execute(&mut conn)?;
            },
            None => {
                self.create_default_proactive_settings(user_id)?;
                // Update the newly created settings with the proactive value
                diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                    .set(proactive_settings::imap_proactive.eq(proactive))
                    .execute(&mut conn)?;
            }
        }
        Ok(())
    }

    pub fn get_imap_proactive(&self, user_id: i32) -> Result<bool, DieselError> {
        self.get_imap_proactive_status(user_id).map(|(enabled, _)| enabled)
    }

    pub fn get_imap_proactive_status(&self, user_id: i32) -> Result<(bool, i32), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        Ok(settings.map_or((false, 0), |s| (s.imap_proactive, s.proactive_email_last_activated)))
    }


    // Update user's custom IMAP general checks
    pub fn update_imap_general_checks(&self, user_id: i32, checks: Option<&str>) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // Check if settings exist for this user
        let existing_settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        match existing_settings {
            Some(_) => {
                // Update existing settings
                diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                    .set((
                        proactive_settings::imap_general_checks.eq(checks.map(|s| s.to_string())),
                        proactive_settings::updated_at.eq(current_time),
                    ))
                    .execute(&mut conn)?;
            },
            None => {
                self.create_default_proactive_settings(user_id)?;
                // Update the newly created settings with the checks value
                if let Some(checks_str) = checks {
                    diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                        .set(proactive_settings::imap_general_checks.eq(checks_str.to_string()))
                        .execute(&mut conn)?;
                }
            }
        }
        Ok(())
    }

    pub fn update_proactive_calendar(&self, user_id: i32, proactive: bool) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // Check if settings exist for this user
        let existing_settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        match existing_settings {
            Some(_) => {
                // Update existing settings
                diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                    .set((
                        proactive_settings::proactive_calendar.eq(proactive),
                        proactive_settings::updated_at.eq(current_time),
                        proactive_settings::proactive_calendar_last_activated.eq(current_time),
                    ))
                    .execute(&mut conn)?;
            },
            None => {
                self.create_default_proactive_settings(user_id)?;
                // Update the newly created settings with the proactive value
                diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                    .set(proactive_settings::proactive_calendar.eq(proactive))
                    .execute(&mut conn)?;
            }
        }
        Ok(())
    }

    pub fn get_proactive_calendar(&self, user_id: i32) -> Result<bool, DieselError> {
        self.get_proactive_calendar_status(user_id).map(|(enabled, _)| enabled)
    }

    pub fn get_proactive_calendar_status(&self, user_id: i32) -> Result<(bool, i32), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        Ok(settings.map_or((false, 0), |s| (s.proactive_calendar, s.proactive_calendar_last_activated)))
    }

    pub fn get_proactive_whatsapp(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        Ok(settings.map_or(false, |s| s.proactive_whatsapp))
    }

    pub fn get_proactive_telegram(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        Ok(settings.map_or(false, |s| s.proactive_telegram))
    }

    pub fn update_proactive_whatsapp(&self, user_id: i32, proactive: bool) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // Check if settings exist for this user
        let existing_settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        match existing_settings {
            Some(_) => {
                // Update existing settings
                diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                    .set((
                        proactive_settings::proactive_whatsapp.eq(proactive),
                        proactive_settings::updated_at.eq(current_time),
                    ))
                    .execute(&mut conn)?;
            },
            None => {
                self.create_default_proactive_settings(user_id)?;
                // Update the newly created settings with the proactive value
                diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                    .set(proactive_settings::proactive_whatsapp.eq(proactive))
                    .execute(&mut conn)?;
            }
        }
        Ok(())
    }

    pub fn update_proactive_telegram(&self, user_id: i32, proactive: bool) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // Check if settings exist for this user
        let existing_settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        match existing_settings {
            Some(_) => {
                // Update existing settings
                diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                    .set((
                        proactive_settings::proactive_telegram.eq(proactive),
                        proactive_settings::updated_at.eq(current_time),
                    ))
                    .execute(&mut conn)?;
            },
            None => {
                self.create_default_proactive_settings(user_id)?;
                // Update the newly created settings with the proactive value
                diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                    .set(proactive_settings::proactive_telegram.eq(proactive))
                    .execute(&mut conn)?;
            }
        }
        Ok(())
    }

    pub fn get_whatsapp_general_checks(&self, user_id: i32) -> Result<String, DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;
            
        Ok(settings.and_then(|s| s.whatsapp_general_checks).unwrap_or_else(|| {
            // Default general checks prompt for WhatsApp messages
            String::from("
                Step 1: Check for Urgency Indicators
                - Look for words like 'urgent', 'immediate', 'asap', 'deadline', 'important', 'emergency'
                - Check for time-sensitive phrases like 'by tomorrow', 'end of day', 'as soon as possible', 'right now'
                - Look for multiple exclamation marks or all-caps words that might indicate urgency
                - Check for repeated messages or follow-ups indicating urgency

                Step 2: Analyze Sender Importance
                - Check if it's from family members, close friends, or emergency contacts
                - Look for messages from work colleagues, managers, or supervisors
                - Consider if it's from clients or important business partners
                - Assess if it's from service providers (doctors, lawyers, etc.)

                Step 3: Assess Content Significance
                - Look for action items or direct requests that need immediate response
                - Check for mentions of meetings, appointments, or time-sensitive events
                - Identify emergency situations or health-related concerns
                - Look for financial matters, payments, or important transactions
                - Check for travel-related information or changes

                Step 4: Consider Context and Timing
                - Consider if it's outside normal hours (late night/early morning might indicate urgency)
                - Check if it's a reply to something you sent recently
                - Look for group messages where you're specifically mentioned
                - Consider if it's breaking a long silence in conversation

                Step 5: Evaluate Personal Impact
                - Assess if immediate action or response is required
                - Consider if delaying response could have negative consequences
                - Look for personal emergencies or family matters
                - Check for work-critical communications
                - Identify if it contains sensitive or confidential information
            ")
        }))
    }

    pub fn get_telegram_general_checks(&self, user_id: i32) -> Result<String, DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;
            
        Ok(settings.and_then(|s| s.telegram_general_checks).unwrap_or_else(|| {
            // Default general checks prompt for Telegram messages
            String::from("
                Step 1: Check for Urgency Indicators
                - Look for words like 'urgent', 'immediate', 'asap', 'deadline', 'important', 'emergency'
                - Check for time-sensitive phrases like 'by tomorrow', 'end of day', 'as soon as possible', 'right now'
                - Look for multiple exclamation marks or all-caps words that might indicate urgency
                - Check for repeated messages or follow-ups indicating urgency

                Step 2: Analyze Sender Importance
                - Check if it's from family members, close friends, or emergency contacts
                - Look for messages from work colleagues, managers, or supervisors
                - Consider if it's from clients or important business partners
                - Assess if it's from service providers (doctors, lawyers, etc.)

                Step 3: Assess Content Significance
                - Look for action items or direct requests that need immediate response
                - Check for mentions of meetings, appointments, or time-sensitive events
                - Identify emergency situations or health-related concerns
                - Look for financial matters, payments, or important transactions
                - Check for travel-related information or changes

                Step 4: Consider Context and Timing
                - Consider if it's outside normal hours (late night/early morning might indicate urgency)
                - Check if it's a reply to something you sent recently
                - Look for group messages where you're specifically mentioned
                - Consider if it's breaking a long silence in conversation

                Step 5: Evaluate Personal Impact
                - Assess if immediate action or response is required
                - Consider if delaying response could have negative consequences
                - Look for personal emergencies or family matters
                - Check for work-critical communications
                - Identify if it contains sensitive or confidential information
            ")
        }))
    }

    // Update user's custom WhatsApp general checks
    pub fn update_whatsapp_general_checks(&self, user_id: i32, checks: Option<&str>) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // Check if settings exist for this user
        let existing_settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        match existing_settings {
            Some(_) => {
                // Update existing settings
                diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                    .set((
                        proactive_settings::whatsapp_general_checks.eq(checks.map(|s| s.to_string())),
                        proactive_settings::updated_at.eq(current_time),
                    ))
                    .execute(&mut conn)?;
            },
            None => {
                self.create_default_proactive_settings(user_id)?;
                // Update the newly created settings with the checks value
                if let Some(checks_str) = checks {
                    diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                        .set(proactive_settings::whatsapp_general_checks.eq(checks_str.to_string()))
                        .execute(&mut conn)?;
                }
            }
        }
        Ok(())
    }


    // Update user's custom Telegram general checks
    pub fn update_telegram_general_checks(&self, user_id: i32, checks: Option<&str>) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // Check if settings exist for this user
        let existing_settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        match existing_settings {
            Some(_) => {
                // Update existing settings
                diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                    .set((
                        proactive_settings::telegram_general_checks.eq(checks.map(|s| s.to_string())),
                        proactive_settings::updated_at.eq(current_time),
                    ))
                    .execute(&mut conn)?;
            },
            None => {
                self.create_default_proactive_settings(user_id)?;
                // Update the newly created settings with the checks value
                if let Some(checks_str) = checks {
                    diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
                        .set(proactive_settings::telegram_general_checks.eq(checks_str.to_string()))
                        .execute(&mut conn)?;
                }
            }
        }
        Ok(())
    }

    // WhatsApp filter activation methods
    pub fn update_whatsapp_keywords_active(&self, user_id: i32, active: bool) -> Result<(), DieselError> {
        println!("at update_whatsapp_keywords_active!");
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        self.ensure_proactive_settings_exist(user_id)?;

        diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
            .set((
                proactive_settings::whatsapp_keywords_active.eq(active),
                proactive_settings::updated_at.eq(current_time),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    // Telegram filter activation methods
    pub fn update_telegram_keywords_active(&self, user_id: i32, active: bool) -> Result<(), DieselError> {
        println!("at update_telegram_keywords_active!");
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        self.ensure_proactive_settings_exist(user_id)?;

        diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
            .set((
                proactive_settings::telegram_keywords_active.eq(active),
                proactive_settings::updated_at.eq(current_time),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_whatsapp_priority_senders_active(&self, user_id: i32, active: bool) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        self.ensure_proactive_settings_exist(user_id)?;

        diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
            .set((
                proactive_settings::whatsapp_priority_senders_active.eq(active),
                proactive_settings::updated_at.eq(current_time),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_telegram_priority_senders_active(&self, user_id: i32, active: bool) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        self.ensure_proactive_settings_exist(user_id)?;

        diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
            .set((
                proactive_settings::telegram_priority_senders_active.eq(active),
                proactive_settings::updated_at.eq(current_time),
            ))
            .execute(&mut conn)?;
        Ok(())
    }


    pub fn update_whatsapp_waiting_checks_active(&self, user_id: i32, active: bool) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        self.ensure_proactive_settings_exist(user_id)?;

        diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
            .set((
                proactive_settings::whatsapp_waiting_checks_active.eq(active),
                proactive_settings::updated_at.eq(current_time),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_telegram_waiting_checks_active(&self, user_id: i32, active: bool) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        self.ensure_proactive_settings_exist(user_id)?;

        diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
            .set((
                proactive_settings::telegram_waiting_checks_active.eq(active),
                proactive_settings::updated_at.eq(current_time),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_whatsapp_general_importance_active(&self, user_id: i32, active: bool) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        self.ensure_proactive_settings_exist(user_id)?;

        diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
            .set((
                proactive_settings::whatsapp_general_importance_active.eq(active),
                proactive_settings::updated_at.eq(current_time),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_telegram_general_importance_active(&self, user_id: i32, active: bool) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        self.ensure_proactive_settings_exist(user_id)?;

        diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
            .set((
                proactive_settings::telegram_general_importance_active.eq(active),
                proactive_settings::updated_at.eq(current_time),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    // Email filter activation methods
    pub fn update_email_keywords_active(&self, user_id: i32, active: bool) -> Result<(), DieselError> {
        println!("at update_email_keywords_active!");
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        self.ensure_proactive_settings_exist(user_id)?;

        diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
            .set((
                proactive_settings::email_keywords_active.eq(active),
                proactive_settings::updated_at.eq(current_time),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_email_priority_senders_active(&self, user_id: i32, active: bool) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        self.ensure_proactive_settings_exist(user_id)?;

        diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
            .set((
                proactive_settings::email_priority_senders_active.eq(active),
                proactive_settings::updated_at.eq(current_time),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_email_waiting_checks_active(&self, user_id: i32, active: bool) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        self.ensure_proactive_settings_exist(user_id)?;

        diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
            .set((
                proactive_settings::email_waiting_checks_active.eq(active),
                proactive_settings::updated_at.eq(current_time),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_email_general_importance_active(&self, user_id: i32, active: bool) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        self.ensure_proactive_settings_exist(user_id)?;

        diesel::update(proactive_settings::table.filter(proactive_settings::user_id.eq(user_id)))
            .set((
                proactive_settings::email_general_importance_active.eq(active),
                proactive_settings::updated_at.eq(current_time),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    // Helper method to ensure proactive settings exist for a user
    fn ensure_proactive_settings_exist(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // Check if settings exist for this user
        let existing_settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        if existing_settings.is_none() {
            // Create new settings with default values
            let new_settings = ProactiveSettings {
                id: None,
                user_id,
                imap_proactive: false,
                imap_general_checks: None,
                proactive_calendar: false,
                created_at: current_time,
                updated_at: current_time,
                proactive_calendar_last_activated: current_time,
                proactive_email_last_activated: current_time,
                proactive_whatsapp: false,
                whatsapp_general_checks: None,
                whatsapp_keywords_active: true,
                whatsapp_priority_senders_active: true,
                whatsapp_waiting_checks_active: true,
                whatsapp_general_importance_active: true,
                email_keywords_active: true,
                email_priority_senders_active: true,
                email_waiting_checks_active: true,
                email_general_importance_active: true,
                proactive_telegram: false,
                telegram_general_checks: None,
                telegram_keywords_active: true,
                telegram_priority_senders_active: true,
                telegram_waiting_checks_active: true,
                telegram_general_importance_active: true,
            };
            diesel::insert_into(proactive_settings::table)
                .values(&new_settings)
                .execute(&mut conn)?;
        }
        Ok(())
    }

    // Getter methods for filter activation status
    pub fn get_whatsapp_filter_settings(&self, user_id: i32) -> Result<(bool, bool, bool, bool), DieselError> {
        println!("at get_whatsapp_filter_settings!");
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        Ok(settings.map_or(
            (true, true, true, true), // Default all active
            |s| (
                s.whatsapp_keywords_active,
                s.whatsapp_priority_senders_active,
                s.whatsapp_waiting_checks_active,
                s.whatsapp_general_importance_active,
            )
        ))
    }

    // Getter methods for filter activation status
    pub fn get_telegram_filter_settings(&self, user_id: i32) -> Result<(bool, bool, bool, bool), DieselError> {
        println!("at get_telegram_filter_settings!");
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        Ok(settings.map_or(
            (true, true, true, true), // Default all active
            |s| (
                s.telegram_keywords_active,
                s.telegram_priority_senders_active,
                s.telegram_waiting_checks_active,
                s.telegram_general_importance_active,
            )
        ))
    }

    pub fn get_email_filter_settings(&self, user_id: i32) -> Result<(bool, bool, bool, bool), DieselError> {
        println!("at get_email_filter_settings!");
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;

        Ok(settings.map_or(
            (true, true, true, true), // Default all active
            |s| (
                s.email_keywords_active,
                s.email_priority_senders_active,
                s.email_waiting_checks_active,
                s.email_general_importance_active,
            )
        ))
    }


    pub fn get_imap_general_checks(&self, user_id: i32) -> Result<String, DieselError> {
        use crate::schema::proactive_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings = proactive_settings::table
            .filter(proactive_settings::user_id.eq(user_id))
            .first::<ProactiveSettings>(&mut conn)
            .optional()?;
            
        Ok(settings.and_then(|s| s.imap_general_checks).unwrap_or_else(|| {
            // Default general checks prompt
            String::from("
                Step 1: Check for Urgency Indicators
                - Look for words like 'urgent', 'immediate', 'asap', 'deadline', 'important'
                - Check for time-sensitive phrases like 'by tomorrow', 'end of day', 'as soon as possible'
                - Look for exclamation marks or all-caps words that might indicate urgency

                Step 2: Analyze Sender Importance
                - Check if it's from a manager, supervisor, or higher-up in organization
                - Look for professional titles or positions in signatures
                - Consider if it's from a client or important business partner

                Step 3: Assess Content Significance
                - Look for action items or direct requests
                - Check for mentions of meetings, deadlines, or deliverables
                - Identify if it's part of an ongoing important conversation
                - Look for financial or legal terms that might indicate important matters

                Step 4: Consider Context
                - Check if it's a reply to an email you sent
                - Look for CC'd important stakeholders
                - Consider if it's outside normal business hours
                - Check if it's marked as high priority

                Step 5: Evaluate Personal Impact
                - Assess if immediate action is required
                - Consider if delaying response could have negative consequences
                - Look for personal or confidential matters
            ")
        }))
    }


    pub fn create_unipile_connection(&self, new_connection: &NewUnipileConnection) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        diesel::insert_into(unipile_connection::table)
            .values(new_connection)
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn has_active_google_calendar(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::google_calendar;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = google_calendar::table
            .filter(google_calendar::user_id.eq(user_id))
            .filter(google_calendar::status.eq("active"))
            .first::<crate::models::user_models::GoogleCalendar>(&mut conn)
            .optional()?;

        Ok(connection.is_some())
    }
    pub fn get_google_calendar_tokens(&self, user_id: i32) -> Result<Option<(String, String)>, DieselError> {
        use crate::schema::google_calendar;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = google_calendar::table
            .filter(google_calendar::user_id.eq(user_id))
            .filter(google_calendar::status.eq("active"))
            .first::<crate::models::user_models::GoogleCalendar>(&mut conn)
            .optional()?;

        if let Some(connection) = connection {
            tracing::info!("Found active Google Calendar connection for user {}", user_id);
            
            // Decrypt access token
            let access_token = match decrypt(&connection.encrypted_access_token) {
                Ok(token) => {
                    tracing::debug!("Successfully decrypted access token");
                    token
                },
                Err(e) => {
                    tracing::error!("Failed to decrypt access token: {:?}", e);
                    return Err(DieselError::RollbackTransaction);
                }
            };

            // Decrypt refresh token
            let refresh_token = match decrypt(&connection.encrypted_refresh_token) {
                Ok(token) => {
                    tracing::debug!("Successfully decrypted refresh token");
                    token
                },
                Err(e) => {
                    tracing::error!("Failed to decrypt refresh token: {:?}", e);
                    return Err(DieselError::RollbackTransaction);
                }
            };
            

            tracing::info!("Successfully retrieved and decrypted calendar tokens for user {}", user_id);
            Ok(Some((access_token, refresh_token)))
        } else {
            tracing::info!("No active calendar connection found for user {}", user_id);
            Ok(None)
        }
    }

    pub fn update_google_calendar_access_token(
        &self,
        user_id: i32,
        new_access_token: &str,
        expires_in: i32,
    ) -> Result<(), DieselError> {
        use crate::schema::google_calendar;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let encrypted_access_token = encrypt(new_access_token)
            .map_err(|_| DieselError::RollbackTransaction)?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        diesel::update(google_calendar::table)
            .filter(google_calendar::user_id.eq(user_id))
            .filter(google_calendar::status.eq("active"))
            .set((
                google_calendar::encrypted_access_token.eq(encrypted_access_token),
                google_calendar::expires_in.eq(expires_in),
                google_calendar::last_update.eq(current_time),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn delete_google_calendar_connection(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::google_calendar;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(google_calendar::table)
            .filter(google_calendar::user_id.eq(user_id))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_active_imap_connection_users(&self) -> Result<Vec<i32>, DieselError> {
        use crate::schema::imap_connection;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let user_ids = imap_connection::table
            .filter(imap_connection::status.eq("active"))
            .select(imap_connection::user_id)
            .load::<i32>(&mut conn)?;

        Ok(user_ids)
    }

    pub fn has_active_google_tasks(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::google_tasks;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = google_tasks::table
            .filter(google_tasks::user_id.eq(user_id))
            .filter(google_tasks::status.eq("active"))
            .first::<crate::models::user_models::GoogleTasks>(&mut conn)
            .optional()?;

        Ok(connection.is_some())
    }

    pub fn get_google_tasks_tokens(&self, user_id: i32) -> Result<Option<(String, String)>, DieselError> {
        use crate::schema::google_tasks;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = google_tasks::table
            .filter(google_tasks::user_id.eq(user_id))
            .filter(google_tasks::status.eq("active"))
            .first::<crate::models::user_models::GoogleTasks>(&mut conn)
            .optional()?;

        if let Some(connection) = connection {
            let access_token = match decrypt(&connection.encrypted_access_token) {
                Ok(token) => token,
                Err(e) => {
                    tracing::error!("Failed to decrypt access token: {:?}", e);
                    return Err(DieselError::RollbackTransaction);
                }
            };

            let refresh_token = match decrypt(&connection.encrypted_refresh_token) {
                Ok(token) => token,
                Err(e) => {
                    tracing::error!("Failed to decrypt refresh token: {:?}", e);
                    return Err(DieselError::RollbackTransaction);
                }
            };

            Ok(Some((access_token, refresh_token)))
        } else {
            Ok(None)
        }
    }

    pub fn update_google_tasks_access_token(
        &self,
        user_id: i32,
        new_access_token: &str,
        expires_in: i32,
    ) -> Result<(), DieselError> {
        use crate::schema::google_tasks;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let encrypted_access_token = encrypt(new_access_token)
            .map_err(|_| DieselError::RollbackTransaction)?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        diesel::update(google_tasks::table)
            .filter(google_tasks::user_id.eq(user_id))
            .filter(google_tasks::status.eq("active"))
            .set((
                google_tasks::encrypted_access_token.eq(encrypted_access_token),
                google_tasks::expires_in.eq(expires_in),
                google_tasks::last_update.eq(current_time),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn delete_google_tasks_connection(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::google_tasks;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(google_tasks::table)
            .filter(google_tasks::user_id.eq(user_id))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn create_google_tasks_connection(
        &self,
        user_id: i32,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: i32,
    ) -> Result<(), DieselError> {
        use crate::schema::google_tasks;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let encrypted_access_token = encrypt(access_token)
            .map_err(|_| DieselError::RollbackTransaction)?;
        let encrypted_refresh_token = refresh_token
            .map(|token| encrypt(token))
            .transpose()
            .map_err(|_| DieselError::RollbackTransaction)?
            .unwrap_or_default();

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_connection = NewGoogleTasks {
            user_id,
            encrypted_access_token,
            encrypted_refresh_token,
            expires_in,
            status: "active".to_string(),
            last_update: current_time,
            created_on: current_time,
            description: "Google Tasks Connection".to_string(),
        };

        // First, delete any existing connections for this user
        diesel::delete(google_tasks::table)
            .filter(google_tasks::user_id.eq(user_id))
            .execute(&mut conn)?;

        // Then insert the new connection
        diesel::insert_into(google_tasks::table)
            .values(&new_connection)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn set_matrix_credentials(&self, user_id: i32, username: &str, access_token: &str, device_id: &str, password: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Encrypt the access token before storing
        let encrypted_token = crate::utils::encryption::encrypt(access_token)
            .map_err(|_| DieselError::RollbackTransaction)?;

        // Encrypt the password before storing
        let encrypted_password = crate::utils::encryption::encrypt(password)
            .map_err(|_| DieselError::RollbackTransaction)?;

        diesel::update(users::table.find(user_id))
            .set((
                users::matrix_username.eq(username),
                users::encrypted_matrix_access_token.eq(encrypted_token),
                users::matrix_device_id.eq(device_id),
                users::encrypted_matrix_password.eq(encrypted_password),
            ))
            .execute(&mut conn)?;

        Ok(())
    }
    pub fn set_matrix_device_id_and_access_token(&self, user_id: i32, access_token: &str, device_id: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Encrypt the access token before storing
        let encrypted_token = crate::utils::encryption::encrypt(access_token)
            .map_err(|_| DieselError::RollbackTransaction)?;

        diesel::update(users::table.find(user_id))
            .set((
                users::encrypted_matrix_access_token.eq(encrypted_token),
                users::matrix_device_id.eq(device_id),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn set_matrix_secret_storage_recovery_key(&self, user_id: i32, recovery_key: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Encrypt the password before storing
        let encrypted_key= crate::utils::encryption::encrypt(recovery_key)
            .map_err(|_| DieselError::RollbackTransaction)?;

        diesel::update(users::table.find(user_id))
            .set(users::encrypted_matrix_secret_storage_recovery_key.eq(encrypted_key))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_matrix_secret_storage_recovery_key(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let encrypted_key= users::table
            .find(user_id)
            .select(users::encrypted_matrix_secret_storage_recovery_key)
            .first::<Option<String>>(&mut conn)?;

        match encrypted_key {
            Some(key) => {
                let rec_key= crate::utils::encryption::decrypt(&key)
                    .map_err(|_| DieselError::RollbackTransaction)?;
                Ok(Some(rec_key))
            },
            _ => Ok(None),
        }
    }

    pub fn get_matrix_credentials(&self, user_id: i32) -> Result<Option<(String, String, String, String)>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let user = users::table
            .find(user_id)
            .first::<User>(&mut conn)?;

        match (user.matrix_username, user.encrypted_matrix_access_token, user.matrix_device_id, user.encrypted_matrix_password) {
            (Some(username), Some(encrypted_token), Some(device_id), Some(encrypted_password)) => {
                let token = crate::utils::encryption::decrypt(&encrypted_token)
                    .map_err(|_| DieselError::RollbackTransaction)?;
                let password= crate::utils::encryption::decrypt(&encrypted_password)
                    .map_err(|_| DieselError::RollbackTransaction)?;
                Ok(Some((username, token, device_id, password)))
            },
            _ => Ok(None),
        }
    }



    pub fn delete_matrix_credentials(&self, user_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::update(users::table.find(user_id))
            .set((
                users::matrix_username.eq::<Option<String>>(None),
                users::encrypted_matrix_access_token.eq::<Option<String>>(None),
                users::matrix_device_id.eq::<Option<String>>(None),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn create_bridge(&self, new_bridge: NewBridge) -> Result<(), DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::insert_into(bridges::table)
            .values(&new_bridge)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn delete_whatsapp_bridge(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(bridges::table)
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq("whatsapp"))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn delete_telegram_bridge(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(bridges::table)
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq("telegram"))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_whatsapp_bridge(&self, user_id: i32) -> Result<Option<Bridge>, DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let bridge = bridges::table
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq("whatsapp"))
            .first::<Bridge>(&mut conn)
            .optional()?;

        Ok(bridge)
    }

    pub fn get_telegram_bridge(&self, user_id: i32) -> Result<Option<Bridge>, DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let bridge = bridges::table
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq("telegram"))
            .first::<Bridge>(&mut conn)
            .optional()?;

        Ok(bridge)
    }

    pub fn get_active_whatsapp_connection(&self, user_id: i32) -> Result<Option<Bridge>, DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let bridge = bridges::table
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq("whatsapp"))
            .filter(bridges::status.eq("connected"))
            .first::<Bridge>(&mut conn)
            .optional()?;

        Ok(bridge)
    }

    pub fn get_active_telegram_connection(&self, user_id: i32) -> Result<Option<Bridge>, DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let bridge = bridges::table
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq("telegram"))
            .filter(bridges::status.eq("connected"))
            .first::<Bridge>(&mut conn)
            .optional()?;

        Ok(bridge)
    }
    
    pub fn get_users_with_matrix_bridge_connections(&self) -> Result<Vec<i32>, DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Get distinct user_ids that have at least one bridge connection
        let user_ids = bridges::table
            .select(bridges::user_id)
            .distinct()
            .load::<i32>(&mut conn)?;
            
        Ok(user_ids)
    }

    pub fn get_user_by_matrix_user_id(&self, matrix_user_id: &str) -> Result<Option<User>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let user = users::table
            .filter(users::matrix_username.eq(matrix_user_id))
            .first::<User>(&mut conn)
            .optional()?;
            
        Ok(user)
    }


    // Mark an email as processed
    pub fn mark_email_as_processed(&self, user_id: i32, email_uid: &str) -> Result<(), DieselError> {
        use crate::schema::processed_emails;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // First check if the email is already processed
        let already_processed = self.is_email_processed(user_id, email_uid)?;
        if already_processed {
            tracing::debug!("Email {} for user {} is already marked as processed", email_uid, user_id);
            return Ok(());
        }

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_processed_email = crate::models::user_models::NewProcessedEmails {
            user_id,
            email_uid: email_uid.to_string(),
            processed_at: current_time,
        };

        match diesel::insert_into(processed_emails::table)
            .values(&new_processed_email)
            .execute(&mut conn)
        {
            Ok(_) => {
                tracing::debug!("Successfully marked email {} as processed for user {}", email_uid, user_id);
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to mark email {} as processed for user {}: {}", email_uid, user_id, e);
                Err(e)
            }
        }
    }

    // Check if an email is processed
    pub fn is_email_processed(&self, user_id: i32, email_uid: &str) -> Result<bool, DieselError> {
        use crate::schema::processed_emails;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let processed = processed_emails::table
            .filter(processed_emails::user_id.eq(user_id))
            .filter(processed_emails::email_uid.eq(email_uid))
            .first::<crate::models::user_models::ProcessedEmail>(&mut conn)
            .optional()?;

        Ok(processed.is_some())
    }

    // Get all processed emails for a user
    pub fn get_processed_emails(&self, user_id: i32) -> Result<Vec<crate::models::user_models::ProcessedEmail>, DieselError> {
        use crate::schema::processed_emails;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let processed = processed_emails::table
            .filter(processed_emails::user_id.eq(user_id))
            .order_by(processed_emails::processed_at.desc())
            .load::<crate::models::user_models::ProcessedEmail>(&mut conn)?;

        Ok(processed)
    }

    // Delete a single processed email record
    pub fn delete_processed_email(&self, user_id: i32, email_uid: &str) -> Result<(), DieselError> {
        use crate::schema::processed_emails;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(processed_emails::table)
            .filter(processed_emails::user_id.eq(user_id))
            .filter(processed_emails::email_uid.eq(email_uid))
            .execute(&mut conn)?;

        Ok(())
    }

    // Create a new email judgment
    pub fn create_email_judgment(&self, new_judgment: &crate::models::user_models::NewEmailJudgment) -> Result<(), DieselError> {
        use crate::schema::email_judgments;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::insert_into(email_judgments::table)
            .values(new_judgment)
            .execute(&mut conn)?;

        Ok(())
    }

    // Delete email judgments older than 30 days
    pub fn delete_old_email_judgments(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::email_judgments;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Calculate timestamp for 30 days ago
        let thirty_days_ago = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32 - (30 * 24 * 60 * 60); // 30 days in seconds

        diesel::delete(email_judgments::table)
            .filter(email_judgments::user_id.eq(user_id))
            .filter(email_judgments::processed_at.lt(thirty_days_ago))
            .execute(&mut conn)?;

        Ok(())
    }

    // Get all email judgments for a specific user
    pub fn get_user_email_judgments(&self, user_id: i32) -> Result<Vec<crate::models::user_models::EmailJudgment>, DieselError> {
        use crate::schema::email_judgments;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let judgments = email_judgments::table
            .filter(email_judgments::user_id.eq(user_id))
            .order_by(email_judgments::processed_at.desc())
            .load::<crate::models::user_models::EmailJudgment>(&mut conn)?;

        Ok(judgments)
    }

    // Clean up old calendar notifications
    pub fn cleanup_old_calendar_notifications(&self, older_than_timestamp: i32) -> Result<(), DieselError> {
        use crate::schema::calendar_notifications;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(calendar_notifications::table)
            .filter(calendar_notifications::notification_time.lt(older_than_timestamp))
            .execute(&mut conn)?;

        Ok(())
    }

    // Create a new calendar notification
    pub fn create_calendar_notification(&self, new_notification: &crate::models::user_models::NewCalendarNotification) -> Result<(), DieselError> {
        use crate::schema::calendar_notifications;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::insert_into(calendar_notifications::table)
            .values(new_notification)
            .execute(&mut conn)?;

        Ok(())
    }

    // Check if a calendar notification exists
    pub fn check_calendar_notification_exists(&self, user_id: i32, event_id: &str) -> Result<bool, DieselError> {
        use crate::schema::calendar_notifications;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let count = calendar_notifications::table
            .filter(calendar_notifications::user_id.eq(user_id))
            .filter(calendar_notifications::event_id.eq(event_id))
            .count()
            .get_result::<i64>(&mut conn)?;

        Ok(count > 0)
    }


    pub fn create_google_calendar_connection(
        &self,
        user_id: i32,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: i32,
    ) -> Result<(), DieselError> {
        use crate::schema::google_calendar;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let encrypted_access_token = encrypt(access_token)
            .map_err(|_| DieselError::RollbackTransaction)?;
        let encrypted_refresh_token = refresh_token
            .map(|token| encrypt(token))
            .transpose()
            .map_err(|_| DieselError::RollbackTransaction)?
            .unwrap_or_default();

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_connection = NewGoogleCalendar {
            user_id,
            encrypted_access_token,
            encrypted_refresh_token,
            expires_in,
            status: "active".to_string(),
            last_update: current_time,
            created_on: current_time,
            description: "Google Calendar Connection".to_string(),
        };

        // First, delete any existing connections for this user
        diesel::delete(google_calendar::table)
            .filter(google_calendar::user_id.eq(user_id))
            .execute(&mut conn)?;

        // Then insert the new connection
        diesel::insert_into(google_calendar::table)
            .values(&new_connection)
            .execute(&mut conn)?;

        println!("Successfully created google calendar connection");
        Ok(())
    }
}
