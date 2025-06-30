use diesel::prelude::*;
use diesel::sql_types::Text;
use diesel::result::Error as DieselError;
use std::error::Error;
use crate::{
    models::user_models::{User, UserSettings, NewUserSettings, TempVariable, NewTempVariable},
    schema::{users, user_settings, temp_variables},
    DbPool,
};

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
                    timezone: None,
                    timezone_auto: None,
                    agent_language: "en".to_string(),
                    sub_country: None,
                    info: None,
                    save_context: Some(5),
                    critical_enabled: true,
                    number_of_digests_locked: 0,
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
                timezone: None,
                timezone_auto: None,
                agent_language: "en".to_string(),
                sub_country: None,
                info: None,
                save_context: Some(5),
                critical_enabled: true,
                number_of_digests_locked: 0,
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

    // Update user's profile
    pub fn update_profile(&self, user_id: i32, email: &str, phone_number: &str, nickname: &str, info: &str, timezone: &str, timezone_auto: &bool, notification_type: Option<&str>, save_context: Option<i32>) -> Result<(), DieselError> {
        use crate::schema::user_settings;
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

            // Update the settings
            diesel::update(user_settings::table.filter(user_settings::user_id.eq(user_id)))
                .set((
                    user_settings::timezone.eq(timezone.to_string()),
                    user_settings::timezone_auto.eq(timezone_auto),
                    user_settings::notification_type.eq(notification_type.map(|s| s.to_string())),
                    user_settings::info.eq(info),
                    user_settings::save_context.eq(save_context),
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
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // First fetch the user settings to check timezone_auto
        let user_settings = self.get_user_settings(user_id)?;
        // Only update if timezone_auto is false (manual timezone setting)
        if !user_settings.timezone_auto.unwrap_or(false) {
            diesel::update(user_settings::table)
                .filter(user_settings::user_id.eq(user_id))
                .set(user_settings::timezone.eq(timezone.to_string()))
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


    pub fn get_whatsapp_temp_variable(&self, user_id: i32) -> Result<Option<(Option<String>, Option<String>, Option<String>)>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let temp_var = temp_variables::table
            .filter(temp_variables::user_id.eq(user_id))
            .filter(temp_variables::confirm_send_event_type.eq("whatsapp"))
            .first::<TempVariable>(&mut conn)
            .optional()?;
            
        match temp_var {
            Some(var) => Ok(Some((var.confirm_send_event_recipient, var.confirm_send_event_content, var.confirm_send_event_image_url))),
            None => Ok(None)
        }
    }

    pub fn get_telegram_temp_variable(&self, user_id: i32) -> Result<Option<(Option<String>, Option<String>, Option<String>)>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let temp_var = temp_variables::table
            .filter(temp_variables::user_id.eq(user_id))
            .filter(temp_variables::confirm_send_event_type.eq("telegram"))
            .first::<TempVariable>(&mut conn)
            .optional()?;
            
        match temp_var {
            Some(var) => Ok(Some((var.confirm_send_event_recipient, var.confirm_send_event_content, var.confirm_send_event_image_url))),
            None => Ok(None)
        }
    }

    pub fn get_calendar_temp_variable(&self, user_id: i32) -> Result<Option<(Option<String>, Option<String>, Option<String>, Option<String>)>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let temp_var = temp_variables::table
            .filter(temp_variables::user_id.eq(user_id))
            .filter(temp_variables::confirm_send_event_type.eq("calendar"))
            .first::<TempVariable>(&mut conn)
            .optional()?;
            
        match temp_var {
            Some(var) => Ok(Some((
                var.confirm_send_event_subject,
                var.confirm_send_event_start_time,
                var.confirm_send_event_duration,
                var.confirm_send_event_content,
            ))),
            None => Ok(None)
        }
    }

    pub fn get_email_temp_variable(&self, user_id: i32) -> Result<Option<(Option<String>, Option<String>, Option<String>, Option<String>)>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let temp_var = temp_variables::table
            .filter(temp_variables::user_id.eq(user_id))
            .filter(temp_variables::confirm_send_event_type.eq("email"))
            .first::<TempVariable>(&mut conn)
            .optional()?;
            
        match temp_var {
            Some(var) => Ok(Some((
                var.confirm_send_event_recipient,
                var.confirm_send_event_subject,
                var.confirm_send_event_content,
                var.confirm_send_event_id
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

    pub fn update_critical_enabled(&self, user_id: i32, enabled: bool) -> Result<(), DieselError> {
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

    pub fn get_critical_enabled(&self, user_id: i32) -> Result<bool, DieselError> {
        use crate::schema::user_settings;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Ensure user settings exist
        self.ensure_user_settings_exist(user_id)?;

        // Get the critical_enabled setting
        let critical_enabled = user_settings::table
            .filter(user_settings::user_id.eq(user_id))
            .select(user_settings::critical_enabled)
            .first::<bool>(&mut conn)?;

        Ok(critical_enabled)
    }

    pub fn update_next_billing_date(&self, user_id: i32, timestamp: i32) -> Result<(), DieselError> {
        use crate::schema::users;
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        diesel::update(users::table.find(user_id))
            .set(users::next_billing_date_timestamp.eq(timestamp))
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

}

