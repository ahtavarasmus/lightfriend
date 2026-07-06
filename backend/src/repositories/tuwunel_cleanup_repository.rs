use crate::pg_models::NewTuwunelCleanupEvent;
use crate::pg_schema::tuwunel_cleanup_events;
use crate::PgDbPool;
use anyhow::{anyhow, Result};
use diesel::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};

pub const STATUS_ENQUEUED: &str = "enqueued";
pub const STATUS_ATTEMPTING: &str = "attempting";
pub const STATUS_COMMAND_ACCEPTED: &str = "command_accepted";
pub const STATUS_RETRYING: &str = "retrying";
pub const STATUS_EXHAUSTED: &str = "exhausted";
pub const STATUS_COMMANDS_SUBMITTED: &str = "commands_submitted";
pub const STATUS_PARTIAL_COMMANDS_SUBMITTED: &str = "partial_commands_submitted";

const MAX_ERROR_LEN: usize = 4000;

type PgPooledConnection =
    diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>;

pub struct TuwunelCleanupRepository {
    pool: PgDbPool,
}

impl TuwunelCleanupRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_enqueued(
        &self,
        user_id: i32,
        ontology_message_id: i64,
        service: &str,
        room_id: &str,
        event_id: &str,
        delete_media: bool,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        let entry = NewTuwunelCleanupEvent {
            user_id,
            ontology_message_id,
            service: service.to_string(),
            room_id: room_id.to_string(),
            event_id: event_id.to_string(),
            delete_media,
            commands_expected: if delete_media { 2 } else { 1 },
            commands_accepted: 0,
            attempt_count: 0,
            status: STATUS_ENQUEUED.to_string(),
            last_command_kind: None,
            last_admin_room_id: None,
            last_admin_command_event_id: None,
            last_error: None,
            enqueued_at: now,
            last_attempted_at: None,
            completed_at: None,
            updated_at: now,
        };

        diesel::insert_into(tuwunel_cleanup_events::table)
            .values(&entry)
            .on_conflict(tuwunel_cleanup_events::event_id)
            .do_nothing()
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn record_attempt(&self, event_id: &str, attempt: u8) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();

        diesel::update(
            tuwunel_cleanup_events::table.filter(tuwunel_cleanup_events::event_id.eq(event_id)),
        )
        .set((
            tuwunel_cleanup_events::attempt_count.eq(i32::from(attempt)),
            tuwunel_cleanup_events::status.eq(STATUS_ATTEMPTING),
            tuwunel_cleanup_events::last_attempted_at.eq(Some(now)),
            tuwunel_cleanup_events::updated_at.eq(now),
        ))
        .execute(&mut conn)?;

        Ok(())
    }

    pub fn record_command_accepted(
        &self,
        event_id: &str,
        command_kind: &str,
        admin_room_id: &str,
        admin_command_event_id: &str,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();

        diesel::update(
            tuwunel_cleanup_events::table.filter(tuwunel_cleanup_events::event_id.eq(event_id)),
        )
        .set((
            tuwunel_cleanup_events::commands_accepted
                .eq(tuwunel_cleanup_events::commands_accepted + 1),
            tuwunel_cleanup_events::status.eq(STATUS_COMMAND_ACCEPTED),
            tuwunel_cleanup_events::last_command_kind.eq(Some(command_kind.to_string())),
            tuwunel_cleanup_events::last_admin_room_id.eq(Some(admin_room_id.to_string())),
            tuwunel_cleanup_events::last_admin_command_event_id
                .eq(Some(admin_command_event_id.to_string())),
            tuwunel_cleanup_events::last_error.eq(None::<String>),
            tuwunel_cleanup_events::updated_at.eq(now),
        ))
        .execute(&mut conn)?;

        Ok(())
    }

    pub fn record_retrying(&self, event_id: &str, attempt: u8, error: &str) -> Result<()> {
        self.record_failure(event_id, attempt, STATUS_RETRYING, error, false)
    }

    pub fn record_exhausted(&self, event_id: &str, attempt: u8, error: &str) -> Result<()> {
        self.record_failure(event_id, attempt, STATUS_EXHAUSTED, error, true)
    }

    pub fn record_commands_submitted(&self, event_id: &str) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();

        diesel::update(
            tuwunel_cleanup_events::table.filter(tuwunel_cleanup_events::event_id.eq(event_id)),
        )
        .set((
            tuwunel_cleanup_events::status.eq(STATUS_COMMANDS_SUBMITTED),
            tuwunel_cleanup_events::last_error.eq(None::<String>),
            tuwunel_cleanup_events::completed_at.eq(Some(now)),
            tuwunel_cleanup_events::updated_at.eq(now),
        ))
        .execute(&mut conn)?;

        Ok(())
    }

    pub fn record_partial_commands_submitted(&self, event_id: &str, error: &str) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();

        diesel::update(
            tuwunel_cleanup_events::table.filter(tuwunel_cleanup_events::event_id.eq(event_id)),
        )
        .set((
            tuwunel_cleanup_events::status.eq(STATUS_PARTIAL_COMMANDS_SUBMITTED),
            tuwunel_cleanup_events::last_error.eq(Some(trim_error(error))),
            tuwunel_cleanup_events::completed_at.eq(Some(now)),
            tuwunel_cleanup_events::updated_at.eq(now),
        ))
        .execute(&mut conn)?;

        Ok(())
    }

    fn record_failure(
        &self,
        event_id: &str,
        attempt: u8,
        status: &str,
        error: &str,
        completed: bool,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        let completed_at = if completed { Some(now) } else { None };

        diesel::update(
            tuwunel_cleanup_events::table.filter(tuwunel_cleanup_events::event_id.eq(event_id)),
        )
        .set((
            tuwunel_cleanup_events::attempt_count.eq(i32::from(attempt)),
            tuwunel_cleanup_events::status.eq(status),
            tuwunel_cleanup_events::last_error.eq(Some(trim_error(error))),
            tuwunel_cleanup_events::completed_at.eq(completed_at),
            tuwunel_cleanup_events::updated_at.eq(now),
        ))
        .execute(&mut conn)?;

        Ok(())
    }

    fn connection(&self) -> Result<PgPooledConnection> {
        self.pool
            .get()
            .map_err(|e| anyhow!("failed to get DB connection: {}", e))
    }
}

fn now_timestamp() -> i32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32
}

fn trim_error(error: &str) -> String {
    error.chars().take(MAX_ERROR_LEN).collect()
}
