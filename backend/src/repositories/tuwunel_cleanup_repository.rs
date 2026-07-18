use crate::pg_models::{NewTuwunelCleanupEvent, TuwunelCleanupEvent};
use crate::pg_schema::{ont_messages, tuwunel_cleanup_events};
use crate::PgDbPool;
use anyhow::{anyhow, Result};
use diesel::prelude::*;
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

pub const STATUS_PENDING_PURGE: &str = "pending_purge";
pub const STATUS_INGESTING: &str = "ingesting";
pub const STATUS_INGEST_FAILED: &str = "ingest_failed";
pub const STATUS_PURGE_ATTEMPTING: &str = "purge_attempting";
pub const STATUS_PURGE_SUBMITTED: &str = "purge_submitted";
pub const STATUS_PURGE_RETRYING: &str = "purge_retrying";
pub const STATUS_PURGE_SUCCEEDED: &str = "purge_succeeded";
pub const STATUS_PURGE_EXHAUSTED: &str = "purge_exhausted";
pub const STATUS_BACKFILL_AUDIT_VERIFIED: &str = "backfill_audit_verified";
pub const STATUS_BACKFILL_AUDIT_BLOCKED: &str = "backfill_audit_blocked";

const MAX_ERROR_LEN: usize = 4000;
const MAX_DUE_SCAN: i64 = 5000;

