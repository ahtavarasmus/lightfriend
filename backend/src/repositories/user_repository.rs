use diesel::prelude::*;
use diesel::sql_types::Text;
use serde::Serialize;
use diesel::result::Error as DieselError;
use std::error::Error;
use crate::utils::encryption::{encrypt, decrypt};
use rand;

#[derive(Serialize, PartialEq)]
pub struct UsageDataPoint {
    pub timestamp: i32,
    pub credits: f32,
}

use crate::{
    models::user_models::{User, NewUsageLog, NewGoogleCalendar, 
        NewImapConnection, Bridge, NewBridge, WaitingCheck, 
        NewWaitingCheck, PrioritySender, NewPrioritySender, Keyword, 
        NewKeyword, NewGoogleTasks,
        TaskNotification, NewTaskNotification, Uber, NewUber,
    },
    schema::{
        users, usage_logs, 
        waiting_checks, priority_senders, keywords, 
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
        limit: i64,
        include_tools: bool,
    ) -> Result<Vec<crate::models::user_models::MessageHistory>, diesel::result::Error> {
        use crate::schema::message_history;
        use diesel::prelude::*;
        use crate::utils::encryption;

        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let user_messages: Vec<crate::models::user_models::MessageHistory>;
        // First, get the user messages to establish time boundaries
        if include_tools {
            user_messages = message_history::table
                .filter(message_history::user_id.eq(user_id))
                .filter(message_history::role.eq("user"))
                .order_by(message_history::created_at.desc())
                .limit(limit)
                .load::<crate::models::user_models::MessageHistory>(&mut conn)?;
        } else {
            user_messages = message_history::table
                .filter(message_history::user_id.eq(user_id))
                .filter(message_history::role.ne("tool"))
                .filter(message_history::role.eq("user"))
                .order_by(message_history::created_at.desc())
                .limit(limit)
                .load::<crate::models::user_models::MessageHistory>(&mut conn)?;
        }

        if user_messages.is_empty() {
            return Ok(Vec::new());
        }

        // Get the timestamp of the oldest user message
        let oldest_timestamp = user_messages.last().map(|msg| msg.created_at).unwrap_or(0);

        // Now get all messages from the oldest user message onwards
        let encrypted_messages = message_history::table
            .filter(message_history::user_id.eq(user_id))
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
            tool_calls_json: new_message.tool_calls_json.clone(),
            created_at: new_message.created_at,
            conversation_id: new_message.conversation_id.clone(),
        };

        diesel::insert_into(message_history::table)
            .values(&encrypted_message)
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

    pub fn delete_old_message_history(&self, user_id: i32, save_context_limit: i64) -> Result<usize, diesel::result::Error> {
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

                    base_delete.execute(conn)
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

    // log the usage. activity_type either 'call' or 'sms', or the new 'notification'
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
        
        if service_type == "email" {
            diesel::delete(waiting_checks::table)
                .filter(waiting_checks::user_id.eq(user_id))
                .filter(waiting_checks::service_type.eq("imap").or(waiting_checks::service_type.eq("email")))
                .filter(waiting_checks::content.eq(content))
                .execute(&mut conn)?;
        } else if service_type == "messaging" {
            diesel::delete(waiting_checks::table)
                .filter(waiting_checks::user_id.eq(user_id))
                .filter(waiting_checks::service_type.eq("messaging").or(waiting_checks::service_type.eq("whatsapp")))
                .filter(waiting_checks::content.eq(content))
                .execute(&mut conn)?;
        } else {
            diesel::delete(waiting_checks::table)
                .filter(waiting_checks::user_id.eq(user_id))
                .filter(waiting_checks::service_type.eq(service_type))
                .filter(waiting_checks::content.eq(content))
                .execute(&mut conn)?;
        }
        Ok(())
    }

    pub fn delete_waiting_check_by_id(&self, user_id: i32, id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(waiting_checks::table)
            .filter(waiting_checks::user_id.eq(user_id))
            .filter(waiting_checks::id.eq(id))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_waiting_checks_all(&self, user_id: i32) -> Result<Vec<WaitingCheck>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let mut checks = waiting_checks::table
            .filter(waiting_checks::user_id.eq(user_id))
            .load::<WaitingCheck>(&mut conn)?;

        // Update service types
        for check in &mut checks {
            if check.service_type == "whatsapp" {
                check.service_type = "messaging".to_string();
            } else if check.service_type == "imap" {
                check.service_type = "email".to_string();
            }
        }

        Ok(checks)
    }

    pub fn get_waiting_checks(&self, user_id: i32, service_type: &str) -> Result<Vec<WaitingCheck>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        if service_type == "email" {
            waiting_checks::table
                .filter(waiting_checks::user_id.eq(user_id))
                .filter(waiting_checks::service_type.eq("imap").or(waiting_checks::service_type.eq("email")))
                .load::<WaitingCheck>(&mut conn)
        } else if service_type == "messaging" {
            waiting_checks::table
                .filter(waiting_checks::user_id.eq(user_id))
                .filter(waiting_checks::service_type.eq("messaging").or(waiting_checks::service_type.eq("whatsapp")))
                .load::<WaitingCheck>(&mut conn)
        } else {
            waiting_checks::table
                .filter(waiting_checks::user_id.eq(user_id))
                .filter(waiting_checks::service_type.eq(service_type))
                .load::<WaitingCheck>(&mut conn)
        }
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

    pub fn get_priority_senders_all(&self, user_id: i32) -> Result<Vec<PrioritySender>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        priority_senders::table
            .filter(priority_senders::user_id.eq(user_id))
            .load::<PrioritySender>(&mut conn)
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

    pub fn has_active_uber(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::uber;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let connection = uber::table
            .filter(uber::user_id.eq(user_id))
            .filter(uber::status.eq("active"))
            .first::<crate::models::user_models::Uber>(&mut conn)
            .optional()?;
        Ok(connection.is_some())
    }

    pub fn create_uber_connection(
        &self,
        user_id: i32,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: i32,
    ) -> Result<(), DieselError> {
        use crate::schema::uber;
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
        let new_connection = NewUber {
            user_id,
            encrypted_access_token,
            encrypted_refresh_token,
            expires_in,
            status: "active".to_string(),
            last_update: current_time,
            created_on: current_time,
            description: "Uber Connection".to_string(),
        };
        // First, delete any existing connections for this user
        diesel::delete(uber::table)
            .filter(uber::user_id.eq(user_id))
            .execute(&mut conn)?;
        // Then insert the new connection
        diesel::insert_into(uber::table)
            .values(&new_connection)
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_uber_tokens(&self, user_id: i32) -> Result<Option<(String, String)>, DieselError> {
        use crate::schema::uber;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let connection = uber::table
            .filter(uber::user_id.eq(user_id))
            .filter(uber::status.eq("active"))
            .first::<crate::models::user_models::Uber>(&mut conn)
            .optional()?;
        if let Some(connection) = connection {
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
            Ok(Some((access_token, refresh_token)))
        } else {
            tracing::info!("No active Uber connection found for user {}", user_id);
            Ok(None)
        }
    }

    pub fn delete_uber_connection(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::uber;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(uber::table)
            .filter(uber::user_id.eq(user_id))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_uber_token_info(&self, user_id: i32) -> Result<Option<(String, String, i32, i32)>, DieselError> {
        use crate::schema::uber;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let connection = uber::table
            .filter(uber::user_id.eq(user_id))
            .filter(uber::status.eq("active"))
            .first::<crate::models::user_models::Uber>(&mut conn)
            .optional()?;
        if let Some(connection) = connection {
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
            Ok(Some((access_token, refresh_token, connection.expires_in, connection.last_update)))
        } else {
            tracing::info!("No active Uber connection found for user {}", user_id);
            Ok(None)
        }
    }

    pub fn update_uber_access_token(
        &self,
        user_id: i32,
        new_access_token: &str,
        expires_in: i32,
    ) -> Result<(), DieselError> {
        use crate::schema::uber;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let encrypted_access_token = encrypt(new_access_token)
            .map_err(|_| DieselError::RollbackTransaction)?;
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;
        diesel::update(uber::table)
            .filter(uber::user_id.eq(user_id))
            .filter(uber::status.eq("active"))
            .set((
                uber::encrypted_access_token.eq(encrypted_access_token),
                uber::expires_in.eq(expires_in),
                uber::last_update.eq(current_time),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_uber_refresh_token(
        &self,
        user_id: i32,
        new_refresh_token: &str,
    ) -> Result<(), DieselError> {
        use crate::schema::uber;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let encrypted_refresh_token = encrypt(new_refresh_token)
            .map_err(|_| DieselError::RollbackTransaction)?;
        diesel::update(uber::table)
            .filter(uber::user_id.eq(user_id))
            .filter(uber::status.eq("active"))
            .set(
                uber::encrypted_refresh_token.eq(encrypted_refresh_token),
            )
            .execute(&mut conn)?;
        Ok(())
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

    pub fn create_bridge(&self, new_bridge: NewBridge) -> Result<(), DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::insert_into(bridges::table)
            .values(&new_bridge)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn delete_bridge(&self, user_id: i32, service: &str) -> Result<(), DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(bridges::table)
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq(service))
            .execute(&mut conn)?;

        Ok(())
    }


    pub fn get_bridge(&self, user_id: i32, service: &str) -> Result<Option<Bridge>, DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let bridge = bridges::table
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq(service))
            .first::<Bridge>(&mut conn)
            .optional()?;

        Ok(bridge)
    }

    pub fn has_active_bridges(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = bridges::table
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::status.eq("connected"))
            .count()
            .get_result::<i64>(&mut conn)?;
        Ok(count > 0)
    }

    pub fn get_active_signal_connection(&self, user_id: i32) -> Result<Option<Bridge>, DieselError> {
        use crate::schema::bridges;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let bridge = bridges::table
            .filter(bridges::user_id.eq(user_id))
            .filter(bridges::bridge_type.eq("signal"))
            .filter(bridges::status.eq("connected"))
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

        let new_processed_email = crate::models::user_models::NewProcessedEmail {
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
