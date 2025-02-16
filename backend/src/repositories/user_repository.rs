





use diesel::prelude::*;
use diesel::result::Error as DieselError;
use crate::{
    models::user_models::User,
    handlers::auth_dtos::NewUser,
    schema::users,
    DbPool,
};

pub struct UserRepository {
    pool: DbPool
}

impl UserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
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

        Ok(user.email == "rasmus")
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
    pub fn update_profile(&self, user_id: i32, phone_number: &str, nickname: &str, info: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Check if phone number exists for a different user
        let existing_user = users::table
            .filter(users::phone_number.eq(phone_number))
            .filter(users::id.ne(user_id))
            .first::<User>(&mut conn)
            .optional()?;
            
        if existing_user.is_some() {
            return Err(DieselError::RollbackTransaction);
        }

        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set((
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

    // Update user's IQ (credits)
    pub fn update_user_iq(&self, user_id: i32, new_iq: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::iq.eq(new_iq))
            .execute(&mut conn)?;
        Ok(())
    }
    // Increase user's IQ by a specified amount
    pub fn increase_iq(&self, user_id: i32, amount: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table
            .find(user_id)
            .first::<User>(&mut conn)?;
        
        let new_iq = user.iq + amount;
        
        diesel::update(users::table.find(user_id))
            .set(users::iq.eq(new_iq))
            .execute(&mut conn)?;
        Ok(())
    }

    // Update user's notify_credits preference
    pub fn update_notify_credits(&self, user_id: i32, notify: bool) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::notify_credits.eq(notify))
            .execute(&mut conn)?;
        Ok(())
    }

}
