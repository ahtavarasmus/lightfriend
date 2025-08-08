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

    pub fn find_by_email(&self, search_email: &str) -> Result<Option<User>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table
            .filter(lower(users::email).eq(lower(search_email)))
            .first::<User>(&mut conn)
            .optional()?;
        Ok(user)
    }

    pub fn find_by_id(&self, user_id: i32) -> Result<Option<User>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table
            .find(user_id)
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

    pub fn get_all_users(&self) -> Result<Vec<User>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let users_list = users::table
            .load::<User>(&mut conn)?;
        Ok(users_list)
    }

    pub fn delete_user(&self, user_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(users::table.find(user_id))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn verify_user(&self, user_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::verified.eq(true))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_password(&self, email: &str, password_hash: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table)
            .filter(users::email.eq(email))
            .set(users::password_hash.eq(password_hash))
            .execute(&mut conn)?;
        Ok(())
    }

    // Helper function to ensure user_info exists
    pub fn ensure_user_info_exists(&self, user_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let exists = user_info::table
            .filter(user_info::user_id.eq(user_id))
            .first::<UserInfo>(&mut conn)
            .optional()?
            .is_some();

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
    pub fn get_user_info(&self, user_id: i32) -> Result<UserInfo, DieselError> {
        self.ensure_user_info_exists(user_id)?;

        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let user_info = user_info::table
            .filter(user_info::user_id.eq(user_id))
            .first::<UserInfo>(&mut conn)?;

        Ok(user_info)
    }


    // User settings operations
    pub fn get_user_settings(&self, user_id: i32) -> Result<UserSettings, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .first::<UserSettings>(&mut conn)
            .optional()?;
            
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
    pub fn ensure_user_settings_exist(&self, user_id: i32) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let settings_exist = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .first::<UserSettings>(&mut conn)
            .optional()?
            .is_some();

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
            };
            
            diesel::insert_into(user_settings::table)
                .values(&new_settings)
                .execute(&mut conn)?;
        }
        Ok(())
    }

    // Basic validation methods
    pub fn email_exists(&self, search_email: &str) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let existing_user: Option<User> = users::table
            .filter(lower(users::email).eq(lower(search_email)))
            .first::<User>(&mut conn)
            .optional()?;
        Ok(existing_user.is_some())
    }

    pub fn phone_number_exists(&self, search_phone: &str) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let existing_user: Option<User> = users::table
            .filter(users::phone_number.eq(search_phone))
            .first::<User>(&mut conn)
            .optional()?;
        Ok(existing_user.is_some())
    }

    pub fn is_admin(&self, user_id: i32) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table
            .find(user_id)
            .first::<User>(&mut conn)?;
        Ok(user.email == "rasmus@ahtava.com" && user.id == 1)
    }

    pub fn update_blocker_password(&self, user_id: i32, new_password: Option<&str>) -> Result<(), DieselError> {
        use crate::schema::user_info;
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Ensure user settings exist
        self.ensure_user_info_exists(user_id)?;

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
        self.ensure_user_info_exists(user_id)?;

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
        self.ensure_user_settings_exist(user_id)?;

        // Update the settings
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::sub_country.eq(country))
            .execute(&mut conn)?;


        Ok(())
    }

    pub fn update_preferred_number(&self, user_id: i32, preferred_number: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
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

    pub fn set_preferred_number_to_us_default(&self, user_id: i32) -> Result<String, Box<dyn Error>> {
        let preferred_number = std::env::var("USA_PHONE").expect("USA_PHONE not found");

        // Update the user's preferred number in the database
        self.update_preferred_number(user_id, &preferred_number)?;

        Ok(preferred_number)
    }

    // Update user's profile
    pub fn update_profile(&self, user_id: i32, email: &str, phone_number: &str, nickname: &str, info: &str, timezone: &str, timezone_auto: &bool, notification_type: Option<&str>, save_context: Option<i32>, require_confirmation: bool, location: &str, nearby_places: &str) -> Result<(), DieselError> {
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
            // Check if email exists for a different user
            let existing_email = users::table
                .filter(users::email.eq(email.to_lowercase()))
                .filter(users::id.ne(user_id))
                .first::<User>(conn)
                .optional()?;
               
            if existing_email.is_some() {
                return Err(DieselError::NotFound);
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
                    users::email.eq(email),
                    users::phone_number.eq(phone_number),
                    users::nickname.eq(nickname),
                    users::verified.eq(!should_unverify && current_user.verified), // Only keep verified true if phone number hasn't changed
                ))
                .execute(conn)?;
            // Ensure user settings exist
            self.ensure_user_settings_exist(user_id)?;
            // Ensure user info exists
            self.ensure_user_info_exists(user_id)?;
            // Update the settings
            diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
                .set((
                    user_settings::timezone_auto.eq(timezone_auto),
                    user_settings::notification_type.eq(notification_type.map(|s| s.to_string())),
                    user_settings::save_context.eq(save_context),
                    user_settings::require_confirmation.eq(require_confirmation),
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
        self.ensure_user_settings_exist(user_id)?;

        // Update the settings
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::notify.eq(notify))
            .execute(&mut conn)?;


        Ok(())
    }

    pub fn update_timezone(&self, user_id: i32, timezone: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // First fetch the user settings to check timezone_auto
        let user_settings= self.get_user_settings(user_id)?;
        // Only update if timezone_auto is false (manual timezone setting)
        if !user_settings.timezone_auto.unwrap_or(false) {
            diesel::update(user_info::table)
                .filter(user_info::user_id.eq(user_id))
                .set(user_info::timezone.eq(timezone.to_string()))
                .execute(&mut conn)?;
        }
        
        Ok(())
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

    pub fn set_free_reply(&self, user_id: i32, free_reply: bool) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::free_reply.eq(free_reply))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn set_temp_variable(&self, user_id: i32, event_type: Option<&str>, recipient: Option<&str>, subject: Option<&str>, 
        content: Option<&str>, start_time: Option<&str>, duration: Option<&str>, event_id: Option<&str>, image_url: Option<&str>) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Start a transaction
        conn.transaction(|conn| {
            // First set the confirm_send_event flag to true
            diesel::update(users::table.find(user_id))
                .set(users::confirm_send_event.eq(event_type))
                .execute(conn)?;

            // Delete any existing temp variables for this user
            diesel::delete(temp_variables::table.filter(temp_variables::user_id.eq(user_id)))
                .execute(conn)?;

            // Create new temp variable
            let new_temp_var = NewTempVariable {
                user_id,
                confirm_send_event_type: event_type.unwrap().to_string(),
                confirm_send_event_recipient: recipient.map(|s| s.to_string()),
                confirm_send_event_subject: subject.map(|s| s.to_string()),
                confirm_send_event_content: content.map(|s| s.to_string()),
                confirm_send_event_start_time: start_time.map(|s| s.to_string()),
                confirm_send_event_duration: duration.map(|s| s.to_string()),
                confirm_send_event_id: event_id.map(|s| s.to_string()),
                confirm_send_event_image_url: image_url.map(|s| s.to_string()),
            };

            // Insert the new temp variable
            diesel::insert_into(temp_variables::table)
                .values(&new_temp_var)
                .execute(conn)?;

            Ok(())
        })
    }


    pub fn get_temp_variable(&self, user_id: i32, service_type: &str) -> Result<Option<(Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let temp_var = temp_variables::table
            .filter(temp_variables::user_id.eq(user_id))
            .filter(temp_variables::confirm_send_event_type.eq(service_type))
            .first::<TempVariable>(&mut conn)
            .optional()?;
            
        match temp_var {
            Some(var) => Ok(Some((
                var.confirm_send_event_recipient, 
                var.confirm_send_event_subject, 
                var.confirm_send_event_content, 
                var.confirm_send_event_start_time, 
                var.confirm_send_event_duration, 
                var.confirm_send_event_id, 
                var.confirm_send_event_image_url
            ))),
            None => Ok(None)
        }
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

    pub fn update_discount_tier(&self, user_id: i32, discount_tier: Option<&str>) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        diesel::update(users::table.find(user_id))
            .set(users::discount_tier.eq(discount_tier))
            .execute(&mut conn)?;
            
        Ok(())
    }

    pub fn clear_confirm_send_event(&self, user_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Start a transaction
        conn.transaction(|conn| {
            // Clear the confirm_send_event flag
            diesel::update(users::table.find(user_id))
                .set(users::confirm_send_event.eq::<Option<String>>(None))
                .execute(conn)?;

            // Delete any existing temp variables for this user
            diesel::delete(temp_variables::table.filter(temp_variables::user_id.eq(user_id)))
                .execute(conn)?;

            Ok(())
        })
    }

    pub fn update_digests(&self, user_id: i32, morning_digest: Option<&str>, day_digest: Option<&str>, evening_digest: Option<&str>) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;

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

    pub fn get_digests(&self, user_id: i32) -> Result<(Option<String>, Option<String>, Option<String>), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;

        // Get the user settings
        let settings = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
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
        self.ensure_user_settings_exist(user_id)?;

        // Update the setting
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::proactive_agent_on.eq(enabled))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_proactive_agent_on(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;

        // Get the setting
        let proactive_agent_on= user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select(user_settings::proactive_agent_on)
            .first::<bool>(&mut conn)?;

        Ok(proactive_agent_on)
    }

    pub fn update_critical_enabled(&self, user_id: i32, enabled: Option<String>) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;

        // Update the critical_enabled setting
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::critical_enabled.eq(enabled))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_critical_enabled(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;

        // Get the critical_enabled setting
        let critical_enabled = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select(user_settings::critical_enabled)
            .first::<Option<String>>(&mut conn)?;

        Ok(critical_enabled)
    }


    pub fn get_critical_notification_info(&self, user_id: i32) -> Result<crate::handlers::profile_handlers::CriticalNotificationInfo, diesel::result::Error> {
            use crate::schema::{user_settings, usage_logs};
            let mut conn = self.pool.get().expect("Failed to get DB connection");
            // Ensure user settings exist
            self.ensure_user_settings_exist(user_id)?;
            // Get the critical_enabled setting
            let enabled = user_settings::table
                .filter(user_settings::user_id.eq(user_id))
                .select(user_settings::critical_enabled)
                .first::<Option<String>>(&mut conn)?;
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
                .find_by_id(user_id)?
                .map(|user| user.phone_number)
                .ok_or_else(|| diesel::result::Error::NotFound)?;
            // Determine country based on phone number
            let country = if phone_number.starts_with("+1") {
                "US"
            } else if phone_number.starts_with("+358") {
                "FI"
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
            })
        }

            pub fn get_priority_notification_info(&self, user_id: i32) -> Result<crate::handlers::filter_handlers::PriorityNotificationInfo, diesel::result::Error> {
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
                .filter(crate::schema::usage_logs::user_id.eq(user_id))
                .filter(usage_logs::created_at.ge(thirty_days_ago as i32))
                .first(&mut conn)?;
            if active_days_count < 3 {
                0.0
            } else {
                let oldest_day: i64 = usage_logs::table
                    .select(sql::<BigInt>("MIN(created_at / 86400)"))
                    .filter(crate::schema::usage_logs::user_id.eq(user_id))
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
                        .filter(crate::schema::usage_logs::user_id.eq(user_id))
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
        println!("average per day: {}", average_priority_per_day);
        // Get user's phone number to determine country
        let phone_number = self
            .find_by_id(user_id)?
            .map(|user| user.phone_number)
            .ok_or_else(|| diesel::result::Error::NotFound)?;
        // Determine country based on phone number
        let country = if phone_number.starts_with("+1") {
            "US"
        } else if phone_number.starts_with("+358") {
            "FI"
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

    pub fn verify_pairing_code(&self, pairing_code: &str, server_instance_id: &str) -> Result<(bool, Option<String>), DieselError> {
        // Try to find a user with the given pairing code
        if let Some(user_id) = self.find_user_by_pairing_code(pairing_code)? {
            // If user found, update their server instance ID
            self.set_server_instance_id(user_id, server_instance_id)?;
            
            // Get the user's phone number
            let user = self.find_by_id(user_id)?;
            let phone_number = user.map(|u| u.phone_number);
            
            Ok((true, phone_number))
        } else {
            Ok((false, None))
        }
    }

    pub fn update_next_billing_date(&self, user_id: i32, timestamp: i32) -> Result<(), DieselError> {
        use crate::schema::users;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        diesel::update(users::table.find(user_id))
            .set(users::next_billing_date_timestamp.eq(timestamp))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn update_subscription_tier(&self, user_id: i32, tier: Option<&str>) -> Result<(), DieselError> {
        use crate::schema::users;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        diesel::update(users::table.find(user_id))
            .set(users::sub_tier.eq(tier))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn update_server_instance_last_ping(&self, user_id: i32, timestamp: i32) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;

        // Update the last ping timestamp
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::server_instance_last_ping_timestamp.eq(timestamp))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn update_server_ip(&self, user_id: i32, server_ip: &str) -> Result<(), DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;

        // Update the last ping timestamp
        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
            .set(user_settings::server_ip.eq(server_ip))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_next_billing_date(&self, user_id: i32) -> Result<Option<i32>, DieselError> {
        use crate::schema::users;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let timestamp = users::table
            .find(user_id)
            .select(users::next_billing_date_timestamp)
            .first::<Option<i32>>(&mut conn)?;

        Ok(timestamp)
    }

    pub fn generate_pairing_code(&self, user_id: i32) -> Result<String, DieselError> {
        use crate::schema::user_settings;
        use rand::{thread_rng, Rng};
        use std::time::{SystemTime, UNIX_EPOCH};
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;

        // Get current timestamp for uniqueness
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        loop {
            // Generate base random part (8 chars)
            const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
            let mut rng = thread_rng();
            let random_part: String = (0..8)
                .map(|_| {
                    let idx = rng.gen_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect();

            // Create unique code combining:
            // - First 2 chars: User-specific hash (to spread users across the space)
            // - Next 4 chars: Timestamp-based (for temporal uniqueness)
            // - Last 8 chars: Random (for entropy)
            let user_hash = (user_id as u64 * 17 + 255) % 1296; // 36^2 possibilities
            let time_hash = timestamp % 1679616; // 36^4 possibilities
            
            let user_part = Self::encode_base36(user_hash, 2);
            let time_part = Self::encode_base36(time_hash, 4);
            
            let code = format!("{}{}{}", user_part, time_part, random_part);

            // Verify this code doesn't exist anywhere in the database
            let exists = user_settings::table
                .filter(user_settings::server_instance_id.eq(&code))
                .count()
                .get_result::<i64>(&mut conn)?;

            if exists == 0 {
                // Update the user settings with the new code
                diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
                    .set(user_settings::server_instance_id.eq(Some(&code)))
                    .execute(&mut conn)?;

                return Ok(code);
            }
            // If code exists (extremely unlikely), loop will continue and generate a new one
        }
    }

    // Helper function to encode numbers in base36 with fixed width
    fn encode_base36(mut num: u64, width: usize) -> String {
        const CHARSET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let mut result = vec!['0'; width];
        
        for i in (0..width).rev() {
            let digit = (num % 36) as usize;
            result[i] = CHARSET[digit] as char;
            num /= 36;
        }
        
        result.into_iter().collect()
    }

    pub fn find_user_by_pairing_code(&self, pairing_code: &str) -> Result<Option<i32>, DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Search for a user_settings record with matching server_instance_id
        let user_id = user_settings::table
            .filter(user_settings::server_instance_id.eq(pairing_code))
            .select(user_settings::user_id)
            .first::<i32>(&mut conn)
            .optional()?;
            
        Ok(user_id)
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

    pub fn get_twilio_credentials(&self, user_id: i32) -> Result<(String, String), Box<dyn Error>> {
        use crate::schema::user_settings;
        use crate::utils::encryption::decrypt;
        
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Get the user settings
        let settings = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select((
                user_settings::encrypted_twilio_account_sid,
                user_settings::encrypted_twilio_auth_token,
            ))
            .first::<(Option<String>, Option<String>)>(&mut conn)?;

        match settings {
            (Some(encrypted_account_sid), Some(encrypted_auth_token)) => {
                let account_sid = decrypt(&encrypted_account_sid)?;
                let auth_token = decrypt(&encrypted_auth_token)?;
                Ok((account_sid, auth_token))
            },
            _ => Err("Twilio credentials not found".into())
        }
    }


    pub fn update_twilio_credentials(&self, user_id: i32, account_sid: &str, auth_token: &str) -> Result<(), Box<dyn Error>> {
        use crate::schema::user_settings;
        use crate::utils::encryption::encrypt;
        
        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;

        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let encrypted_account_sid = encrypt(account_sid)?;
        let encrypted_auth_token = encrypt(auth_token)?;

        diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
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
        self.ensure_user_settings_exist(user_id)?;

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
        self.ensure_user_settings_exist(user_id)?;

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
        self.ensure_user_settings_exist(user_id)?;

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
        self.ensure_user_settings_exist(user_id)?;

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
        self.ensure_user_settings_exist(first_user.id)?;

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

