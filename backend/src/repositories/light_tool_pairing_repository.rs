use crate::{
    models::light_tool_models::{
        LightToolDevice, LightToolPairingSession, NewLightToolPairingSession,
    },
    pg_schema::{light_tool_devices, light_tool_pairing_sessions},
    PgDbPool,
};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairingConsumption {
    Linked { user_id: i32, newly_linked: bool },
    InvalidOrExpired,
    DeviceUnavailable,
    DeviceAlreadyLinked,
}

#[derive(Debug, Error)]
pub enum LightToolPairingRepositoryError {
    #[error(transparent)]
    Database(#[from] DieselError),
}

pub struct LightToolPairingRepository {
    pool: PgDbPool,
}

impl LightToolPairingRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    /// Replaces this account's previous QR session. At most one token per
    /// account can therefore remain valid, including under concurrent writes.
    pub fn upsert_for_user(
        &self,
        session: &NewLightToolPairingSession,
    ) -> Result<LightToolPairingSession, LightToolPairingRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        Ok(diesel::insert_into(light_tool_pairing_sessions::table)
            .values(session)
            .on_conflict(light_tool_pairing_sessions::user_id)
            .do_update()
            .set((
                light_tool_pairing_sessions::token_hash.eq(&session.token_hash),
                light_tool_pairing_sessions::expires_at.eq(session.expires_at),
                light_tool_pairing_sessions::consumed_at.eq::<Option<i32>>(None),
                light_tool_pairing_sessions::consumed_by_device_id.eq::<Option<i32>>(None),
                light_tool_pairing_sessions::created_at.eq(session.created_at),
            ))
            .get_result::<LightToolPairingSession>(&mut conn)?)
    }

    pub fn find_for_user(
        &self,
        user_id: i32,
    ) -> Result<Option<LightToolPairingSession>, LightToolPairingRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        Ok(light_tool_pairing_sessions::table
            .find(user_id)
            .select(LightToolPairingSession::as_select())
            .first::<LightToolPairingSession>(&mut conn)
            .optional()?)
    }

    pub fn consume(
        &self,
        token_hash: &str,
        device_id: i32,
        now: i32,
    ) -> Result<PairingConsumption, LightToolPairingRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        conn.transaction::<PairingConsumption, LightToolPairingRepositoryError, _>(|conn| {
            let session = light_tool_pairing_sessions::table
                .filter(light_tool_pairing_sessions::token_hash.eq(token_hash))
                .for_update()
                .select(LightToolPairingSession::as_select())
                .first::<LightToolPairingSession>(conn)
                .optional()?;
            let Some(session) = session else {
                return Ok(PairingConsumption::InvalidOrExpired);
            };
            if session.expires_at <= now {
                return Ok(PairingConsumption::InvalidOrExpired);
            }

            let device = light_tool_devices::table
                .filter(light_tool_devices::id.eq(device_id))
                .filter(light_tool_devices::revoked_at.is_null())
                .for_update()
                .select(LightToolDevice::as_select())
                .first::<LightToolDevice>(conn)
                .optional()?;
            let Some(device) = device else {
                return Ok(PairingConsumption::DeviceUnavailable);
            };

            if session.consumed_at.is_some() {
                if session.consumed_by_device_id == Some(device_id)
                    && device.user_id == Some(session.user_id)
                {
                    return Ok(PairingConsumption::Linked {
                        user_id: session.user_id,
                        newly_linked: false,
                    });
                }
                return Ok(PairingConsumption::InvalidOrExpired);
            }
            if device.user_id.is_some() {
                return Ok(PairingConsumption::DeviceAlreadyLinked);
            }

            diesel::update(light_tool_devices::table.find(device_id))
                .set((
                    light_tool_devices::user_id.eq(Some(session.user_id)),
                    light_tool_devices::updated_at.eq(now),
                ))
                .execute(conn)?;
            diesel::update(
                light_tool_pairing_sessions::table
                    .filter(light_tool_pairing_sessions::user_id.eq(session.user_id)),
            )
            .set((
                light_tool_pairing_sessions::consumed_at.eq(Some(now)),
                light_tool_pairing_sessions::consumed_by_device_id.eq(Some(device_id)),
            ))
            .execute(conn)?;

            Ok(PairingConsumption::Linked {
                user_id: session.user_id,
                newly_linked: true,
            })
        })
    }
}
