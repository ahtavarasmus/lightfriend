use crate::{
    models::light_tool_models::{LightToolDevice, NewLightToolDevice},
    pg_schema::light_tool_devices,
    PgDbPool,
};
use diesel::prelude::*;
use diesel::result::Error as DieselError;

pub struct LightToolDevicesRepository {
    pool: PgDbPool,
}

impl LightToolDevicesRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, device: &NewLightToolDevice) -> Result<LightToolDevice, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(light_tool_devices::table)
            .values(device)
            .get_result::<LightToolDevice>(&mut conn)
    }

    pub fn find_by_installation_hash(
        &self,
        installation_id_hash: &str,
    ) -> Result<Option<LightToolDevice>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        light_tool_devices::table
            .filter(light_tool_devices::installation_id_hash.eq(installation_id_hash))
            .select(LightToolDevice::as_select())
            .first::<LightToolDevice>(&mut conn)
            .optional()
    }

    pub fn find_active_by_token_hash(
        &self,
        device_token_hash: &str,
    ) -> Result<Option<LightToolDevice>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        light_tool_devices::table
            .filter(light_tool_devices::device_token_hash.eq(device_token_hash))
            .filter(light_tool_devices::revoked_at.is_null())
            .select(LightToolDevice::as_select())
            .first::<LightToolDevice>(&mut conn)
            .optional()
    }

    pub fn update_last_seen_if_active(
        &self,
        device_id: i32,
        now: i32,
    ) -> Result<Option<LightToolDevice>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(
            light_tool_devices::table
                .filter(light_tool_devices::id.eq(device_id))
                .filter(light_tool_devices::revoked_at.is_null()),
        )
        .set((
            light_tool_devices::last_seen_at.eq(now),
            light_tool_devices::updated_at.eq(now),
        ))
        .get_result::<LightToolDevice>(&mut conn)
        .optional()
    }

    /// The eligibility predicates and increment share one UPDATE, so concurrent
    /// callers cannot both claim the final available trial message.
    pub fn claim_anonymous_trial_message(
        &self,
        device_id: i32,
        now: i32,
        message_limit: i32,
    ) -> Result<Option<LightToolDevice>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(
            light_tool_devices::table
                .filter(light_tool_devices::id.eq(device_id))
                .filter(light_tool_devices::user_id.is_null())
                .filter(light_tool_devices::revoked_at.is_null())
                .filter(light_tool_devices::trial_expires_at.gt(now))
                .filter(light_tool_devices::trial_messages_used.lt(message_limit)),
        )
        .set((
            light_tool_devices::trial_messages_used.eq(light_tool_devices::trial_messages_used + 1),
            light_tool_devices::updated_at.eq(now),
        ))
        .get_result::<LightToolDevice>(&mut conn)
        .optional()
    }
}
