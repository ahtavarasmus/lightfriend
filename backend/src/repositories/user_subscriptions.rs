use crate::{
    models::user_models::{NewRefundInfo, RefundInfo, User},
    schema::{refund_info, users},
};
use diesel::prelude::*;
use diesel::result::Error as DieselError;

impl crate::repositories::user_repository::UserRepository {
    // Subscription related methods
    pub fn get_subscription_tier(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        let sub_tier = users::table
            .find(user_id)
            .select(users::sub_tier)
            .first::<Option<String>>(&mut conn)?;
        Ok(sub_tier)
    }

    pub fn set_subscription_tier(
        &self,
        user_id: i32,
        tier: Option<&str>,
    ) -> Result<(), DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::sub_tier.eq(tier))
            .execute(&mut conn)?;
        Ok(())
    }

    // Credits management
    pub fn update_user_credits(&self, user_id: i32, new_credits: f32) -> Result<(), DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::credits.eq(new_credits))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_user_credits_left(
        &self,
        user_id: i32,
        new_credits_left: f32,
    ) -> Result<(), DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::credits_left.eq(new_credits_left))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn increase_credits(&self, user_id: i32, amount: f32) -> Result<(), DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        let user = users::table.find(user_id).first::<User>(&mut conn)?;

        let new_credits = user.credits + amount;

        // Clear last_credits_notification when credits are added so user can be
        // notified again if credits deplete in the future
        diesel::update(users::table.find(user_id))
            .set((
                users::credits.eq(new_credits),
                users::last_credits_notification.eq(None::<i32>),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    // Stripe related methods
    pub fn get_stripe_customer_id(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        let stripe_id = users::table
            .find(user_id)
            .select(users::stripe_customer_id)
            .first::<Option<String>>(&mut conn)?;
        Ok(stripe_id)
    }

    pub fn set_stripe_customer_id(
        &self,
        user_id: i32,
        customer_id: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::stripe_customer_id.eq(customer_id))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn get_stripe_payment_method_id(
        &self,
        user_id: i32,
    ) -> Result<Option<String>, DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        let payment_method_id = users::table
            .find(user_id)
            .select(users::stripe_payment_method_id)
            .first::<Option<String>>(&mut conn)?;
        Ok(payment_method_id)
    }

    pub fn set_stripe_payment_method_id(
        &self,
        user_id: i32,
        payment_method_id: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::stripe_payment_method_id.eq(payment_method_id))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn find_by_stripe_customer_id(
        &self,
        customer_id: &str,
    ) -> Result<Option<User>, DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        let user = users::table
            .filter(users::stripe_customer_id.eq(customer_id))
            .first::<User>(&mut conn)
            .optional()?;
        Ok(user)
    }

    pub fn set_stripe_checkout_session_id(
        &self,
        user_id: i32,
        session_id: &str,
    ) -> Result<(), DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::stripe_checkout_session_id.eq(session_id))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_sub_credits(&self, user_id: i32, new_credits: f32) -> Result<(), DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::credits_left.eq(new_credits))
            .execute(&mut conn)?;
        Ok(())
    }

    /// Update the user's plan type ("assistant", "autopilot", "byot", or None)
    pub fn update_plan_type(
        &self,
        user_id: i32,
        plan_type: Option<&str>,
    ) -> Result<(), DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        diesel::update(users::table.find(user_id))
            .set(users::plan_type.eq(plan_type))
            .execute(&mut conn)?;
        Ok(())
    }

    /// Get the user's plan type
    pub fn get_plan_type(&self, user_id: i32) -> Result<Option<String>, DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        users::table
            .find(user_id)
            .select(users::plan_type)
            .first::<Option<String>>(&mut conn)
    }

    // Check if user has a specific subscription tier and has messages left
    pub fn has_valid_subscription_tier(
        &self,
        user_id: i32,
        tier: &str,
    ) -> Result<bool, DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        let sub_tier = users::table
            .find(user_id)
            .select(users::sub_tier)
            .first::<Option<String>>(&mut conn)?;

        // Check subscription tier
        Ok(sub_tier.is_some_and(|t| t == tier))
    }

    // Refund info methods
    pub fn get_refund_info(&self, user_id: i32) -> Result<Option<RefundInfo>, DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");
        refund_info::table
            .filter(refund_info::user_id.eq(user_id))
            .select(RefundInfo::as_select())
            .first::<RefundInfo>(&mut conn)
            .optional()
    }

    pub fn get_or_create_refund_info(&self, user_id: i32) -> Result<RefundInfo, DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");

        // Try to get existing
        if let Some(info) = refund_info::table
            .filter(refund_info::user_id.eq(user_id))
            .select(RefundInfo::as_select())
            .first::<RefundInfo>(&mut conn)
            .optional()?
        {
            return Ok(info);
        }

        // Create new
        let new_info = NewRefundInfo {
            user_id,
            has_refunded: 0,
        };
        diesel::insert_into(refund_info::table)
            .values(&new_info)
            .execute(&mut conn)?;

        // Return the created row
        refund_info::table
            .filter(refund_info::user_id.eq(user_id))
            .select(RefundInfo::as_select())
            .first::<RefundInfo>(&mut conn)
    }

    pub fn set_has_refunded(&self, user_id: i32, timestamp: i32) -> Result<(), DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");

        // Ensure row exists
        self.get_or_create_refund_info(user_id)?;

        diesel::update(refund_info::table.filter(refund_info::user_id.eq(user_id)))
            .set((
                refund_info::has_refunded.eq(1),
                refund_info::refunded_at.eq(Some(timestamp)),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_last_credit_pack_purchase(
        &self,
        user_id: i32,
        amount: f32,
        timestamp: i32,
    ) -> Result<(), DieselError> {
        let mut conn = self.db_pool.get().expect("Failed to get DB connection");

        // Ensure row exists
        self.get_or_create_refund_info(user_id)?;

        diesel::update(refund_info::table.filter(refund_info::user_id.eq(user_id)))
            .set((
                refund_info::last_credit_pack_amount.eq(Some(amount)),
                refund_info::last_credit_pack_purchase_timestamp.eq(Some(timestamp)),
            ))
            .execute(&mut conn)?;
        Ok(())
    }
}
