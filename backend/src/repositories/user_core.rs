use diesel::prelude::*;
use diesel::sql_types::Text;
use diesel::result::Error as DieselError;
use std::error::Error;
use crate::{
    models::user_models::{User, UserSettings, UserInfo, NewUserInfo, NewUserSettings, TempVariable, NewTempVariable},
    schema::{users, user_settings, temp_variables, user_info},
    DbPool,
};

use diesel::dsl::sql;
use diesel::sql_types::BigInt;
use std::time::{SystemTime, UNIX_EPOCH};

sql_function! {
    fn lower(x: Text) -> Text;
}

pub struct UserCore {
    pool: DbPool
}

impl UserCore {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    // Core user operations
    pub fn create_user(&self, new_user: crate::handlers::auth_dtos::NewUser) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(users::table)
            .values(&new_user)
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_user(&self) -> Result<Option<User>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table
            .find(1)
            .first::<User>(&mut conn)
            .optional()?;
        Ok(user)
    }

    pub fn find_by_phone_number(&self, phone_number: &str) -> Result<Option<User>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let cleaned_phone = phone_number
            .chars()
            .filter(|c| c.is_digit(10) || *c == '+')
            .collect::<String>();
        let user = users::table
            .filter(users::phone_number.eq(cleaned_phone))
            .first::<User>(&mut conn)
            .optional()?;
        Ok(user)
    }

    pub fn update_phone_number_country(&self, country: Option<&str>) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table)
            .filter(users::id.eq(1))
            .set(users::phone_number_country.eq(country))
            .execute(&mut conn)?;
        Ok(())
    }

    // Helper function to ensure user_info exists
    pub fn ensure_user_info_exists(&self) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let exists = user_info::table
            .filter(user_info::user_id.eq(1))
            .first::<UserInfo>(&mut conn)
            .optional()?
            .is_some();

        let user_id = 1;
        if !exists {
            let new_user_info = NewUserInfo {
                user_id,
                location: None,
                dictionary: None,
                info: None,
                timezone: None,
                nearby_places: None,
                recent_contacts: None,
            };

            diesel::insert_into(user_info::table)
                .values(&new_user_info)
                .execute(&mut conn)?;
        }

        Ok(())
    }

    // Get user_info, ensuring it exists first
    pub fn get_user_info(&self) -> Result<UserInfo, DieselError> {
        self.ensure_user_info_exists()?;

        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let user_info = user_info::table
            .filter(user_info::user_id.eq(1))
            .first::<UserInfo>(&mut conn)?;

        Ok(user_info)
    }


    // User settings operations
    pub fn get_user_settings(&self) -> Result<UserSettings, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings = user_settings::table
            .filter(user_settings::user_id.eq(1))
            .first::<UserSettings>(&mut conn)
            .optional()?;
            
        let user_id = 1;
        match settings {
            Some(settings) => Ok(settings),
            None => {
                let new_settings = NewUserSettings {
                    user_id,
                    notify: true,
                    notification_type: None,
                    timezone_auto: None,
                    agent_language: "en".to_string(),
                    sub_country: None,
                    save_context: Some(5),
                    number_of_digests_locked: 0,
                    critical_enabled: Some("sms".to_string()),
                    proactive_agent_on: true,
                    notify_about_calls: true,
                };
                
                diesel::insert_into(user_settings::table)
                    .values(&new_settings)
                    .execute(&mut conn)?;
                    
                let created_settings = user_settings::table
                    .filter(user_settings::user_id.eq(user_id))
                    .first::<UserSettings>(&mut conn)?;
                    
                Ok(created_settings)
            }
        }
    }

    // Helper function to ensure user settings exist
    pub fn ensure_user_settings_exist(&self) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings_exist = user_settings::table
            .filter(user_settings::user_id.eq(1))
            .first::<UserSettings>(&mut conn)
            .optional()?
            .is_some();

        let user_id = 1;
        if !settings_exist {
            let new_settings = NewUserSettings {
                user_id,
                notify: true,
                notification_type: None,
                timezone_auto: None,
                agent_language: "en".to_string(),
                sub_country: None,
                save_context: Some(5),
                number_of_digests_locked: 0,
                critical_enabled: Some("sms".to_string()),
                proactive_agent_on: true,
                notify_about_calls: true,
            };
            
            diesel::insert_into(user_settings::table)
                .values(&new_settings)
                .execute(&mut conn)?;
        }
        Ok(())
    }

    pub fn update_blocker_password(&self, user_id: i32, new_password: Option<&str>) -> Result<(), DieselError> {
        use crate::schema::user_info;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Ensure user settings exist
        self.ensure_user_info_exists()?;

        // Update  
        diesel::update(user_info::table.filter(user_info::user_id.eq(user_id)))
            .set(user_info::blocker_password_vault.eq(new_password))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_lockbox_password(&self, user_id: i32, new_password: Option<&str>) -> Result<(), DieselError> {
        use crate::schema::user_info;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Ensure user settings exist
        self.ensure_user_info_exists()?;

        // Update  
        diesel::update(user_info::table.filter(user_info::user_id.eq(user_id)))
            .set(user_info::lockbox_password_vault.eq(new_password))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_sub_country(&self, user_id: i32, country: Option<&str>) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Ensure user settings exist
        self.ensure_user_settings_exist()?;

        // Update the settings
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::sub_country.eq(country))
            .execute(&mut conn)?;


        Ok(())
    }

    pub fn update_preferred_number(&self, preferred_number: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(1))
            .set(users::preferred_number.eq(preferred_number))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_agent_language(&self, user_id: i32, language: &str) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::agent_language.eq(language))
            .execute(&mut conn)?;
        Ok(())
    }

    // Update user's profile
    pub fn update_profile(&self, user_id: i32, email: &str, phone_number: &str, nickname: &str, info: &str, timezone: &str, timezone_auto: &bool, notification_type: Option<&str>, save_context: Option<i32>, location: &str, nearby_places: &str) -> Result<(), DieselError> {
        use crate::schema::users;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        println!("Repository: Updating user {} with notification type: {:?}", user_id, notification_type);
       
        // Start a transaction
        conn.transaction(|conn| {
            // Check if phone number exists for a different user
            let existing_phone = users::table
                .filter(users::phone_number.eq(phone_number))
                .filter(users::id.ne(user_id))
                .first::<User>(conn)
                .optional()?;
               
            if existing_phone.is_some() {
                return Err(DieselError::RollbackTransaction);
            }
               
            // Get current user to check if phone number is changing
            let current_user = users::table
                .find(user_id)
                .first::<User>(conn)?;
            // If phone number is changing, set verified to false
            let should_unverify = current_user.phone_number != phone_number;
            // Update user table
            diesel::update(users::table.find(user_id))
                .set((
                    users::phone_number.eq(phone_number),
                    users::nickname.eq(nickname),
                ))
                .execute(conn)?;
            // Ensure user settings exist
            self.ensure_user_settings_exist()?;
            // Ensure user info exists
            self.ensure_user_info_exists()?;
            // Update the settings
            diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
                .set((
                    user_settings::timezone_auto.eq(timezone_auto),
                    user_settings::notification_type.eq(notification_type.map(|s| s.to_string())),
                    user_settings::save_context.eq(save_context),
                ))
                .execute(conn)?;
            // Update user info
            diesel::update(user_info::table.filter(user_info::user_id.eq(user_id)))
                .set((
                    user_info::timezone.eq(timezone),
                    user_info::info.eq(info),
                    user_info::location.eq(location),
                    user_info::nearby_places.eq(nearby_places),
                ))
                .execute(conn)?;
            Ok(())
        })
    }

    // Update user's notify preference in user_settings
    pub fn update_notify(&self, user_id: i32, notify: bool) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;

        // Update the settings
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::notify.eq(notify))
            .execute(&mut conn)?;


        Ok(())
    }

    pub fn update_timezone(&self, user_id: i32, timezone: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // First fetch the user settings to check timezone_auto
        let user_settings= self.get_user_settings()?;
        // Only update if timezone_auto is false (manual timezone setting)
        if !user_settings.timezone_auto.unwrap_or(false) {
            diesel::update(user_info::table)
                .filter(user_info::user_id.eq(user_id))
                .set(user_info::timezone.eq(timezone.to_string()))
                .execute(&mut conn)?;
        }
        
        Ok(())
    }

    pub fn update_digests(&self, user_id: i32, morning_digest: Option<&str>, day_digest: Option<&str>, evening_digest: Option<&str>) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;

        // Update the digest settings
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set((
                user_settings::morning_digest.eq(morning_digest.map(|s| s.to_string())),
                user_settings::day_digest.eq(day_digest.map(|s| s.to_string())),
                user_settings::evening_digest.eq(evening_digest.map(|s| s.to_string())),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_digests(&self) -> Result<(Option<String>, Option<String>, Option<String>), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;

        // Get the user settings
        let settings = user_settings::table
            .filter(user_settings::user_id.eq(1))
            .select((
                user_settings::morning_digest,
                user_settings::day_digest,
                user_settings::evening_digest,
            ))
            .first::<(Option<String>, Option<String>, Option<String>)>(&mut conn)?;

        Ok(settings)
    }
    
    pub fn update_proactive_agent_on(&self, user_id: i32, enabled: bool) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;

        // Update the setting
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::proactive_agent_on.eq(enabled))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_proactive_agent_on(&self) -> Result<bool, DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;

        // Get the setting
        let proactive_agent_on= user_settings::table
            .filter(user_settings::user_id.eq(1))
            .select(user_settings::proactive_agent_on)
            .first::<bool>(&mut conn)?;

        Ok(proactive_agent_on)
    }

    pub fn update_critical_enabled(&self, user_id: i32, enabled: Option<String>) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
      
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;
        // Update the critical_enabled setting
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::critical_enabled.eq(enabled))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_action_on_critical_message(&self, user_id: i32, action: Option<String>) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
      
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;
        // Update the action_on_critical_message setting
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::action_on_critical_message.eq(action))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_call_notify(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;

        // Get the setting
        let proactive_agent_on= user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select(user_settings::notify_about_calls)
            .first::<bool>(&mut conn)?;

        Ok(proactive_agent_on)
    }

    pub fn update_call_notify(&self, user_id: i32, call_notify: bool) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
       
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;
        // Update the call_notify setting
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::notify_about_calls.eq(call_notify))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_critical_notification_info(&self, user_id: i32) -> Result<crate::handlers::profile_handlers::CriticalNotificationInfo, diesel::result::Error> {
        use crate::schema::{user_settings, usage_logs};
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;
        // Get the critical_enabled and call_notify settings
        let (enabled, call_notify, action_on_critical_message) = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select((user_settings::critical_enabled, user_settings::notify_about_calls.nullable(), user_settings::action_on_critical_message))
            .first::<(Option<String>, Option<bool>, Option<String>)>(&mut conn)?;
        let call_notify = call_notify.unwrap_or(true); // Default to true if not set
        // Get average critical notifications per day
        let average_critical_per_day = {
            let now: i64 = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs() as i64;
            let thirty_days_ago: i64 = now - 2_592_000; // 30 * 86_400
            let active_days_count: i64 = usage_logs::table
                .select(sql::<BigInt>("COUNT(DISTINCT created_at / 86400)"))
                .filter(crate::schema::usage_logs::user_id.eq(user_id))
                .filter(usage_logs::created_at.ge(thirty_days_ago as i32))
                .first(&mut conn)?;
            if active_days_count < 3 {
                1.0
            } else {
                let oldest_day: i64 = usage_logs::table
                    .select(sql::<BigInt>("MIN(created_at / 86400)"))
                    .filter(crate::schema::usage_logs::user_id.eq(user_id))
                    .filter(usage_logs::created_at.ge(thirty_days_ago as i32))
                    .first(&mut conn)?;
                let current_day: i64 = now / 86_400;
                let num_days = (current_day - oldest_day + 1) as i64;
                if num_days <= 0 {
                    1.0
                } else {
                    let start_timestamp: i64 = oldest_day * 86_400;
                    let end_timestamp: i64 = (current_day + 1) * 86_400;
                    let total_critical: i64 = usage_logs::table
                        .filter(crate::schema::usage_logs::user_id.eq(user_id))
                        .filter(usage_logs::activity_type.like("%_critical"))
                        .filter(usage_logs::created_at.ge(start_timestamp as i32))
                        .filter(usage_logs::created_at.lt(end_timestamp as i32))
                        .count()
                        .get_result(&mut conn)?;
                    if total_critical == 0 {
                        1.0
                    } else {
                        total_critical as f32 / num_days as f32
                    }
                }
            }
        };
        println!("average per day: {}", average_critical_per_day);
        // Get user's phone number to determine country
        let phone_number = self
            .get_user()?
            .map(|user| user.phone_number)
            .ok_or_else(|| diesel::result::Error::NotFound)?;
        // Determine country based on phone number
        let country = if phone_number.starts_with("+1") {
            "US"
        } else if phone_number.starts_with("+358") {
            "FI"
        } else if phone_number.starts_with("+31") {
            "NL"
        } else if phone_number.starts_with("+44") {
            "UK"
        } else if phone_number.starts_with("+61") {
            "AU"
        } else {
            "Other"
        };
        // Calculate estimated monthly price based on country and notification method
        let estimated_monthly_price = if enabled.is_none() {
            0.0
        } else {
            let notifications_per_month = average_critical_per_day * 30.0; // Assume 30 days per month
            match (country, enabled.as_deref()) {
                ("US", Some("sms")) => notifications_per_month * 0.5, // 1/2 message cost
                ("US", Some("call")) => notifications_per_month * 0.5, // 1/2 message cost
                ("FI", Some("sms")) => notifications_per_month * 0.15,
                ("FI", Some("call")) => notifications_per_month * 0.70,
                ("NL", Some("sms")) => notifications_per_month * 0.15,
                ("NL", Some("call")) => notifications_per_month * 0.45,
                ("UK", Some("sms")) => notifications_per_month * 0.15,
                ("UK", Some("call")) => notifications_per_month * 0.15,
                ("AU", Some("sms")) => notifications_per_month * 0.15,
                ("AU", Some("call")) => notifications_per_month * 0.15,
                _ => 0.0, // No pricing for "Other" or disabled
            }
        };
        Ok(crate::handlers::profile_handlers::CriticalNotificationInfo {
            enabled,
            average_critical_per_day,
            estimated_monthly_price,
            call_notify,
            action_on_critical_message,
        })
    }

    pub fn get_priority_notification_info(&self) -> Result<crate::handlers::filter_handlers::PriorityNotificationInfo, diesel::result::Error> {
        use crate::schema::{usage_logs};
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        // Get average priority notifications per day
        let average_priority_per_day = {
            let now: i64 = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs() as i64;
            let thirty_days_ago: i64 = now - 2_592_000; // 30 * 86_400
            let active_days_count: i64 = usage_logs::table
                .select(sql::<BigInt>("COUNT(DISTINCT created_at / 86400)"))
                .filter(crate::schema::usage_logs::user_id.eq(1))
                .filter(usage_logs::created_at.ge(thirty_days_ago as i32))
                .first(&mut conn)?;
            if active_days_count < 3 {
                0.0
            } else {
                let oldest_day: i64 = usage_logs::table
                    .select(sql::<BigInt>("MIN(created_at / 86400)"))
                    .filter(crate::schema::usage_logs::user_id.eq(1))
                    .filter(usage_logs::created_at.ge(thirty_days_ago as i32))
                    .first(&mut conn)?;
                let current_day: i64 = now / 86_400;
                let num_days = (current_day - oldest_day + 1) as i64;
                if num_days <= 0 {
                    0.0
                } else {
                    let start_timestamp: i64 = oldest_day * 86_400;
                    let end_timestamp: i64 = (current_day + 1) * 86_400;
                    let total_priority: i64 = usage_logs::table
                        .filter(crate::schema::usage_logs::user_id.eq(1))
                        .filter(usage_logs::activity_type.like("%_priority"))
                        .filter(usage_logs::created_at.ge(start_timestamp as i32))
                        .filter(usage_logs::created_at.lt(end_timestamp as i32))
                        .count()
                        .get_result(&mut conn)?;
                    if total_priority == 0 {
                        0.0
                    } else {
                        total_priority as f32 / num_days as f32
                    }
                }
            }
        };
        // Get user's phone number to determine country
        let phone_number = self
            .get_user()?
            .map(|user| user.phone_number)
            .ok_or_else(|| diesel::result::Error::NotFound)?;
        // Determine country based on phone number
        let country = if phone_number.starts_with("+1") {
            "US"
        } else if phone_number.starts_with("+358") {
            "FI"
        } else if phone_number.starts_with("+31") {
            "NL"
        } else if phone_number.starts_with("+44") {
            "UK"
        } else if phone_number.starts_with("+61") {
            "AU"
        } else {
            "Other"
        };
        // Calculate estimated monthly price based on country, assuming "sms"
        let estimated_monthly_price = {
            let notifications_per_month = average_priority_per_day * 30.0; // Assume 30 days per month
            match (country, "sms") {
                ("US", "sms") => notifications_per_month * 0.15 / 2.0,
                ("FI", "sms") => notifications_per_month * 0.15,
                ("NL", "sms") => notifications_per_month * 0.15,
                ("UK", "sms") => notifications_per_month * 0.15,
                ("AU", "sms") => notifications_per_month * 0.15,
                _ => 0.0, // No pricing for "Other"
            }
        };
        Ok(crate::handlers::filter_handlers::PriorityNotificationInfo {
            average_per_day: average_priority_per_day,
            estimated_monthly_price,
        })
    }


    pub fn get_openrouter_api_key(&self, user_id: i32) -> Result<String, Box<dyn Error>> {
        use crate::schema::user_settings;
        use crate::utils::encryption::decrypt;
        
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Get the user settings
        let settings = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select(
                user_settings::encrypted_openrouter_api_key,
            )
            .first::<Option<String>>(&mut conn)?;

        match settings {
            Some(encrypted_openrouter_api_key) => {
                let openrouter_api_key= decrypt(&encrypted_openrouter_api_key)?;
                Ok(openrouter_api_key)
            },
            _ => Err("Openrouter api key not found".into())
        }
    }

    pub fn get_twilio_credentials(&self) -> Result<(String, String, String), Box<dyn Error>> {
        use crate::schema::user_settings;
        use crate::utils::encryption::decrypt;
        
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Get the user settings
        let settings = user_settings::table
            .filter(user_settings::user_id.eq(1))
            .select((
                user_settings::encrypted_twilio_account_sid,
                user_settings::encrypted_twilio_auth_token,
                user_settings::server_url,
            ))
            .first::<(Option<String>, Option<String>, Option<String>)>(&mut conn)?;

        match settings {
            (Some(encrypted_account_sid), Some(encrypted_auth_token), Some(server_url)) => {
                let account_sid = decrypt(&encrypted_account_sid)?;
                let auth_token = decrypt(&encrypted_auth_token)?;
                Ok((account_sid, auth_token, server_url))
            },
            _ => Err("Twilio credentials not found".into())
        }
    }


    pub fn update_twilio_credentials(&self, account_sid: &str, auth_token: &str) -> Result<(), Box<dyn Error>> {
        use crate::schema::user_settings;
        use crate::utils::encryption::encrypt;
        
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;

        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let encrypted_account_sid = encrypt(account_sid)?;
        let encrypted_auth_token = encrypt(auth_token)?;

        diesel::update(user_settings::table.filter(user_settings::user_id.eq(1)))
            .set((
                user_settings::encrypted_twilio_account_sid.eq(encrypted_account_sid.clone()),
                user_settings::encrypted_twilio_auth_token.eq(encrypted_auth_token.clone()),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_textbee_credentials(&self, user_id: i32) -> Result<(String, String), Box<dyn Error>> {
        use crate::schema::user_settings;
        use crate::utils::encryption::decrypt;
        
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Get the user settings
        let settings = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select((
                user_settings::encrypted_textbee_device_id,
                user_settings::encrypted_textbee_api_key,
            ))
            .first::<(Option<String>, Option<String>)>(&mut conn)?;

        match settings {
            (Some(encrypted_device_id), Some(encrypted_api_key)) => {
                let device_id= decrypt(&encrypted_device_id)?;
                let api_key= decrypt(&encrypted_api_key)?;
                Ok((device_id, api_key))
            },
            _ => Err("Textbee credentials not found".into())
        }
    }

    pub fn update_textbee_credentials(&self, user_id: i32, device_id: &str, api_key: &str) -> Result<(), Box<dyn Error>> {
        use crate::schema::user_settings;
        use crate::utils::encryption::encrypt;
        
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;

        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let encrypted_device_id = encrypt(device_id)?;
        let encrypted_api_key= encrypt(api_key)?;

        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set((
                user_settings::encrypted_textbee_device_id.eq(encrypted_device_id.clone()),
                user_settings::encrypted_textbee_api_key.eq(encrypted_api_key.clone()),
            ))
            .execute(&mut conn)?;

        Ok(())
    }
    pub fn get_elevenlabs_phone_number_id(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;

        // Get the critical_enabled setting
        let number= user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select(user_settings::elevenlabs_phone_number_id)
            .first::<Option<String>>(&mut conn)?;

        Ok(number)
    }


    pub fn set_elevenlabs_phone_number_id(&self, user_id: i32, phone_number_id: &str) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;

        // Update the server_instance_id
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::elevenlabs_phone_number_id.eq(Some(phone_number_id)))
            .execute(&mut conn)?;

        Ok(())
    }


    pub fn set_server_instance_id(&self, user_id: i32, server_instance_id: &str) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist()?;

        // Update the server_instance_id
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::server_instance_id.eq(Some(server_instance_id)))
            .execute(&mut conn)?;

        Ok(())
    }

    // for self hosted instance
    pub fn update_instance_id_to_self_hosted(&self, server_instance_id: &str) -> Result<(), DieselError> {
        use crate::schema::{users, user_settings};
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Get the first user from the users table
        let first_user = users::table
            .order(users::id.asc())
            .first::<User>(&mut conn)?;

        // Ensure user settings exist for this user
        self.ensure_user_settings_exist()?;

        // Update the server_instance_id for this user
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(first_user.id)))
            .set(user_settings::server_instance_id.eq(Some(server_instance_id)))
            .execute(&mut conn)?;
        Ok(())
    }

    // for self hosted instance
    pub fn get_settings_for_tier3(
        &self,
    ) -> Result<(Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>), Box<dyn std::error::Error>> {
        use crate::schema::{users, user_settings};
        use crate::utils::encryption::decrypt;
        
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Find the first user(should be only one)
        let tier3_user = users::table
            .first::<User>(&mut conn)?;
            
        // Get their settings
        let settings = user_settings::table
            .filter(user_settings::user_id.eq(tier3_user.id))
            .select((
                user_settings::encrypted_twilio_account_sid,
                user_settings::encrypted_twilio_auth_token,
                user_settings::encrypted_openrouter_api_key,
                user_settings::server_url,
                user_settings::encrypted_geoapify_key,
                user_settings::encrypted_pirate_weather_key,
            ))
            .first::<(Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(&mut conn)?;
            
        match settings {
            (Some(encrypted_account_sid), Some(encrypted_auth_token), openrouter_api_key, server_url, geoapify_key, pirate_key) => {
                let account_sid = decrypt(&encrypted_account_sid).ok();
                let auth_token = decrypt(&encrypted_auth_token).ok();
                Ok((account_sid, auth_token, openrouter_api_key, server_url, geoapify_key, pirate_key))
            },
            _ => Ok((None, None, None, None, None, None))
        }
    }
}

