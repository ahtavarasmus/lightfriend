use diesel::prelude::*;
use serde::Serialize;
use diesel::result::Error as DieselError;
use std::error::Error;

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


    pub fn update_subscription_with_customer_id(
        &self,
        subscription_id: &str,
        customer_id: &str,
        status: &str,
        next_bill_date: i32,
        stage: &str,
    ) -> Result<(), Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(subscriptions::table)
            .filter(subscriptions::paddle_customer_id.eq(customer_id))
            .set((
                subscriptions::paddle_subscription_id.eq(subscription_id),
                subscriptions::status.eq(status),
                subscriptions::next_bill_date.eq(next_bill_date),
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


    pub fn get_active_subscription(&self, user_id: i32) -> Result<Option<Subscription>, Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let active_subscription = subscriptions::table
            .filter(subscriptions::user_id.eq(user_id))
            .filter(subscriptions::status.eq_any(&["active", "trialing"]))
            .first::<Subscription>(&mut conn)
            .optional()?;
        
        Ok(active_subscription)
    }


    pub fn has_active_subscription(&self, user_id: i32) -> Result<bool, Box<dyn Error>> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let active_subscription = subscriptions::table
            .filter(subscriptions::user_id.eq(user_id))
            .filter(subscriptions::status.eq_any(&["active", "trialing"]))
            .first::<Subscription>(&mut conn)
            .optional()?;
        
        Ok(active_subscription.is_some())
    }
}


