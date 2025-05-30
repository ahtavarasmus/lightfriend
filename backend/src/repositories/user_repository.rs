use diesel::prelude::*;
use diesel::sql_types::Text;
use serde::Serialize;
use diesel::result::Error as DieselError;
use std::error::Error;
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
    pool: DbPool
}

impl UserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
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

        // Get encryption key from environment
        let encryption_key = std::env::var("ENCRYPTION_KEY")
            .expect("ENCRYPTION_KEY must be set");

        use magic_crypt::MagicCryptTrait;
        let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);

        // Encrypt password
        let encrypted_password = cipher.encrypt_str_to_base64(password);

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

        // Get encryption key from environment
        let encryption_key = std::env::var("ENCRYPTION_KEY")
            .expect("ENCRYPTION_KEY must be set");

        // Get the active IMAP connection for the user
        let imap_conn = imap_connection::table
            .filter(imap_connection::user_id.eq(user_id))
            .filter(imap_connection::status.eq("active"))
            .first::<crate::models::user_models::ImapConnection>(&mut conn)
            .optional()?;

        if let Some(conn) = imap_conn {
            use magic_crypt::MagicCryptTrait;
            let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);

            // Decrypt the password
            match cipher.decrypt_base64_to_string(&conn.encrypted_password) {
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
    

    pub fn set_preferred_number_to_default(&self, user_id: i32, phone_number: &str) -> Result<String, Box<dyn Error>> {
        // Get all Twilio phone numbers from environment
        let phone_numbers = [
            ("FIN_PHONE", "+358"),
            ("USA_PHONE", "+1"),
            ("AUS_PHONE", "+61"),
            ("GB_PHONE", "+44"),
        ];

        // Collect phone numbers into a HashMap for easier matching
        let mut number_map = std::collections::HashMap::new();
        for (env_key, prefix) in phone_numbers {
            if let Ok(number) = std::env::var(env_key) {
                number_map.insert(prefix, number);
            }
        }

        // Validate phone number format
        if !phone_number.starts_with('+') {
            return Err("Invalid phone number format".into());
        }

        // Find the matching Twilio number based on the country code prefix
        let preferred_number = phone_numbers.iter()
            .find(|(_, prefix)| phone_number.starts_with(prefix))
            .and_then(|(env_key, _)| std::env::var(env_key).ok())
            .unwrap_or_else(|| {
                // If no match is found, use the country code from the phone number to find a match
                number_map
                    .get("+358") // Default to Finnish number if no match
                    .expect("FIN_PHONE not set")
                    .clone()
            });

        // Update the user's preferred number in the database
        self.update_preferred_number(user_id, &preferred_number)?;

        Ok(preferred_number)
    }

    // Check if a email exists
    pub fn email_exists(&self, search_email: &str) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let existing_user: Option<User> = users::table
            .filter(lower(users::email).eq(lower(search_email)))
            .first::<User>(&mut conn)
            .optional()?;
        Ok(existing_user.is_some())
    }

    // Check if a phone number exists
    pub fn phone_number_exists(&self, search_phone: &str) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let existing_user: Option<User> = users::table
            .filter(users::phone_number.eq(search_phone))
            .first::<User>(&mut conn)
            .optional()?;
        Ok(existing_user.is_some())
    }

    pub fn update_preferred_number(&self, user_id: i32, preferred_number: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::preferred_number.eq(preferred_number))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_agent_language(&self, user_id: i32, language: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::agent_language.eq(language))
            .execute(&mut conn)?;
        Ok(())
    }


    // Create and insert a new user
    pub fn create_user(&self, new_user: NewUser) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(users::table)
            .values(&new_user)
            .execute(&mut conn)?;
        Ok(())
    }

    // Find a user by email
    pub fn find_by_email(&self, search_email: &str) -> Result<Option<User>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table
            .filter(lower(users::email).eq(lower(search_email)))
            .first::<User>(&mut conn)
            .optional()?;
        Ok(user)
    }

    // Get all users
    pub fn get_all_users(&self) -> Result<Vec<User>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let users_list = users::table
            .load::<User>(&mut conn)?;
        Ok(users_list)
    }


    // Check if a user is an admin (email is 'rasmus')
    pub fn is_admin(&self, user_id: i32) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table
            .find(user_id)
            .first::<User>(&mut conn)?;

        Ok(user.email == "rasmus@ahtava.com")
     }

    
    // Find a user by ID
    pub fn find_by_id(&self, user_id: i32) -> Result<Option<User>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table
            .find(user_id)
            .first::<User>(&mut conn)
            .optional()?;
        Ok(user)
    }
    // Update user's profile
    pub fn update_profile(&self, user_id: i32, email: &str, phone_number: &str, nickname: &str, info: &str, timezone: &str, timezone_auto: &bool) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Check if phone number exists for a different user
        let existing_phone = users::table
            .filter(users::phone_number.eq(phone_number))
            .filter(users::id.ne(user_id))
            .first::<User>(&mut conn)
            .optional()?;
            
        if existing_phone.is_some() {
            return Err(DieselError::RollbackTransaction);
        }

        // Check if email exists for a different user
        let existing_email = users::table
            .filter(users::email.eq(email))
            .filter(users::id.ne(user_id))
            .first::<User>(&mut conn)
            .optional()?;
            
        if existing_email.is_some() {
            return Err(DieselError::NotFound);
        }

        // Get current user to check if phone number is changing
        let current_user = users::table
            .find(user_id)
            .first::<User>(&mut conn)?;

        // If phone number is changing, set verified to false
        let should_unverify = current_user.phone_number != phone_number;

        diesel::update(users::table.find(user_id))
            .set((
                users::email.eq(email),
                users::phone_number.eq(phone_number),
                users::nickname.eq(nickname),
                users::info.eq(info),
                users::timezone.eq(timezone),
                users::timezone_auto.eq(timezone_auto),
                users::verified.eq(!should_unverify && current_user.verified), // Only keep verified true if phone number hasn't changed
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn find_by_phone_number(&self, phone_number: &str) -> Result<Option<User>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let cleaned_phone = phone_number
            .chars()
            .filter(|c| c.is_digit(10) || *c == '+')
            .collect::<String>();
        println!("cleaned_phone: {}", cleaned_phone);
        let user = users::table
            .filter(users::phone_number.eq(cleaned_phone))
            .first::<User>(&mut conn)
            .optional()?;
        Ok(user)
    }

    // Set user as verified
    pub fn verify_user(&self, user_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::verified.eq(true))
            .execute(&mut conn)?;
        Ok(())
    }

    // Delete a user
    pub fn delete_user(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::{usage_logs, conversations, google_calendar, gmail, unipile_connection};
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Start transaction
        conn.transaction(|conn| {
            // Delete related records first
            diesel::delete(usage_logs::table.filter(usage_logs::user_id.eq(user_id)))
                .execute(conn)?;
            
            diesel::delete(conversations::table.filter(conversations::user_id.eq(user_id)))
                .execute(conn)?;
            
            diesel::delete(google_calendar::table.filter(google_calendar::user_id.eq(user_id)))
                .execute(conn)?;
            
            diesel::delete(gmail::table.filter(gmail::user_id.eq(user_id)))
                .execute(conn)?;
            
            diesel::delete(unipile_connection::table.filter(unipile_connection::user_id.eq(user_id)))
                .execute(conn)?;

            // Finally delete the user
            diesel::delete(users::table.find(user_id))
                .execute(conn)?;
            
            Ok(())
        })
    }

    // Update user's (credits)
    pub fn update_user_credits(&self, user_id: i32, new_credits: f32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::credits.eq(new_credits))
            .execute(&mut conn)?;
        Ok(())
    }


    pub fn update_user_credits_left(&self, user_id: i32, new_credits_left: f32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::credits_left.eq(new_credits_left))
            .execute(&mut conn)?;
        Ok(())
    }

    // Decrease user's credits by a specified amount
    pub fn decrease_credits(&self, user_id: i32, amount: f32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table
            .find(user_id)
            .first::<User>(&mut conn)?;
        
        let new_credits = user.credits - amount;
        
        diesel::update(users::table.find(user_id))
            .set(users::credits.eq(new_credits))
            .execute(&mut conn)?;
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

    // Increase user's credits by a specified amount
    pub fn increase_credits(&self, user_id: i32, amount: f32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let user = users::table
            .find(user_id)
            .first::<User>(&mut conn)?;
        

        let new_credits = user.credits + amount;
        
        diesel::update(users::table.find(user_id))
            .set(users::credits.eq(new_credits))
            .execute(&mut conn)?;

        // get the user again with updated credits
        let user = users::table
            .find(user_id)
            .first::<User>(&mut conn)?;


        // get any ongoing usage log for this user
        let ongoing_log = usage_logs::table
            .filter(usage_logs::user_id.eq(user_id))
            .filter(usage_logs::status.eq("ongoing"))
            .first::<crate::models::user_models::UsageLog>(&mut conn)
            .optional()?;
        
        // If there's an ongoing log, update its timestamps
        if let Some(log) = ongoing_log {
            if let Some(sid) = log.sid {

                let charge_back_threshold= std::env::var("CHARGE_BACK_THRESHOLD")
                    .expect("CHARGE_BACK_THRESHOLD not set")
                    .parse::<f32>()
                    .unwrap_or(2.00);
                let voice_second_cost = std::env::var("VOICE_SECOND_COST")
                    .expect("VOICE_SECOND_COST not set")
                    .parse::<f32>()
                    .unwrap_or(0.0033);

                let user_current_credits_to_threshold = user.credits - charge_back_threshold;

                let seconds_to_threshold = (user_current_credits_to_threshold / voice_second_cost) as i32;
                let recharge_threshold_timestamp: i32 = (chrono::Utc::now().timestamp() as i32) + seconds_to_threshold as i32;

                let seconds_to_zero_credits= (user.credits / voice_second_cost) as i32;
                let zero_credits_timestamp: i32 = (chrono::Utc::now().timestamp() as i32) + seconds_to_zero_credits as i32;

                diesel::update(usage_logs::table)
                    .filter(usage_logs::sid.eq(sid))
                    .set((
                        usage_logs::recharge_threshold_timestamp.eq(recharge_threshold_timestamp),
                        usage_logs::zero_credits_timestamp.eq(zero_credits_timestamp), 
                    ))
                    .execute(&mut conn)?;
            }
        }

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

    // 

    // Update user's notify preference
    pub fn update_notify(&self, user_id: i32, notify: bool) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::notify.eq(notify))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_timezone(&self, user_id: i32, timezone: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // First fetch the user to check timezone_auto
        let user = users::table
            .find(user_id)
            .first::<User>(&mut conn)?;
            
        // Only update if timezone_auto is true
        match user.timezone_auto {
            Some(maybe) => {
                if maybe {
                    diesel::update(users::table.find(user_id))
                        .set(users::timezone.eq(timezone.to_string()))
                        .execute(&mut conn)?;
                }
            },
            None => {},
        }
        
        Ok(())
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

    // Update user's auto top-up settings
    pub fn update_auto_topup(&self, user_id: i32, active: bool, amount: Option<f32>) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Update the user's auto top-up settings
        diesel::update(users::table.find(user_id))
            .set((
                users::charge_when_under.eq(active),
                users::charge_back_to.eq(amount),
            ))
            .execute(&mut conn)?;
            
        Ok(())
    }

    pub fn get_stripe_customer_id(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let stripe_id = users::table
            .find(user_id)
            .select(users::stripe_customer_id)
            .first::<Option<String>>(&mut conn)?;
        Ok(stripe_id)
    }

    pub fn set_stripe_customer_id(&self, user_id: i32, customer_id: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::stripe_customer_id.eq(customer_id))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_stripe_payment_method_id(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let payment_method_id = users::table
            .find(user_id)
            .select(users::stripe_payment_method_id)
            .first::<Option<String>>(&mut conn)?;
        Ok(payment_method_id)
    }
    
    pub fn set_stripe_payment_method_id(&self, user_id: i32, payment_method_id: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::stripe_payment_method_id.eq(payment_method_id))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn find_by_stripe_customer_id(&self, customer_id: &str) -> Result<Option<User>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table
            .filter(users::stripe_customer_id.eq(customer_id))
            .first::<User>(&mut conn)
            .optional()?;
        Ok(user)
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

    pub fn set_stripe_checkout_session_id(&self, user_id: i32, session_id: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::stripe_checkout_session_id.eq(session_id))
            .execute(&mut conn)?;
        Ok(())
    }
    
    pub fn get_stripe_checkout_session_id(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let session_id = users::table
            .find(user_id)
            .select(users::stripe_checkout_session_id)
            .first::<Option<String>>(&mut conn)?;
        Ok(session_id)
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

    pub fn set_confirm_send_event(&self, user_id: i32, confirm: bool) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::confirm_send_event.eq(confirm))
            .execute(&mut conn)?;
        Ok(())
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


    pub fn update_last_credits_notification(&self, user_id: i32, timestamp: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::last_credits_notification.eq(timestamp))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn has_auto_topup_enabled(&self, user_id: i32) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let charge_when_under = users::table
            .find(user_id)
            .select(users::charge_when_under)
            .first::<bool>(&mut conn)?;
        Ok(charge_when_under)
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

    // Get the user's subscription tier
    pub fn get_subscription_tier(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let sub_tier = users::table
            .find(user_id)
            .select(users::sub_tier)
            .first::<Option<String>>(&mut conn)?;
        Ok(sub_tier)
    }

    // Set the user's subscription tier
    pub fn update_proactive_messages_left(&self, user_id: i32, new_count: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::msgs_left.eq(new_count))
            .execute(&mut conn)?;
        Ok(())
    }
    pub fn update_sub_credits(&self, user_id: i32, new_credits: f32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::credits_left.eq(new_credits))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn set_subscription_tier(&self, user_id: i32, tier: Option<&str>) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::sub_tier.eq(tier))
            .execute(&mut conn)?;
        Ok(())
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

    // Check if user has a specific subscription tier and has messages left
    pub fn has_valid_subscription_tier_with_messages(&self, user_id: i32, tier: &str) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table
            .find(user_id)
            .select((users::sub_tier, users::msgs_left))
            .first::<(Option<String>, i32)>(&mut conn)?;
        
        // Check both subscription tier and remaining messages
        Ok(user.0.map_or(false, |t| t == tier) && user.1 > 0)
    }

    pub fn decrease_messages_left(&self, user_id: i32) -> Result<i32, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Get current messages left
        let current_msgs = users::table
            .find(user_id)
            .select(users::msgs_left)
            .first::<i32>(&mut conn)?;
            
        // Only decrease if greater than 0
        if current_msgs > 0 {
            let new_msgs = current_msgs - 1;
            diesel::update(users::table.find(user_id))
                .set(users::msgs_left.eq(new_msgs))
                .execute(&mut conn)?;
            Ok(new_msgs)
        } else {
            Ok(0)
        }
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
            // Get encryption key from environment
            let encryption_key = match std::env::var("ENCRYPTION_KEY") {
                Ok(key) => key,
                Err(_) => {
                    tracing::error!("ENCRYPTION_KEY not set in environment");
                    return Err(DieselError::RollbackTransaction);
                }
            };

            use magic_crypt::MagicCryptTrait;
            let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);

            // Log the encrypted tokens for debugging
            tracing::debug!(
                "Attempting to decrypt tokens - Access token length: {}, Refresh token length: {}", 
                connection.encrypted_access_token.len(),
                connection.encrypted_refresh_token.len()
            );

            // Decrypt access token
            let access_token = match cipher.decrypt_base64_to_string(&connection.encrypted_access_token) {
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
            let refresh_token = match cipher.decrypt_base64_to_string(&connection.encrypted_refresh_token) {
                Ok(token) => {
                    tracing::debug!("Successfully decrypted refresh token");
                    token
                },
                Err(e) => {
                    tracing::error!("Failed to decrypt refresh token: {:?}", e);
                    return Err(DieselError::RollbackTransaction);
                }
            };

            tracing::debug!("Successfully decrypted both calendar tokens for user {}", user_id);
            Ok(Some((access_token, refresh_token)))
        } else {
            tracing::debug!("No active calendar connection found for user {}", user_id);
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

        // Get encryption key from environment
        let encryption_key = std::env::var("ENCRYPTION_KEY")
            .expect("ENCRYPTION_KEY must be set");

        use magic_crypt::MagicCryptTrait;
        let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);

        // Encrypt new access token
        let encrypted_access_token = cipher.encrypt_str_to_base64(new_access_token);

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

    pub fn has_active_gmail(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::gmail;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = gmail::table
            .filter(gmail::user_id.eq(user_id))
            .filter(gmail::status.eq("active"))
            .first::<crate::models::user_models::Gmail>(&mut conn)
            .optional()?;

        Ok(connection.is_some())
    }

    pub fn get_active_gmail_connection_users(&self) -> Result<Vec<i32>, DieselError> {
        use crate::schema::gmail;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let user_ids = gmail::table
            .filter(gmail::status.eq("active"))
            .select(gmail::user_id)
            .load::<i32>(&mut conn)?;

        Ok(user_ids)
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

    pub fn get_gmail_tokens(&self, user_id: i32) -> Result<Option<(String, String)>, DieselError> {
        use crate::schema::gmail;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let connection = gmail::table
            .filter(gmail::user_id.eq(user_id))
            .filter(gmail::status.eq("active"))
            .first::<crate::models::user_models::Gmail>(&mut conn)
            .optional()?;

        if let Some(connection) = connection {
            let encryption_key = match std::env::var("ENCRYPTION_KEY") {
                Ok(key) => key,
                Err(_) => {
                    tracing::error!("ENCRYPTION_KEY not set in environment");
                    return Err(DieselError::RollbackTransaction);
                }
            };

            use magic_crypt::MagicCryptTrait;
            let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);

            let access_token = match cipher.decrypt_base64_to_string(&connection.encrypted_access_token) {
                Ok(token) => token,
                Err(e) => {
                    tracing::error!("Failed to decrypt access token: {:?}", e);
                    return Err(DieselError::RollbackTransaction);
                }
            };

            let refresh_token = match cipher.decrypt_base64_to_string(&connection.encrypted_refresh_token) {
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

    pub fn update_gmail_access_token(
        &self,
        user_id: i32,
        new_access_token: &str,
        expires_in: i32,
    ) -> Result<(), DieselError> {
        use crate::schema::gmail;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let encryption_key = std::env::var("ENCRYPTION_KEY")
            .expect("ENCRYPTION_KEY must be set");

        use magic_crypt::MagicCryptTrait;
        let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);

        let encrypted_access_token = cipher.encrypt_str_to_base64(new_access_token);

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        diesel::update(gmail::table)
            .filter(gmail::user_id.eq(user_id))
            .filter(gmail::status.eq("active"))
            .set((
                gmail::encrypted_access_token.eq(encrypted_access_token),
                gmail::expires_in.eq(expires_in),
                gmail::last_update.eq(current_time),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn delete_gmail_connection(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::gmail;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(gmail::table)
            .filter(gmail::user_id.eq(user_id))
            .execute(&mut conn)?;

        Ok(())
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
            let encryption_key = match std::env::var("ENCRYPTION_KEY") {
                Ok(key) => key,
                Err(_) => {
                    tracing::error!("ENCRYPTION_KEY not set in environment");
                    return Err(DieselError::RollbackTransaction);
                }
            };

            use magic_crypt::MagicCryptTrait;
            let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);

            let access_token = match cipher.decrypt_base64_to_string(&connection.encrypted_access_token) {
                Ok(token) => token,
                Err(e) => {
                    tracing::error!("Failed to decrypt access token: {:?}", e);
                    return Err(DieselError::RollbackTransaction);
                }
            };

            let refresh_token = match cipher.decrypt_base64_to_string(&connection.encrypted_refresh_token) {
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

        let encryption_key = std::env::var("ENCRYPTION_KEY")
            .expect("ENCRYPTION_KEY must be set");

        use magic_crypt::MagicCryptTrait;
        let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);

        let encrypted_access_token = cipher.encrypt_str_to_base64(new_access_token);

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

        let encryption_key = std::env::var("ENCRYPTION_KEY")
            .expect("ENCRYPTION_KEY must be set");

        use magic_crypt::MagicCryptTrait;
        let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);

        let encrypted_access_token = cipher.encrypt_str_to_base64(access_token);
        let encrypted_refresh_token = refresh_token
            .map(|token| cipher.encrypt_str_to_base64(token))
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
        let encrypted_token = crate::utils::matrix_auth::encrypt_token(access_token)
            .map_err(|_| DieselError::RollbackTransaction)?;

        // Encrypt the password before storing
        let encrypted_password = crate::utils::matrix_auth::encrypt_token(password)
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
        let encrypted_token = crate::utils::matrix_auth::encrypt_token(access_token)
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
        let encrypted_key= crate::utils::matrix_auth::encrypt_token(recovery_key)
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
                let rec_key= crate::utils::matrix_auth::decrypt_token(&key)
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
                let token = crate::utils::matrix_auth::decrypt_token(&encrypted_token)
                    .map_err(|_| DieselError::RollbackTransaction)?;
                let password= crate::utils::matrix_auth::decrypt_token(&encrypted_password)
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

    pub fn create_gmail_connection(
        &self,
        user_id: i32,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: i32,
    ) -> Result<(), DieselError> {
        use crate::schema::gmail;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let encryption_key = std::env::var("ENCRYPTION_KEY")
            .expect("ENCRYPTION_KEY must be set");

        use magic_crypt::MagicCryptTrait;
        let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);

        let encrypted_access_token = cipher.encrypt_str_to_base64(access_token);
        let encrypted_refresh_token = refresh_token
            .map(|token| cipher.encrypt_str_to_base64(token))
            .unwrap_or_default();

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        use crate::models::user_models::NewGmail;

        let new_connection = NewGmail {
            user_id,
            encrypted_access_token,
            encrypted_refresh_token,
            expires_in,
            status: "active".to_string(),
            last_update: current_time,
            created_on: current_time,
            description: "Gmail Connection".to_string(),
        };

        // First, delete any existing connections for this user
        diesel::delete(gmail::table)
            .filter(gmail::user_id.eq(user_id))
            .execute(&mut conn)?;

        // Then insert the new connection
        diesel::insert_into(gmail::table)
            .values(&new_connection)
            .execute(&mut conn)?;

        Ok(())
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

        println!("Creating google calendar connection for user: {}", user_id);
        println!("Got refresh token: {:?}", refresh_token);

        // Get encryption key from environment
        let encryption_key = std::env::var("ENCRYPTION_KEY")
            .expect("ENCRYPTION_KEY must be set");

        use magic_crypt::MagicCryptTrait;
        // Create encryption cipher
        let cipher = magic_crypt::new_magic_crypt!(encryption_key, 256);

        // Encrypt tokens
        let encrypted_access_token = cipher.encrypt_str_to_base64(access_token);
        let encrypted_refresh_token = refresh_token
            .map(|token| cipher.encrypt_str_to_base64(token))
            .unwrap_or_default();

        println!("Encrypted refresh token: {}", encrypted_refresh_token);

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
