//! Real implementation of TwilioStatusRepository using Diesel.
//!
//! This implementation wraps a database connection pool and performs
//! actual database operations.

use std::time::{SystemTime, UNIX_EPOCH};

use diesel::prelude::*;
use diesel::sql_types::{Integer, Nullable, Text};

use crate::pg_schema::message_status_log;
use crate::repositories::twilio_status_repository::{
    DeliveryFallbackMessage, MessageUserInfo, StatusUpdate, TwilioStatusRepository,
    TwilioStatusRepositoryError,
};
use crate::PgDbPool;

/// Real repository implementation using Diesel.
pub struct DieselTwilioStatusRepository {
    db_pool: PgDbPool,
}

impl DieselTwilioStatusRepository {
    /// Create a new repository with the given database pool.
    pub fn new(db_pool: PgDbPool) -> Self {
        Self { db_pool }
    }

    fn get_timestamp() -> i32 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32
    }

    /// Atomically claim a failed Twilio message for Telnyx fallback.
    ///
    /// The `fallback_attempted_at IS NULL` guard keeps duplicate Twilio
    /// callback retries from sending the same fallback SMS more than once.
    pub fn claim_telnyx_delivery_fallback(
        &self,
        message_sid: &str,
    ) -> Result<Option<DeliveryFallbackMessage>, TwilioStatusRepositoryError> {
        #[derive(QueryableByName)]
        struct ClaimRow {
            #[diesel(sql_type = Integer)]
            user_id: i32,
            #[diesel(sql_type = Text)]
            to_number: String,
            #[diesel(sql_type = Nullable<Text>)]
            encrypted_body: Option<String>,
        }

        let mut conn = self
            .db_pool
            .get()
            .map_err(|e| TwilioStatusRepositoryError::Database(e.to_string()))?;
        let now = Self::get_timestamp();

        let rows: Vec<ClaimRow> = diesel::sql_query(
            "UPDATE message_status_log \
             SET fallback_attempted_at = $2, \
                 fallback_provider = 'telnyx', \
                 fallback_message_sid = NULL, \
                 fallback_error = NULL, \
                 updated_at = $2 \
             WHERE message_sid = $1 \
               AND fallback_attempted_at IS NULL \
             RETURNING user_id, to_number, encrypted_body",
        )
        .bind::<Text, _>(message_sid)
        .bind::<Integer, _>(now)
        .load(&mut conn)
        .map_err(|e| TwilioStatusRepositoryError::Database(e.to_string()))?;

        Ok(rows.into_iter().next().map(|row| DeliveryFallbackMessage {
            user_id: row.user_id,
            to_number: row.to_number,
            encrypted_body: row.encrypted_body,
        }))
    }

    pub fn record_telnyx_delivery_fallback_success(
        &self,
        message_sid: &str,
        fallback_message_sid: &str,
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
            message_status_log::fallback_message_sid.eq(fallback_message_sid),
            message_status_log::fallback_error.eq(Option::<String>::None),
            message_status_log::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| TwilioStatusRepositoryError::Database(e.to_string()))?;

        Ok(())
    }

    pub fn record_telnyx_delivery_fallback_error(
        &self,
        message_sid: &str,
        error: &str,
    ) -> Result<(), TwilioStatusRepositoryError> {
        let mut conn = self
            .db_pool
            .get()
            .map_err(|e| TwilioStatusRepositoryError::Database(e.to_string()))?;
        let now = Self::get_timestamp();
        let error = error.chars().take(1000).collect::<String>();

        diesel::update(
            message_status_log::table.filter(message_status_log::message_sid.eq(message_sid)),
        )
        .set((
            message_status_log::fallback_error.eq(Some(error)),
            message_status_log::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .map_err(|e| TwilioStatusRepositoryError::Database(e.to_string()))?;

        Ok(())
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

        Ok(
            result.map(|(user_id, to_number, from_number)| MessageUserInfo {
                user_id,
                to_number,
                from_number,
            }),
        )
    }
}