#[derive(QueryableByName)]
pub struct HistoricalBackfillCandidate {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub ontology_message_id: i64,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub user_id: i32,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub room_id: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub service: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub event_id: String,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub created_at: i32,
}

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

    pub fn record_intentionally_discarded(
        &self,
        user_id: i32,
        service: &str,
        room_id: &str,
        event_id: &str,
        reason: &str,
    ) -> Result<bool> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        let entry = NewTuwunelCleanupEvent {
            user_id,
            ontology_message_id: 0,
            service: service.to_string(),
            room_id: room_id.to_string(),
            event_id: event_id.to_string(),
            delete_media: true,
            commands_expected: 1,
            commands_accepted: 0,
            attempt_count: 0,
            status: STATUS_PENDING_PURGE.to_string(),
            last_command_kind: Some("intentional_discard".to_string()),
            last_admin_room_id: None,
            last_admin_command_event_id: None,
            last_error: Some(trim_error(&format!("intentional discard: {reason}"))),
            enqueued_at: now,
            last_attempted_at: None,
            completed_at: None,
            updated_at: now,
        };

        Ok(diesel::insert_into(tuwunel_cleanup_events::table)
            .values(&entry)
            .on_conflict(tuwunel_cleanup_events::event_id)
            .do_nothing()
            .execute(&mut conn)?
            > 0)
    }

    pub fn record_unproven_blocker(
        &self,
        user_id: i32,
        service: &str,
        room_id: &str,
        event_id: &str,
        reason: &str,
    ) -> Result<bool> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        let entry = NewTuwunelCleanupEvent {
            user_id,
            ontology_message_id: 0,
            service: service.to_string(),
            room_id: room_id.to_string(),
            event_id: event_id.to_string(),
            delete_media: false,
            commands_expected: 1,
            commands_accepted: 0,
            attempt_count: 0,
            status: STATUS_INGEST_FAILED.to_string(),
            last_command_kind: Some("retained_unproven".to_string()),
            last_admin_room_id: None,
            last_admin_command_event_id: None,
            last_error: Some(trim_error(&format!(
                "retained without ontology proof: {reason}"
            ))),
            enqueued_at: now,
            last_attempted_at: None,
            completed_at: None,
            updated_at: now,
        };

        Ok(diesel::insert_into(tuwunel_cleanup_events::table)
            .values(&entry)
            .on_conflict(tuwunel_cleanup_events::event_id)
            .do_nothing()
            .execute(&mut conn)?
            > 0)
    }

    pub fn list_historical_backfill_candidates(
        &self,
        boundary_cutoff: i32,
        audit_recheck_cutoff: i32,
        limit: usize,
    ) -> Result<Vec<HistoricalBackfillCandidate>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let mut conn = self.connection()?;
        Ok(diesel::sql_query(
            "WITH latest_by_room AS (
                 SELECT DISTINCT ON (room_id)
                        id AS ontology_message_id,
                        user_id,
                        room_id,
                        platform AS service,
                        matrix_event_id AS event_id,
                        created_at
                   FROM ont_messages
                  WHERE matrix_event_id IS NOT NULL
                    AND created_at <= $1
                  ORDER BY room_id, created_at DESC, id DESC
             )
             SELECT latest.ontology_message_id,
                    latest.user_id,
                    latest.room_id,
                    latest.service,
                    latest.event_id,
                    latest.created_at
               FROM latest_by_room latest
               LEFT JOIN tuwunel_cleanup_events cleanup
                 ON cleanup.event_id = latest.event_id
              WHERE (
                        cleanup.id IS NULL
                     OR (
                            cleanup.status IN ('backfill_audit_verified', 'backfill_audit_blocked')
                        AND cleanup.updated_at <= $2
                     )
                    )
                AND NOT EXISTS (
                    SELECT 1
                      FROM tuwunel_cleanup_events blocker
                     WHERE blocker.room_id = latest.room_id
                       AND blocker.status IN ('ingesting', 'ingest_failed')
                )
              ORDER BY (cleanup.id IS NULL) DESC,
                       COALESCE(cleanup.updated_at, 0),
                       latest.ontology_message_id
              LIMIT $3",
        )
        .bind::<diesel::sql_types::Integer, _>(boundary_cutoff)
        .bind::<diesel::sql_types::Integer, _>(audit_recheck_cutoff)
        .bind::<diesel::sql_types::BigInt, _>(i64::try_from(limit).unwrap_or(i64::MAX))
        .load::<HistoricalBackfillCandidate>(&mut conn)?)
    }

    pub fn record_historical_backfill_audit(
        &self,
        candidate: &HistoricalBackfillCandidate,
        verified: bool,
        summary: &str,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        let status = if verified {
            STATUS_BACKFILL_AUDIT_VERIFIED
        } else {
            STATUS_BACKFILL_AUDIT_BLOCKED
        };
        let entry = NewTuwunelCleanupEvent {
            user_id: candidate.user_id,
            ontology_message_id: candidate.ontology_message_id,
            service: candidate.service.clone(),
            room_id: candidate.room_id.clone(),
            event_id: candidate.event_id.clone(),
            delete_media: true,
            commands_expected: 0,
            commands_accepted: 0,
            attempt_count: 0,
            status: status.to_string(),
            last_command_kind: Some("historical_backfill_audit".to_string()),
            last_admin_room_id: None,
            last_admin_command_event_id: None,
            last_error: Some(trim_error(summary)),
            enqueued_at: now,
            last_attempted_at: None,
            completed_at: Some(now),
            updated_at: now,
        };

        diesel::insert_into(tuwunel_cleanup_events::table)
            .values(&entry)
            .on_conflict(tuwunel_cleanup_events::event_id)
            .do_update()
            .set((
                tuwunel_cleanup_events::ontology_message_id.eq(candidate.ontology_message_id),
                tuwunel_cleanup_events::status.eq(status),
                tuwunel_cleanup_events::commands_expected.eq(0),
                tuwunel_cleanup_events::commands_accepted.eq(0),
                tuwunel_cleanup_events::attempt_count.eq(0),
                tuwunel_cleanup_events::last_command_kind.eq(Some("historical_backfill_audit")),
                tuwunel_cleanup_events::last_error.eq(Some(trim_error(summary))),
                tuwunel_cleanup_events::completed_at.eq(Some(now)),
                tuwunel_cleanup_events::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn unproven_event_ids(&self, event_ids: &[String]) -> Result<Vec<String>> {
        if event_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = self.connection()?;
        let ontology_proof: HashSet<String> = ont_messages::table
            .filter(ont_messages::matrix_event_id.eq_any(event_ids))
            .select(ont_messages::matrix_event_id)
            .load::<Option<String>>(&mut conn)?
            .into_iter()
            .flatten()
            .collect();
        let cleanup_proof: HashSet<String> = tuwunel_cleanup_events::table
            .filter(tuwunel_cleanup_events::event_id.eq_any(event_ids))
            .filter(
                tuwunel_cleanup_events::ontology_message_id
                    .gt(0)
                    .or(tuwunel_cleanup_events::last_command_kind.eq("intentional_discard")),
            )
            .select(tuwunel_cleanup_events::event_id)
            .load::<String>(&mut conn)?
            .into_iter()
            .collect();

        Ok(event_ids
            .iter()
            .filter(|event_id| {
                !ontology_proof.contains(*event_id) && !cleanup_proof.contains(*event_id)
            })
            .cloned()
            .collect())
    }

    pub fn enqueue_verified_historical_backfill(
        &self,
        candidate: &HistoricalBackfillCandidate,
        audit_summary: &str,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        let updated = diesel::update(
            tuwunel_cleanup_events::table
                .filter(tuwunel_cleanup_events::event_id.eq(&candidate.event_id))
                .filter(tuwunel_cleanup_events::status.eq(STATUS_BACKFILL_AUDIT_VERIFIED)),
        )
        .set((
            tuwunel_cleanup_events::ontology_message_id.eq(candidate.ontology_message_id),
            tuwunel_cleanup_events::delete_media.eq(true),
            tuwunel_cleanup_events::commands_expected.eq(1),
            tuwunel_cleanup_events::commands_accepted.eq(0),
            tuwunel_cleanup_events::attempt_count.eq(0),
            tuwunel_cleanup_events::status.eq(STATUS_PENDING_PURGE),
            tuwunel_cleanup_events::last_command_kind.eq(Some("historical_backfill_verified")),
            tuwunel_cleanup_events::last_error.eq(Some(trim_error(audit_summary))),
            tuwunel_cleanup_events::enqueued_at.eq(now),
            tuwunel_cleanup_events::last_attempted_at.eq(None::<i32>),
            tuwunel_cleanup_events::completed_at.eq(None::<i32>),
            tuwunel_cleanup_events::updated_at.eq(now),
        ))
        .execute(&mut conn)?;
        if updated != 1 {
            return Err(anyhow!(
                "verified historical boundary {} was not in an auditable state",
                candidate.event_id
            ));
        }
        Ok(())
    }

    pub fn recover_stale_ingest_blockers(&self, cutoff: i32, limit: i64) -> Result<usize> {
        let mut conn = self.connection()?;
        let blockers = tuwunel_cleanup_events::table
            .filter(tuwunel_cleanup_events::status.eq_any([STATUS_INGESTING, STATUS_INGEST_FAILED]))
            .filter(tuwunel_cleanup_events::updated_at.le(cutoff))
            .order(tuwunel_cleanup_events::updated_at.asc())
            .limit(limit)
            .select((tuwunel_cleanup_events::id, tuwunel_cleanup_events::event_id))
            .load::<(i32, String)>(&mut conn)?;
        if blockers.is_empty() {
            return Ok(0);
        }

        let blocker_event_ids: Vec<String> =
            blockers.iter().map(|(_, event)| event.clone()).collect();
        let ontology_rows = ont_messages::table
            .filter(ont_messages::matrix_event_id.eq_any(&blocker_event_ids))
            .select((ont_messages::id, ont_messages::matrix_event_id))
            .load::<(i64, Option<String>)>(&mut conn)?;
        let ontology_ids: HashMap<String, i64> = ontology_rows
            .into_iter()
            .filter_map(|(id, event_id)| event_id.map(|event_id| (event_id, id)))
            .collect();

        let now = now_timestamp();
        let mut recovered = 0;
        for (id, event_id) in blockers {
            let Some(ontology_message_id) = ontology_ids.get(&event_id) else {
                continue;
            };
            recovered += diesel::update(
                tuwunel_cleanup_events::table
                    .filter(tuwunel_cleanup_events::id.eq(id))
                    .filter(
                        tuwunel_cleanup_events::status
                            .eq_any([STATUS_INGESTING, STATUS_INGEST_FAILED]),
                    ),
            )
            .set((
                tuwunel_cleanup_events::ontology_message_id.eq(*ontology_message_id),
                tuwunel_cleanup_events::status.eq(STATUS_PENDING_PURGE),
                tuwunel_cleanup_events::attempt_count.eq(0),
                tuwunel_cleanup_events::last_error.eq(None::<String>),
                tuwunel_cleanup_events::enqueued_at.eq(now),
                tuwunel_cleanup_events::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        }

        Ok(recovered)
    }

    pub fn requeue_exhausted(&self, cutoff: i32, limit: i64) -> Result<usize> {
        let mut conn = self.connection()?;
        let ids = tuwunel_cleanup_events::table
            .filter(tuwunel_cleanup_events::status.eq(STATUS_PURGE_EXHAUSTED))
            .filter(tuwunel_cleanup_events::updated_at.le(cutoff))
            .order(tuwunel_cleanup_events::updated_at.asc())
            .limit(limit)
            .select(tuwunel_cleanup_events::id)
            .load::<i32>(&mut conn)?;
        if ids.is_empty() {
            return Ok(0);
        }

        let now = now_timestamp();
        Ok(diesel::update(
            tuwunel_cleanup_events::table.filter(tuwunel_cleanup_events::id.eq_any(ids)),
        )
        .set((
            tuwunel_cleanup_events::status.eq(STATUS_PURGE_RETRYING),
            tuwunel_cleanup_events::attempt_count.eq(0),
            tuwunel_cleanup_events::last_attempted_at.eq(None::<i32>),
            tuwunel_cleanup_events::completed_at.eq(None::<i32>),
            tuwunel_cleanup_events::updated_at.eq(now),
        ))
        .execute(&mut conn)?)
    }

    pub fn stale_blocker_counts(&self, cutoff: i32) -> Result<Vec<(String, i64)>> {
        let mut conn = self.connection()?;
        Ok(tuwunel_cleanup_events::table
            .filter(tuwunel_cleanup_events::status.eq_any([
                STATUS_INGESTING,
                STATUS_INGEST_FAILED,
                STATUS_PURGE_EXHAUSTED,
            ]))
            .filter(tuwunel_cleanup_events::updated_at.le(cutoff))
            .group_by(tuwunel_cleanup_events::status)
            .select((tuwunel_cleanup_events::status, diesel::dsl::count_star()))
            .load::<(String, i64)>(&mut conn)?)
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

    pub fn has_newer_successful_boundary(
        &self,
        room_id: &str,
        ontology_message_id: i64,
    ) -> Result<bool> {
        let mut conn = self.connection()?;
        Ok(tuwunel_cleanup_events::table
            .filter(tuwunel_cleanup_events::room_id.eq(room_id))
            .filter(tuwunel_cleanup_events::status.eq(STATUS_PURGE_SUCCEEDED))
            .filter(tuwunel_cleanup_events::ontology_message_id.gt(ontology_message_id))
            .select(tuwunel_cleanup_events::id)
            .first::<i32>(&mut conn)
            .optional()?
            .is_some())
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
