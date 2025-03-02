use diesel::prelude::*;
use serde::Serialize;
use diesel::result::Error as DieselError;
use std::error::Error;
use rand;

#[derive(Serialize, PartialEq)]
pub struct UsageDataPoint {
    pub timestamp: i32,
    pub credits: f32,
}

use crate::{
    models::user_models::{User, NewUsageLog},
    handlers::auth_dtos::NewUser,
    schema::{users, usage_logs},
    DbPool,
};

pub struct UserRepository {
    pool: DbPool
}

impl UserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn set_preferred_number_to_default(&self, user_id: i32, phone_number: &str) -> Result<String, Box<dyn Error>> {
        // Get all Twilio phone numbers from environment
        let phone_numbers = [
            ("FIN_PHONE", "+358"),
            ("USA_PHONE", "+1"),
            ("NLD_PHONE", "+31"),
            ("CHZ_PHONE", "+420"),
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
            .filter(users::email.eq(search_email))
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
            .filter(users::email.eq(search_email))
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
    pub fn update_profile(&self, user_id: i32, email: &str, phone_number: &str, nickname: &str, info: &str) -> Result<(), DieselError> {
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

        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set((
                users::email.eq(email),
                users::phone_number.eq(phone_number),
                users::nickname.eq(nickname),
                users::info.eq(info),
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
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(users::table.find(user_id))
            .execute(&mut conn)?;
        Ok(())
    }

    // Update user's (credits)
    pub fn update_user_credits(&self, user_id: i32, new_credits: f32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::credits.eq(new_credits))
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

    // log the usage of either call or sms
    pub fn log_usage(&self, user_id: i32, activity_type: &str, credits: f32, success: bool, possible_summary: Option<String>) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let user = users::table
            .find(user_id)
            .first::<User>(&mut conn)?;

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // only store the summary if user has given permission
        let summary = match user.debug_logging_permission {
            true => possible_summary,
            false => None,
        };

        let new_log = NewUsageLog {
            user_id,
            activity_type: activity_type.to_string(),
            credits,
            created_at: current_time,
            success,
            summary,
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

    // Update user's notify preference
    pub fn update_notify(&self, user_id: i32, notify: bool) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::notify.eq(notify))
            .execute(&mut conn)?;
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
            .load::<(i32, f32)>(&mut conn)?
            .into_iter()
            .map(|(timestamp, credit_amount)| UsageDataPoint {
                timestamp,
                credits: credit_amount,
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


}
