use crate::{
    models::light_tool_models::{LightToolPushOutboxEvent, NewLightToolPushOutboxEvent},
    pg_schema::light_tool_push_outbox,
    PgDbPool,
};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use thiserror::Error;

pub const CONVERSATION_CHANGED_EVENT: &str = "conversation_changed";

#[derive(Debug, Error)]
pub enum LightToolPushOutboxRepositoryError {
    #[error(transparent)]
    Database(#[from] DieselError),
}

pub struct LightToolPushOutboxRepository {
    pool: PgDbPool,
}

impl LightToolPushOutboxRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    pub fn enqueue_conversation_changed(
        &self,
        device_id: i32,
        now: i32,
    ) -> Result<LightToolPushOutboxEvent, LightToolPushOutboxRepositoryError> {
        let event = NewLightToolPushOutboxEvent {
            device_id,
            event_type: CONVERSATION_CHANGED_EVENT.to_string(),
            version: 1,
            attempt_count: 0,
            next_attempt_at: now,
            lease_until: 0,
            created_at: now,
            updated_at: now,
        };
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(light_tool_push_outbox::table)
            .values(&event)
            .on_conflict(light_tool_push_outbox::device_id)
            .do_update()
            .set((
                light_tool_push_outbox::event_type.eq(CONVERSATION_CHANGED_EVENT),
                light_tool_push_outbox::version.eq(light_tool_push_outbox::version + 1_i64),
                light_tool_push_outbox::updated_at.eq(now),
            ))
            .get_result::<LightToolPushOutboxEvent>(&mut conn)
            .map_err(Into::into)
    }

    pub fn claim_due(
        &self,
        now: i32,
        lease_until: i32,
        limit: i64,
    ) -> Result<Vec<LightToolPushOutboxEvent>, LightToolPushOutboxRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        conn.transaction::<_, DieselError, _>(|conn| {
            let device_ids = light_tool_push_outbox::table
                .filter(light_tool_push_outbox::next_attempt_at.le(now))
                .filter(light_tool_push_outbox::lease_until.le(now))
                .order((
                    light_tool_push_outbox::next_attempt_at.asc(),
                    light_tool_push_outbox::device_id.asc(),
                ))
                .select(light_tool_push_outbox::device_id)
                .for_update()
                .skip_locked()
                .limit(limit)
                .load::<i32>(conn)?;

            if device_ids.is_empty() {
                return Ok(Vec::new());
            }

            diesel::update(
                light_tool_push_outbox::table
                    .filter(light_tool_push_outbox::device_id.eq_any(device_ids)),
            )
            .set(light_tool_push_outbox::lease_until.eq(lease_until))
            .returning(LightToolPushOutboxEvent::as_returning())
            .load(conn)
        })
        .map_err(Into::into)
    }

    pub fn complete(
        &self,
        device_id: i32,
        expected_version: i64,
    ) -> Result<bool, LightToolPushOutboxRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        Ok(diesel::delete(
            light_tool_push_outbox::table
                .filter(light_tool_push_outbox::device_id.eq(device_id))
                .filter(light_tool_push_outbox::version.eq(expected_version)),
        )
        .execute(&mut conn)?
            > 0)
    }

    pub fn schedule_retry(
        &self,
        device_id: i32,
        expected_version: i64,
        next_attempt_at: i32,
        now: i32,
    ) -> Result<bool, LightToolPushOutboxRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        Ok(diesel::update(
            light_tool_push_outbox::table
                .filter(light_tool_push_outbox::device_id.eq(device_id))
                .filter(light_tool_push_outbox::version.eq(expected_version)),
        )
        .set((
            light_tool_push_outbox::attempt_count.eq(light_tool_push_outbox::attempt_count + 1),
            light_tool_push_outbox::next_attempt_at.eq(next_attempt_at),
            light_tool_push_outbox::lease_until.eq(0),
            light_tool_push_outbox::updated_at.eq(now),
        ))
        .execute(&mut conn)?
            > 0)
    }

    pub fn find_for_device(
        &self,
        device_id: i32,
    ) -> Result<Option<LightToolPushOutboxEvent>, LightToolPushOutboxRepositoryError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        light_tool_push_outbox::table
            .find(device_id)
            .select(LightToolPushOutboxEvent::as_select())
            .first(&mut conn)
            .optional()
            .map_err(Into::into)
    }
}
