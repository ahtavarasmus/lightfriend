use diesel::prelude::*;
use serde::Serialize;
use diesel::result::Error as DieselError;
use std::error::Error;
use chrono::{DateTime, Utc};

use crate::{
    models::user_models::{Subscription, NewSubscription},
    schema::subscriptions,
    DbPool,
};

pub struct UserSubscription {
    pool: DbPool
}

impl UserSubscription {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn find_by_user_id(&self, user_id: i32) -> Result<Option<Subscription>, Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let subscription= subscriptions::table
            .filter(subscriptions::user_id.eq(user_id))
            .first::<Subscription>(&mut conn)
            .optional()?;
        Ok(subscription)
    }

    pub fn create_subscription(
        &self,
        new_sub: NewSubscription
    ) -> Result<(), Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(subscriptions::table)
            .values(&new_sub)
            .execute(&mut conn)?;

        Ok(())
    }

    pub async fn update_usage(
        &self,
        user_id: i32,
        new_iq_amount: i32,
    ) -> Result<(), Box<dyn Error>> {
        // Get subscription or return early with Ok if none found
        let subscription = match self.find_by_user_id(user_id)? {  // Note the ? here to handle the Result
            Some(sub) => sub,
            None => return Ok(()),  // No subscription found, return early
        };
        
        // Check subscription status
        if subscription.status != "active" && subscription.status != "trialing" {
            return Ok(()); // No update needed for inactive subscriptions
        }
        // Only update Paddle if tokens are being consumed (negative amount)
        if new_iq_amount < 0 {
            let iq_quantity = (new_iq_amount.abs() / 3).max(1);
            crate::api::paddle_utils::sync_paddle_subscription_items(
                &subscription.paddle_subscription_id,
                iq_quantity
            ).await?;
        }
        
        Ok(())
    }


    pub fn update_subscription_status(
        &self,
        subscription_id: &str,
        status: &str
    ) -> Result<(), Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(subscriptions::table)
            .filter(subscriptions::paddle_subscription_id.eq(subscription_id))
            .set(subscriptions::status.eq(status))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn reset_user_iq_with_customer_id(
        &self,
        customer_id: &str,
    ) -> Result<(), Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        
        // Find subscription with customer_id
        let subscription = subscriptions::table
            .filter(subscriptions::paddle_customer_id.eq(customer_id))
            .first::<Subscription>(&mut conn)
            .optional()?;
        
        if let Some(sub) = subscription {
            // Get user_id from subscription
            let user_id = sub.user_id;
            
            // Update user's IQ to zero if it's negative
            diesel::update(crate::schema::users::table)
                .filter(crate::schema::users::id.eq(user_id))
                .filter(crate::schema::users::iq.lt(0))
                .set(crate::schema::users::iq.eq(0))
                .execute(&mut conn)?;
        }
        
        Ok(())
    }

    pub fn update_subscription_with_customer_id(
        &self,
        subscription_id: &str,
        customer_id: &str,
        status: &str,
        next_bill_date: i32,
        stage: &str,
        is_scheduled_to_cancel: bool,
    ) -> Result<(), Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(subscriptions::table)
            .filter(subscriptions::paddle_customer_id.eq(customer_id))
            .set((
                subscriptions::paddle_subscription_id.eq(subscription_id),
                subscriptions::status.eq(status),
                subscriptions::next_bill_date.eq(next_bill_date),
                subscriptions::is_scheduled_to_cancel.eq(is_scheduled_to_cancel),
                subscriptions::stage.eq(stage),
            ))
            .execute(&mut conn)?;

        Ok(())
    } 

    pub fn update_subscription_with_user_id(
        &self,
        user_id: i32,
        subscription_id: &str,
        customer_id: &str,
        status: &str,
        next_bill_date: i32,
        stage: &str,
    ) -> Result<(), Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(subscriptions::table)
            .filter(subscriptions::user_id.eq(user_id))
            .set((
                subscriptions::paddle_subscription_id.eq(subscription_id),
                subscriptions::paddle_customer_id.eq(customer_id),
                subscriptions::status.eq(status),
                subscriptions::next_bill_date.eq(next_bill_date),
                subscriptions::stage.eq(stage),
            ))
            .execute(&mut conn)?;

        Ok(())
    } 

    pub fn update_next_billed_at(
        &self,
        subscription_id: &str,
        next_billed_at: &str,
    ) -> Result<(), Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Parse the RFC 3339 string (e.g., "2025-03-26T00:00:00Z") into a timestamp
        let parsed_date = DateTime::parse_from_rfc3339(next_billed_at)
            .map_err(|e| format!("Failed to parse next_billed_at: {}", e))?;
        let timestamp = parsed_date.timestamp() as i32; // Convert to Unix timestamp (seconds)

        // Update the next_bill_date for the matching subscription
        diesel::update(subscriptions::table)
            .filter(subscriptions::paddle_subscription_id.eq(subscription_id))
            .set(subscriptions::next_bill_date.eq(timestamp))
            .execute(&mut conn)?;

        Ok(())
    }


    pub fn has_active_subscription(&self, user_id: i32) -> Result<bool, Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let active_subscription = subscriptions::table
            .filter(subscriptions::user_id.eq(user_id))
            .filter(subscriptions::status.eq("active"))
            .filter(
                subscriptions::is_scheduled_to_cancel
                    .eq(false) // Explicitly false
                    .or(subscriptions::is_scheduled_to_cancel.is_null()) // Or NULL
            )
            .first::<Subscription>(&mut conn)
            .optional()?;
        
        Ok(active_subscription.is_some())
    }

}


