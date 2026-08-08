use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::pg_models::ByotVerification;
use crate::pg_schema::{byot_verifications, user_secrets, users};
use crate::PgDbPool;

#[derive(Clone)]
pub struct ByotRepository {
    pool: PgDbPool,
}

impl ByotRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    pub fn get(&self, user_id: i32) -> Result<Option<ByotVerification>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        byot_verifications::table
            .find(user_id)
            .first(&mut conn)
            .optional()
    }

    /// Any edited phone number or credential invalidates the prior proof.
    /// Disabling routing and recording the state happen in one transaction.
    pub fn mark_configuration_changed(
        &self,
        user_id: i32,
        phone_number: &str,
    ) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let attempt_id = uuid::Uuid::new_v4().to_string();
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        conn.transaction(|conn| {
            diesel::sql_query(
                "INSERT INTO byot_verifications \
                 (user_id, phone_number, status, attempt_id, last_checked_at, error_code, updated_at) \
                 VALUES ($1, $2, 'error', $3, $4, 'configuration_changed', $4) \
                 ON CONFLICT (user_id) DO UPDATE SET phone_number = EXCLUDED.phone_number, \
                 phone_sid = NULL, status = 'error', attempt_id = EXCLUDED.attempt_id, \
                 configured_at = NULL, verified_at = NULL, last_checked_at = EXCLUDED.last_checked_at, \
                 error_code = EXCLUDED.error_code, updated_at = EXCLUDED.updated_at",
            )
            .bind::<diesel::sql_types::Integer, _>(user_id)
            .bind::<diesel::sql_types::Text, _>(phone_number)
            .bind::<diesel::sql_types::Text, _>(&attempt_id)
            .bind::<diesel::sql_types::Integer, _>(now)
            .execute(conn)?;
            diesel::update(users::table.find(user_id))
                .set(users::own_twilio_enabled.eq(false))
                .execute(conn)?;
            Ok(())
        })
    }

    pub fn update_phone_and_invalidate(
        &self,
        user_id: i32,
        phone_number: &str,
    ) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let attempt_id = uuid::Uuid::new_v4().to_string();
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        conn.transaction(|conn| {
            diesel::update(users::table.find(user_id))
                .set((
                    users::preferred_number.eq(Some(phone_number)),
                    users::own_twilio_enabled.eq(false),
                ))
                .execute(conn)?;
            upsert_changed_verification(conn, user_id, phone_number, &attempt_id, now)?;
            Ok(())
        })
    }

    pub fn replace_credentials_and_invalidate(
        &self,
        user_id: i32,
        phone_number: &str,
        encrypted_account_sid: &str,
        encrypted_auth_token: &str,
    ) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let attempt_id = uuid::Uuid::new_v4().to_string();
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        conn.transaction(|conn| {
            diesel::sql_query(
                "INSERT INTO user_secrets \
                 (user_id, encrypted_twilio_account_sid, encrypted_twilio_auth_token) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (user_id) DO UPDATE SET \
                 encrypted_twilio_account_sid = EXCLUDED.encrypted_twilio_account_sid, \
                 encrypted_twilio_auth_token = EXCLUDED.encrypted_twilio_auth_token",
            )
            .bind::<diesel::sql_types::Integer, _>(user_id)
            .bind::<diesel::sql_types::Text, _>(encrypted_account_sid)
            .bind::<diesel::sql_types::Text, _>(encrypted_auth_token)
            .execute(conn)?;
            diesel::update(users::table.find(user_id))
                .set(users::own_twilio_enabled.eq(false))
                .execute(conn)?;
            upsert_changed_verification(conn, user_id, phone_number, &attempt_id, now)?;
            Ok(())
        })
    }

    pub fn clear_credentials_and_invalidate(
        &self,
        user_id: i32,
        phone_number: &str,
    ) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let attempt_id = uuid::Uuid::new_v4().to_string();
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        conn.transaction(|conn| {
            diesel::update(user_secrets::table.filter(user_secrets::user_id.eq(user_id)))
                .set((
                    user_secrets::encrypted_twilio_account_sid.eq::<Option<String>>(None),
                    user_secrets::encrypted_twilio_auth_token.eq::<Option<String>>(None),
                ))
                .execute(conn)?;
            diesel::update(users::table.find(user_id))
                .set(users::own_twilio_enabled.eq(false))
                .execute(conn)?;
            upsert_changed_verification(conn, user_id, phone_number, &attempt_id, now)?;
            Ok(())
        })
    }

    /// A new attempt immediately disables BYOT. Only the same attempt can
    /// later transition both verification state and routing to enabled.
    pub fn start_attempt(&self, user_id: i32, phone_number: &str) -> Result<String, DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let attempt_id = uuid::Uuid::new_v4().to_string();
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        conn.transaction(|conn| {
            diesel::sql_query(
                "INSERT INTO byot_verifications \
                 (user_id, phone_number, status, attempt_id, last_checked_at, updated_at) \
                 VALUES ($1, $2, 'configuring', $3, $4, $4) \
                 ON CONFLICT (user_id) DO UPDATE SET phone_number = EXCLUDED.phone_number, \
                 phone_sid = NULL, status = 'configuring', attempt_id = EXCLUDED.attempt_id, \
                 configured_at = NULL, verified_at = NULL, last_checked_at = EXCLUDED.last_checked_at, \
                 error_code = NULL, updated_at = EXCLUDED.updated_at",
            )
            .bind::<diesel::sql_types::Integer, _>(user_id)
            .bind::<diesel::sql_types::Text, _>(phone_number)
            .bind::<diesel::sql_types::Text, _>(&attempt_id)
            .bind::<diesel::sql_types::Integer, _>(now)
            .execute(conn)?;
            diesel::update(users::table.find(user_id))
                .set(users::own_twilio_enabled.eq(false))
                .execute(conn)?;
            Ok(attempt_id.clone())
        })
    }

    pub fn activate_if_current(
        &self,
        user_id: i32,
        attempt_id: &str,
        phone_sid: &str,
    ) -> Result<bool, DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        conn.transaction(|conn| {
            let affected = diesel::update(
                byot_verifications::table
                    .find(user_id)
                    .filter(byot_verifications::attempt_id.eq(attempt_id))
                    .filter(byot_verifications::status.eq("configuring")),
            )
            .set((
                byot_verifications::phone_sid.eq(Some(phone_sid)),
                byot_verifications::status.eq("verified"),
                byot_verifications::configured_at.eq(Some(now)),
                byot_verifications::verified_at.eq(Some(now)),
                byot_verifications::last_checked_at.eq(now),
                byot_verifications::error_code.eq::<Option<String>>(None),
                byot_verifications::updated_at.eq(now),
            ))
            .execute(conn)?;
            if affected != 1 {
                return Ok(false);
            }
            diesel::update(users::table.find(user_id))
                .set(users::own_twilio_enabled.eq(true))
                .execute(conn)?;
            Ok(true)
        })
    }

    pub fn fail_if_current(
        &self,
        user_id: i32,
        attempt_id: &str,
        error_code: &str,
    ) -> Result<bool, DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        Ok(diesel::update(
            byot_verifications::table
                .find(user_id)
                .filter(byot_verifications::attempt_id.eq(attempt_id))
                .filter(byot_verifications::status.eq("configuring")),
        )
        .set((
            byot_verifications::status.eq("error"),
            byot_verifications::last_checked_at.eq(now),
            byot_verifications::error_code.eq(Some(error_code)),
            byot_verifications::updated_at.eq(now),
        ))
        .execute(&mut conn)?
            == 1)
    }

    pub fn mark_drifted(&self, user_id: i32, error_code: &str) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        conn.transaction(|conn| {
            diesel::update(byot_verifications::table.find(user_id))
                .set((
                    byot_verifications::status.eq("drifted"),
                    byot_verifications::last_checked_at.eq(now),
                    byot_verifications::error_code.eq(Some(error_code)),
                    byot_verifications::updated_at.eq(now),
                ))
                .execute(conn)?;
            diesel::update(users::table.find(user_id))
                .set(users::own_twilio_enabled.eq(false))
                .execute(conn)?;
            Ok(())
        })
    }

    pub fn mark_checked(&self, user_id: i32) -> Result<bool, DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        let affected = diesel::update(
            byot_verifications::table
                .find(user_id)
                .filter(byot_verifications::status.eq("verified")),
        )
        .set((
            byot_verifications::last_checked_at.eq(now),
            byot_verifications::error_code.eq::<Option<String>>(None),
            byot_verifications::updated_at.eq(now),
        ))
        .execute(&mut conn)?;
        Ok(affected == 1)
    }
}

fn upsert_changed_verification(
    conn: &mut PgConnection,
    user_id: i32,
    phone_number: &str,
    attempt_id: &str,
    now: i32,
) -> Result<(), DieselError> {
    diesel::sql_query(
        "INSERT INTO byot_verifications \
         (user_id, phone_number, status, attempt_id, last_checked_at, error_code, updated_at) \
         VALUES ($1, $2, 'error', $3, $4, 'configuration_changed', $4) \
         ON CONFLICT (user_id) DO UPDATE SET phone_number = EXCLUDED.phone_number, \
         phone_sid = NULL, status = 'error', attempt_id = EXCLUDED.attempt_id, \
         configured_at = NULL, verified_at = NULL, last_checked_at = EXCLUDED.last_checked_at, \
         error_code = EXCLUDED.error_code, updated_at = EXCLUDED.updated_at",
    )
    .bind::<diesel::sql_types::Integer, _>(user_id)
    .bind::<diesel::sql_types::Text, _>(phone_number)
    .bind::<diesel::sql_types::Text, _>(attempt_id)
    .bind::<diesel::sql_types::Integer, _>(now)
    .execute(conn)?;
    Ok(())
}
