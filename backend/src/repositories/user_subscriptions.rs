use diesel::prelude::*;
use diesel::result::Error as DieselError;
use crate::{
    models::user_models::User,
    schema::users,
    DbPool,
};

impl crate::repositories::user_repository::UserRepository {
    // Subscription related methods
    pub fn get_subscription_tier(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let sub_tier = users::table
            .find(user_id)
            .select(users::sub_tier)
            .first::<Option<String>>(&mut conn)?;
        Ok(sub_tier)
    }

    pub fn set_subscription_tier(&self, user_id: i32, tier: Option<&str>) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::sub_tier.eq(tier))
            .execute(&mut conn)?;
        Ok(())
    }

    // Credits management
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

    // Stripe related methods
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

    // Check if user has a specific subscription tier and has messages left
    pub fn has_valid_subscription_tier(&self, user_id: i32, tier: &str) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let user = users::table
            .find(user_id)
            .select((users::sub_tier, users::discount_tier))
            .first::<(Option<String>, Option<String>)>(&mut conn)?;
        
        // If user has msg discount tier, they always have messages available
        if user.1.as_deref() == Some("msg") {
        
            return Ok(user.0.map_or(false, |t| t == tier));
        }
        
        // Check both subscription tier and remaining messages
        Ok(user.0.map_or(false, |t| t == tier))
    }

    // Helper method to ensure msg discount tier users always have messages
    pub fn ensure_msg_tier_messages(&self, user_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        let (current_msgs, discount_tier) = users::table
            .find(user_id)
            .select((users::msgs_left, users::discount_tier))
            .first::<(i32, Option<String>)>(&mut conn)?;
            
        if discount_tier.as_deref() == Some("msg") && current_msgs < 100 {
            diesel::update(users::table.find(user_id))
                .set(users::msgs_left.eq(100))
                .execute(&mut conn)?;
        }
        
        Ok(())
    }

    pub fn decrease_messages_left(&self, user_id: i32) -> Result<i32, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
                // Get current messages left and discount tier
        let (current_msgs, discount_tier) = users::table
            .find(user_id)
            .select((users::msgs_left, users::discount_tier))
            .first::<(i32, Option<String>)>(&mut conn)?;
            
        // If user has "msg" discount tier, return current messages without decreasing
        if discount_tier.as_deref() == Some("msg") {
            // ensure msg tier users have messages
            self.ensure_msg_tier_messages(user_id)?;
            return Ok(current_msgs);
        }
            
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

}

