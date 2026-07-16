use crate::pg_models::{NewTuwunelCleanupEvent, TuwunelCleanupEvent};
use crate::pg_schema::tuwunel_cleanup_events;
use crate::PgDbPool;
use anyhow::{anyhow, Result};
use diesel::prelude::*;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

pub const STATUS_PENDING_PURGE: &str = "pending_purge";
pub const STATUS_INGESTING: &str = "ingesting";
pub const STATUS_INGEST_FAILED: &str = "ingest_failed";
pub const STATUS_PURGE_ATTEMPTING: &str = "purge_attempting";
pub const STATUS_PURGE_SUBMITTED: &str = "purge_submitted";
pub const STATUS_PURGE_RETRYING: &str = "purge_retrying";
pub const STATUS_PURGE_SUCCEEDED: &str = "purge_succeeded";
pub const STATUS_PURGE_EXHAUSTED: &str = "purge_exhausted";

const MAX_ERROR_LEN: usize = 4000;
const MAX_DUE_SCAN: i64 = 5000;

type PgPooledConnection =
    diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>;

pub struct TuwunelCleanupRepository {
    pool: PgDbPool,
}

impl TuwunelCleanupRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    pub fn record_ingesting(
        &self,
        user_id: i32,
        service: &str,
        room_id: &str,
        event_id: &str,
        delete_media: bool,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        let entry = NewTuwunelCleanupEvent {
            user_id,
            ontology_message_id: 0,
            service: service.to_string(),
            room_id: room_id.to_string(),
            event_id: event_id.to_string(),
            delete_media,
            commands_expected: 1,
            commands_accepted: 0,
            attempt_count: 0,
            status: STATUS_INGESTING.to_string(),
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
            commands_expected: 1,
            commands_accepted: 0,
            attempt_count: 0,
            status: STATUS_PENDING_PURGE.to_string(),
            last_command_kind: None,
            last_admin_room_id: None,
            last_admin_command_event_id: None,
            last_error: None,
            enqueued_at: now,
            last_attempted_at: None,
            completed_at: None,
            updated_at: now,
        };

        let updated = diesel::update(
            tuwunel_cleanup_events::table.filter(tuwunel_cleanup_events::event_id.eq(event_id)),
        )
        .set((
            tuwunel_cleanup_events::ontology_message_id.eq(ontology_message_id),
            tuwunel_cleanup_events::delete_media.eq(delete_media),
            tuwunel_cleanup_events::commands_expected.eq(1),
            tuwunel_cleanup_events::commands_accepted.eq(0),
            tuwunel_cleanup_events::attempt_count.eq(0),
            tuwunel_cleanup_events::status.eq(STATUS_PENDING_PURGE),
            tuwunel_cleanup_events::last_error.eq(None::<String>),
            tuwunel_cleanup_events::completed_at.eq(None::<i32>),
            tuwunel_cleanup_events::updated_at.eq(now),
        ))
        .execute(&mut conn)?;

        if updated == 0 {
            diesel::insert_into(tuwunel_cleanup_events::table)
                .values(&entry)
                .execute(&mut conn)?;
        }

        Ok(())
    }

    pub fn record_ingest_failed(&self, event_id: &str, error: &str) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        diesel::update(
            tuwunel_cleanup_events::table.filter(tuwunel_cleanup_events::event_id.eq(event_id)),
        )
        .set((
            tuwunel_cleanup_events::status.eq(STATUS_INGEST_FAILED),
            tuwunel_cleanup_events::last_error.eq(Some(trim_error(error))),
            tuwunel_cleanup_events::updated_at.eq(now),
        ))
        .execute(&mut conn)?;
        Ok(())
    }

    pub fn list_due_room_boundaries(
        &self,
        cutoff: i32,
        limit: usize,
    ) -> Result<Vec<TuwunelCleanupEvent>> {
        let mut conn = self.connection()?;
        let blocked_rooms: HashSet<String> = tuwunel_cleanup_events::table
            .filter(tuwunel_cleanup_events::status.eq_any([
                STATUS_INGESTING,
                STATUS_INGEST_FAILED,
                STATUS_PURGE_SUBMITTED,
            ]))
            .select(tuwunel_cleanup_events::room_id)
            .load::<String>(&mut conn)?
            .into_iter()
            .collect();

        let candidates = tuwunel_cleanup_events::table
            .filter(tuwunel_cleanup_events::status.eq_any([
                STATUS_PENDING_PURGE,
                STATUS_PURGE_ATTEMPTING,
                STATUS_PURGE_RETRYING,
            ]))
            .filter(tuwunel_cleanup_events::enqueued_at.le(cutoff))
            .order(tuwunel_cleanup_events::enqueued_at.desc())
            .limit(MAX_DUE_SCAN)
            .select(TuwunelCleanupEvent::as_select())
            .load::<TuwunelCleanupEvent>(&mut conn)?;

        let mut seen_rooms = HashSet::new();
        Ok(candidates
            .into_iter()
            .filter(|candidate| !blocked_rooms.contains(&candidate.room_id))
            .filter(|candidate| seen_rooms.insert(candidate.room_id.clone()))
            .take(limit)
            .collect())
    }

    pub fn list_submitted(&self, limit: i64) -> Result<Vec<TuwunelCleanupEvent>> {
        let mut conn = self.connection()?;
        Ok(tuwunel_cleanup_events::table
            .filter(tuwunel_cleanup_events::status.eq(STATUS_PURGE_SUBMITTED))
            .order(tuwunel_cleanup_events::updated_at.asc())
            .limit(limit)
            .select(TuwunelCleanupEvent::as_select())
            .load::<TuwunelCleanupEvent>(&mut conn)?)
    }

    pub fn record_attempt(&self, event_id: &str, attempt: i32) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        diesel::update(
            tuwunel_cleanup_events::table.filter(tuwunel_cleanup_events::event_id.eq(event_id)),
        )
        .set((
            tuwunel_cleanup_events::attempt_count.eq(attempt),
            tuwunel_cleanup_events::status.eq(STATUS_PURGE_ATTEMPTING),
            tuwunel_cleanup_events::last_attempted_at.eq(Some(now)),
            tuwunel_cleanup_events::updated_at.eq(now),
        ))
        .execute(&mut conn)?;
        Ok(())
    }

    pub fn record_submitted(&self, event_id: &str, purge_id: &str) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        diesel::update(
            tuwunel_cleanup_events::table.filter(tuwunel_cleanup_events::event_id.eq(event_id)),
        )
        .set((
            tuwunel_cleanup_events::status.eq(STATUS_PURGE_SUBMITTED),
            tuwunel_cleanup_events::last_command_kind.eq(Some("purge_history".to_string())),
            tuwunel_cleanup_events::last_admin_command_event_id.eq(Some(purge_id.to_string())),
            tuwunel_cleanup_events::last_error.eq(None::<String>),
            tuwunel_cleanup_events::updated_at.eq(now),
        ))
        .execute(&mut conn)?;
        Ok(())
    }

    pub fn record_retrying(&self, event_id: &str, attempt: i32, error: &str) -> Result<()> {
        self.record_failure(event_id, attempt, STATUS_PURGE_RETRYING, error, false)
    }

    pub fn record_exhausted(&self, event_id: &str, attempt: i32, error: &str) -> Result<()> {
        self.record_failure(event_id, attempt, STATUS_PURGE_EXHAUSTED, error, true)
    }

    pub fn record_room_succeeded_through(&self, room_id: &str, enqueued_at: i32) -> Result<usize> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        let statuses = [
            STATUS_PENDING_PURGE,
            STATUS_PURGE_ATTEMPTING,
            STATUS_PURGE_SUBMITTED,
            STATUS_PURGE_RETRYING,
        ];
        Ok(diesel::update(
            tuwunel_cleanup_events::table
                .filter(tuwunel_cleanup_events::room_id.eq(room_id))
                .filter(tuwunel_cleanup_events::enqueued_at.le(enqueued_at))
                .filter(tuwunel_cleanup_events::status.eq_any(statuses)),
        )
        .set((
            tuwunel_cleanup_events::status.eq(STATUS_PURGE_SUCCEEDED),
            tuwunel_cleanup_events::commands_accepted.eq(tuwunel_cleanup_events::commands_expected),
            tuwunel_cleanup_events::last_error.eq(None::<String>),
            tuwunel_cleanup_events::completed_at.eq(Some(now)),
            tuwunel_cleanup_events::updated_at.eq(now),
        ))
        .execute(&mut conn)?)
    }

    fn record_failure(
        &self,
        event_id: &str,
        attempt: i32,
        status: &str,
        error: &str,
        completed: bool,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        diesel::update(
            tuwunel_cleanup_events::table.filter(tuwunel_cleanup_events::event_id.eq(event_id)),
        )
        .set((
            tuwunel_cleanup_events::attempt_count.eq(attempt),
            tuwunel_cleanup_events::status.eq(status),
            tuwunel_cleanup_events::last_error.eq(Some(trim_error(error))),
            tuwunel_cleanup_events::completed_at.eq(if completed { Some(now) } else { None }),
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

pub fn now_timestamp() -> i32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32
}

fn trim_error(error: &str) -> String {
    error.chars().take(MAX_ERROR_LEN).collect()
}
