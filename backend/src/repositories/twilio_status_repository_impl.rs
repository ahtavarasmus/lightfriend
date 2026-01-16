//! Real implementation of TwilioStatusRepository using Diesel.
//!
//! This implementation wraps a database connection pool and performs
//! actual database operations.

use std::time::{SystemTime, UNIX_EPOCH};

use diesel::prelude::*;

use crate::repositories::twilio_status_repository::{
    MessageUserInfo, StatusUpdate, TwilioStatusRepository, TwilioStatusRepositoryError,
};
use crate::schema::message_status_log;
use crate::DbPool;

/// Real repository implementation using Diesel.
pub struct DieselTwilioStatusRepository {
    db_pool: DbPool,
}

impl DieselTwilioStatusRepository {
    /// Create a new repository with the given database pool.
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    fn get_timestamp() -> i32 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32
    }
}

impl TwilioStatusRepository for DieselTwilioStatusRepository {
    fn update_message_status(
        &self,
        message_sid: &str,
        update: &StatusUpdate,
    ) -> Result<usize, TwilioStatusRepositoryError> {
        let mut conn = self
            .db_pool
            .get()
            .map_err(|e| TwilioStatusRepositoryError::Database(e.to_string()))?;

        let now = Self::get_timestamp();

        diesel::update(
            message_status_log::table.filter(message_status_log::message_sid.eq(message_sid)),
        )
        .set((
            message_status_log::status.eq(&update.status),
            message_status_log::error_code.eq(&update.error_code),
            message_status_log::error_message.eq(&update.error_message),
            message_status_log::price.eq(update.price),
            message_status_log::price_unit.eq(&update.price_unit),
            message_status_log::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| TwilioStatusRepositoryError::Database(e.to_string()))
    }

    fn update_message_price(
        &self,
        message_sid: &str,
        price: f32,
        price_unit: &str,
    ) -> Result<(), TwilioStatusRepositoryError> {
        let mut conn = self
            .db_pool
            .get()
            .map_err(|e| TwilioStatusRepositoryError::Database(e.to_string()))?;

        let now = Self::get_timestamp();

        diesel::update(
            message_status_log::table.filter(message_status_log::message_sid.eq(message_sid)),
        )
        .set((
            message_status_log::price.eq(price),
            message_status_log::price_unit.eq(price_unit),
            message_status_log::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| TwilioStatusRepositoryError::Database(e.to_string()))?;

        Ok(())
    }

    fn get_message_user_info(
        &self,
        message_sid: &str,
    ) -> Result<Option<MessageUserInfo>, TwilioStatusRepositoryError> {
        let mut conn = self
            .db_pool
            .get()
            .map_err(|e| TwilioStatusRepositoryError::Database(e.to_string()))?;

        let result: Option<(i32, String, Option<String>)> = message_status_log::table
            .filter(message_status_log::message_sid.eq(message_sid))
            .select((
                message_status_log::user_id,
                message_status_log::to_number,
                message_status_log::from_number,
            ))
            .first(&mut conn)
            .optional()
            .map_err(|e| TwilioStatusRepositoryError::Database(e.to_string()))?;

        Ok(result.map(|(user_id, to_number, from_number)| MessageUserInfo {
            user_id,
            to_number,
            from_number,
        }))
    }
}
