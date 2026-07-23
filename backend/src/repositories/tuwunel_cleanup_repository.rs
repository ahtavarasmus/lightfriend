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

pub const BRIDGE_JOB_AUDIT_PENDING: &str = "audit_pending";
pub const BRIDGE_JOB_AUDIT_READY: &str = "audit_ready";
pub const BRIDGE_JOB_PENDING: &str = "pending";
pub const BRIDGE_JOB_RETRYING: &str = "retrying";
pub const BRIDGE_JOB_SUCCEEDED: &str = "succeeded";
pub const BRIDGE_JOB_EXHAUSTED: &str = "exhausted";
pub const BRIDGE_JOB_CANCELLED_RECONNECTED: &str = "cancelled_reconnected";

pub const BRIDGE_ROOM_PENDING: &str = "pending";
pub const BRIDGE_ROOM_AUDIT_READY: &str = "audit_ready";
pub const BRIDGE_ROOM_BLOCKED_ACTIVE: &str = "blocked_active_connection";
pub const BRIDGE_ROOM_DELETING: &str = "deleting";
pub const BRIDGE_ROOM_SUCCEEDED: &str = "succeeded";
pub const BRIDGE_ROOM_RETRYING: &str = "retrying";
pub const BRIDGE_ROOM_EXHAUSTED: &str = "exhausted";

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

#[derive(Debug, QueryableByName)]
pub struct BridgeCleanupJob {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub id: i32,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub user_id: i32,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub bridge_type: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub trigger_kind: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Integer>)]
    pub expected_bridge_id: Option<i32>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Integer>)]
    pub expected_bridge_created_at: Option<i32>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub management_room_id: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub status: String,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub attempt_count: i32,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub portal_cleanup_status: String,
}

#[derive(Debug, QueryableByName)]
pub struct BridgeCleanupRoom {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub id: i32,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub room_id: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub source: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub status: String,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pub attempt_count: i32,
}

#[derive(QueryableByName)]
struct ReturnedId {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    id: i32,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct ReturnedLease {
    #[diesel(sql_type = diesel::sql_types::Text)]
    owner_token: String,
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

    /// Persist a disconnect before its bridge row is removed, then snapshot every
    /// platform room known to the canonical ontology. The bridge generation is
    /// retained so a later reconnect can cancel the destructive work.
    pub fn enqueue_bridge_cleanup(
        &self,
        bridge: &crate::pg_models::PgBridge,
        trigger_kind: &str,
        grace_secs: i32,
    ) -> Result<i32> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        conn.transaction::<i32, anyhow::Error, _>(|conn| {
            let job = diesel::sql_query(
                "INSERT INTO bridge_cleanup_jobs (
                     user_id, bridge_type, trigger_kind, expected_bridge_id,
                     expected_bridge_created_at, management_room_id, status,
                     attempt_count, not_before, created_at, updated_at
                 ) VALUES ($1, $2, $3, $4, $5, $6, 'pending', 0, $7, $8, $8)
                 ON CONFLICT (user_id, bridge_type, expected_bridge_id)
                     WHERE expected_bridge_id IS NOT NULL
                 DO UPDATE SET
                     trigger_kind = EXCLUDED.trigger_kind,
                     management_room_id = COALESCE(EXCLUDED.management_room_id, bridge_cleanup_jobs.management_room_id),
                     not_before = LEAST(bridge_cleanup_jobs.not_before, EXCLUDED.not_before),
                     updated_at = EXCLUDED.updated_at
                 RETURNING id",
            )
            .bind::<diesel::sql_types::Integer, _>(bridge.user_id)
            .bind::<diesel::sql_types::Text, _>(&bridge.bridge_type)
            .bind::<diesel::sql_types::Text, _>(trigger_kind)
            .bind::<diesel::sql_types::Integer, _>(bridge.id)
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Integer>, _>(bridge.created_at)
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(bridge.room_id.as_deref())
            .bind::<diesel::sql_types::Integer, _>(now.saturating_add(grace_secs.max(0)))
            .bind::<diesel::sql_types::Integer, _>(now)
            .get_result::<ReturnedId>(conn)?;

            self.seed_bridge_cleanup_rooms(conn, job.id, bridge.user_id, &bridge.bridge_type, bridge.room_id.as_deref(), now)?;
            Ok(job.id)
        })
    }

    fn seed_bridge_cleanup_rooms(
        &self,
        conn: &mut diesel::PgConnection,
        job_id: i32,
        user_id: i32,
        bridge_type: &str,
        management_room_id: Option<&str>,
        now: i32,
    ) -> Result<()> {
        diesel::sql_query(
            "INSERT INTO bridge_cleanup_rooms (
                 job_id, room_id, source, status, attempt_count, discovered_at, updated_at
             )
             SELECT $1, rooms.room_id, rooms.source, 'pending', 0, $5, $5
               FROM (
                    SELECT DISTINCT room_id, 'ontology'::TEXT AS source
                      FROM ont_messages
                     WHERE user_id = $2 AND platform = $3
                    UNION
                    SELECT $4::TEXT, 'management'::TEXT
                     WHERE $4::TEXT IS NOT NULL AND $4::TEXT LIKE '!%'
               ) rooms
              WHERE rooms.room_id LIKE '!%'
             ON CONFLICT (job_id, room_id) DO NOTHING",
        )
        .bind::<diesel::sql_types::Integer, _>(job_id)
        .bind::<diesel::sql_types::Integer, _>(user_id)
        .bind::<diesel::sql_types::Text, _>(bridge_type)
        .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(management_room_id)
        .bind::<diesel::sql_types::Integer, _>(now)
        .execute(conn)?;
        Ok(())
    }

    pub fn enqueue_orphan_bridge_cleanup_audits(
        &self,
        limit: i64,
        grace_secs: i32,
    ) -> Result<usize> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        let rows = diesel::sql_query(
            "WITH candidates AS (
                 SELECT messages.user_id, messages.platform AS bridge_type
                   FROM ont_messages messages
                  WHERE messages.platform IN ('whatsapp', 'signal', 'telegram')
                    AND NOT EXISTS (
                        SELECT 1 FROM bridges current
                         WHERE current.user_id = messages.user_id
                           AND current.bridge_type = messages.platform
                    )
                    AND NOT EXISTS (
                        SELECT 1 FROM bridge_cleanup_jobs existing
                         WHERE existing.user_id = messages.user_id
                           AND existing.bridge_type = messages.platform
                           AND existing.expected_bridge_id IS NULL
                           AND existing.status IN ('audit_pending', 'audit_ready', 'retrying')
                    )
                  GROUP BY messages.user_id, messages.platform
                  ORDER BY min(messages.created_at)
                  LIMIT $1
             )
             INSERT INTO bridge_cleanup_jobs (
                 user_id, bridge_type, trigger_kind, status, attempt_count,
                 not_before, created_at, updated_at, portal_cleanup_status
             )
             SELECT user_id, bridge_type, 'orphan_audit', 'audit_pending', 0,
                    $2, $3, $3, 'legacy_unverified'
               FROM candidates
             ON CONFLICT DO NOTHING
             RETURNING id",
        )
        .bind::<diesel::sql_types::BigInt, _>(limit.max(1))
        .bind::<diesel::sql_types::Integer, _>(now.saturating_add(grace_secs.max(0)))
        .bind::<diesel::sql_types::Integer, _>(now)
        .load::<ReturnedId>(&mut conn)?;

        for row in &rows {
            let metadata = diesel::sql_query(
                "SELECT id, user_id, bridge_type, trigger_kind, expected_bridge_id,
                        expected_bridge_created_at, management_room_id, status,
                        attempt_count, portal_cleanup_status
                   FROM bridge_cleanup_jobs WHERE id = $1",
            )
            .bind::<diesel::sql_types::Integer, _>(row.id)
            .get_result::<BridgeCleanupJob>(&mut conn)?;
            self.seed_bridge_cleanup_rooms(
                &mut conn,
                row.id,
                metadata.user_id,
                &metadata.bridge_type,
                None,
                now,
            )?;
        }
        Ok(rows.len())
    }

    pub fn list_due_bridge_cleanup_jobs(
        &self,
        now: i32,
        limit: i64,
    ) -> Result<Vec<BridgeCleanupJob>> {
        let mut conn = self.connection()?;
        Ok(diesel::sql_query(
            "SELECT id, user_id, bridge_type, trigger_kind, expected_bridge_id,
                    expected_bridge_created_at, management_room_id, status, attempt_count,
                    portal_cleanup_status
               FROM bridge_cleanup_jobs
              WHERE status IN ('pending', 'audit_pending', 'audit_ready', 'retrying')
                AND not_before <= $1
              ORDER BY updated_at, id
              LIMIT $2",
        )
        .bind::<diesel::sql_types::Integer, _>(now)
        .bind::<diesel::sql_types::BigInt, _>(limit.max(1))
        .load(&mut conn)?)
    }

    pub fn list_bridge_cleanup_rooms(&self, job_id: i32) -> Result<Vec<BridgeCleanupRoom>> {
        let mut conn = self.connection()?;
        Ok(diesel::sql_query(
            "SELECT id, room_id, source, status, attempt_count
               FROM bridge_cleanup_rooms
              WHERE job_id = $1 AND status NOT IN ('succeeded')
              ORDER BY id",
        )
        .bind::<diesel::sql_types::Integer, _>(job_id)
        .load(&mut conn)?)
    }

    pub fn add_bridge_cleanup_rooms(
        &self,
        job_id: i32,
        room_ids: &[String],
        source: &str,
    ) -> Result<usize> {
        if room_ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.connection()?;
        let now = now_timestamp();
        Ok(diesel::sql_query(
            "INSERT INTO bridge_cleanup_rooms (
                 job_id, room_id, source, status, attempt_count, discovered_at, updated_at
             )
             SELECT $1, rooms.room_id, $3, 'pending', 0, $4, $4
               FROM unnest($2::TEXT[]) AS rooms(room_id)
              WHERE rooms.room_id LIKE '!%'
             ON CONFLICT (job_id, room_id) DO NOTHING",
        )
        .bind::<diesel::sql_types::Integer, _>(job_id)
        .bind::<diesel::sql_types::Array<diesel::sql_types::Text>, _>(room_ids)
        .bind::<diesel::sql_types::Text, _>(source)
        .bind::<diesel::sql_types::Integer, _>(now)
        .execute(&mut conn)?)
    }

    pub fn bridge_generation_present(
        &self,
        job: &BridgeCleanupJob,
    ) -> Result<Option<(i32, String, Option<i32>)>> {
        let mut conn = self.connection()?;
        #[derive(QueryableByName)]
        struct CurrentBridge {
            #[diesel(sql_type = diesel::sql_types::Integer)]
            id: i32,
            #[diesel(sql_type = diesel::sql_types::Text)]
            status: String,
            #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Integer>)]
            created_at: Option<i32>,
        }
        let current = diesel::sql_query(
            "SELECT id, status, created_at FROM bridges
              WHERE user_id = $1 AND bridge_type = $2 ORDER BY id DESC LIMIT 1",
        )
        .bind::<diesel::sql_types::Integer, _>(job.user_id)
        .bind::<diesel::sql_types::Text, _>(&job.bridge_type)
        .get_result::<CurrentBridge>(&mut conn)
        .optional()?;
        Ok(current.map(|row| (row.id, row.status, row.created_at)))
    }

    pub fn try_acquire_bridge_connection_lease(
        &self,
        user_id: i32,
        bridge_type: &str,
        lease_kind: &str,
        owner_token: &str,
        lease_until: i32,
    ) -> Result<bool> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        let acquired = diesel::sql_query(
            "INSERT INTO bridge_connection_leases (
                 user_id, bridge_type, lease_kind, owner_token, lease_until, updated_at
             ) VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (user_id, bridge_type) DO UPDATE SET
                 lease_kind = EXCLUDED.lease_kind,
                 owner_token = EXCLUDED.owner_token,
                 lease_until = EXCLUDED.lease_until,
                 updated_at = EXCLUDED.updated_at
             WHERE bridge_connection_leases.lease_until <= $6
                OR bridge_connection_leases.owner_token = EXCLUDED.owner_token
             RETURNING owner_token",
        )
        .bind::<diesel::sql_types::Integer, _>(user_id)
        .bind::<diesel::sql_types::Text, _>(bridge_type)
        .bind::<diesel::sql_types::Text, _>(lease_kind)
        .bind::<diesel::sql_types::Text, _>(owner_token)
        .bind::<diesel::sql_types::Integer, _>(lease_until)
        .bind::<diesel::sql_types::Integer, _>(now)
        .get_result::<ReturnedLease>(&mut conn)
        .optional()?;
        Ok(acquired
            .map(|lease| lease.owner_token == owner_token)
            .unwrap_or(false))
    }

    pub fn release_bridge_connection_lease(
        &self,
        user_id: i32,
        bridge_type: &str,
        owner_token: &str,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        diesel::sql_query(
            "DELETE FROM bridge_connection_leases
              WHERE user_id = $1 AND bridge_type = $2 AND owner_token = $3",
        )
        .bind::<diesel::sql_types::Integer, _>(user_id)
        .bind::<diesel::sql_types::Text, _>(bridge_type)
        .bind::<diesel::sql_types::Text, _>(owner_token)
        .execute(&mut conn)?;
        Ok(())
    }

    pub fn mark_bridge_portal_cleanup(
        &self,
        job_id: i32,
        confirmed: bool,
        error: Option<&str>,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        let status = if confirmed { "confirmed" } else { "failed" };
        diesel::sql_query(
            "UPDATE bridge_cleanup_jobs
                SET portal_cleanup_status = $2,
                    portal_cleanup_confirmed_at = CASE WHEN $3 THEN $4 ELSE NULL END,
                    portal_cleanup_error = $5,
                    not_before = $4,
                    updated_at = $4
              WHERE id = $1",
        )
        .bind::<diesel::sql_types::Integer, _>(job_id)
        .bind::<diesel::sql_types::Text, _>(status)
        .bind::<diesel::sql_types::Bool, _>(confirmed)
        .bind::<diesel::sql_types::Integer, _>(now)
        .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(error.map(trim_error))
        .execute(&mut conn)?;
        Ok(())
    }

    pub fn record_bridge_cleanup_storage(
        &self,
        job_id: i32,
        before: bool,
        rootfs_free_bytes: Option<i64>,
        tuwunel_bytes: Option<i64>,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        diesel::sql_query(
            "UPDATE bridge_cleanup_jobs SET
                 rootfs_free_before_bytes = CASE WHEN $2 THEN COALESCE(rootfs_free_before_bytes, $3) ELSE rootfs_free_before_bytes END,
                 tuwunel_before_bytes = CASE WHEN $2 THEN COALESCE(tuwunel_before_bytes, $4) ELSE tuwunel_before_bytes END,
                 rootfs_free_after_bytes = CASE WHEN $2 THEN rootfs_free_after_bytes ELSE COALESCE($3, rootfs_free_after_bytes) END,
                 tuwunel_after_bytes = CASE WHEN $2 THEN tuwunel_after_bytes ELSE COALESCE($4, tuwunel_after_bytes) END,
                 updated_at = $5
              WHERE id = $1",
        )
        .bind::<diesel::sql_types::Integer, _>(job_id)
        .bind::<diesel::sql_types::Bool, _>(before)
        .bind::<diesel::sql_types::Nullable<diesel::sql_types::BigInt>, _>(rootfs_free_bytes)
        .bind::<diesel::sql_types::Nullable<diesel::sql_types::BigInt>, _>(tuwunel_bytes)
        .bind::<diesel::sql_types::Integer, _>(now)
        .execute(&mut conn)?;
        Ok(())
    }

    pub fn active_ontology_room_owners(
        &self,
        room_id: &str,
        bridge_type: &str,
        excluding_user_id: i32,
    ) -> Result<i64> {
        let mut conn = self.connection()?;
        Ok(diesel::sql_query(
            "SELECT count(DISTINCT messages.user_id) AS count
               FROM ont_messages messages
               JOIN bridges active
                 ON active.user_id = messages.user_id
                AND active.bridge_type = messages.platform
                AND active.status = 'connected'
              WHERE messages.room_id = $1
                AND messages.platform = $2
                AND messages.user_id <> $3",
        )
        .bind::<diesel::sql_types::Text, _>(room_id)
        .bind::<diesel::sql_types::Text, _>(bridge_type)
        .bind::<diesel::sql_types::Integer, _>(excluding_user_id)
        .get_result::<CountRow>(&mut conn)?
        .count)
    }

    pub fn active_bridge_members(
        &self,
        matrix_localparts: &[String],
        bridge_type: &str,
        excluding_user_id: i32,
    ) -> Result<i64> {
        if matrix_localparts.is_empty() {
            return Ok(0);
        }
        let mut conn = self.connection()?;
        Ok(diesel::sql_query(
            "SELECT count(DISTINCT active.user_id) AS count
               FROM bridges active
               JOIN user_secrets secrets ON secrets.user_id = active.user_id
              WHERE active.bridge_type = $1
                AND active.status = 'connected'
                AND active.user_id <> $2
                AND secrets.matrix_username = ANY($3)",
        )
        .bind::<diesel::sql_types::Text, _>(bridge_type)
        .bind::<diesel::sql_types::Integer, _>(excluding_user_id)
        .bind::<diesel::sql_types::Array<diesel::sql_types::Text>, _>(matrix_localparts)
        .get_result::<CountRow>(&mut conn)?
        .count)
    }

    pub fn update_bridge_cleanup_job(
        &self,
        id: i32,
        status: &str,
        attempt_count: i32,
        not_before: i32,
        error: Option<&str>,
        complete: bool,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        diesel::sql_query(
            "UPDATE bridge_cleanup_jobs SET status = $2, attempt_count = $3,
                    not_before = $4, last_error = $5, updated_at = $6,
                    completed_at = CASE WHEN $7 THEN $6 ELSE NULL END
              WHERE id = $1",
        )
        .bind::<diesel::sql_types::Integer, _>(id)
        .bind::<diesel::sql_types::Text, _>(status)
        .bind::<diesel::sql_types::Integer, _>(attempt_count)
        .bind::<diesel::sql_types::Integer, _>(not_before)
        .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(error.map(trim_error))
        .bind::<diesel::sql_types::Integer, _>(now)
        .bind::<diesel::sql_types::Bool, _>(complete)
        .execute(&mut conn)?;
        Ok(())
    }

    pub fn update_bridge_cleanup_room(
        &self,
        id: i32,
        status: &str,
        attempt_count: i32,
        delete_id: Option<&str>,
        error: Option<&str>,
        complete: bool,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        diesel::sql_query(
            "UPDATE bridge_cleanup_rooms SET status = $2, attempt_count = $3,
                    delete_id = COALESCE($4, delete_id), last_error = $5,
                    updated_at = $6,
                    completed_at = CASE WHEN $7 THEN $6 ELSE NULL END
              WHERE id = $1",
        )
        .bind::<diesel::sql_types::Integer, _>(id)
        .bind::<diesel::sql_types::Text, _>(status)
        .bind::<diesel::sql_types::Integer, _>(attempt_count)
        .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(delete_id)
        .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(error.map(trim_error))
        .bind::<diesel::sql_types::Integer, _>(now)
        .bind::<diesel::sql_types::Bool, _>(complete)
        .execute(&mut conn)?;
        Ok(())
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
        self.enqueue_audited_historical_backfill(
            candidate,
            STATUS_BACKFILL_AUDIT_VERIFIED,
            "historical_backfill_verified",
            audit_summary,
        )
    }

    pub fn enqueue_forced_historical_backfill(
        &self,
        candidate: &HistoricalBackfillCandidate,
        audit_summary: &str,
    ) -> Result<()> {
        self.enqueue_audited_historical_backfill(
            candidate,
            STATUS_BACKFILL_AUDIT_BLOCKED,
            "historical_backfill_forced_unverified",
            audit_summary,
        )
    }

    fn enqueue_audited_historical_backfill(
        &self,
        candidate: &HistoricalBackfillCandidate,
        required_status: &str,
        command_kind: &str,
        audit_summary: &str,
    ) -> Result<()> {
        let mut conn = self.connection()?;
        let now = now_timestamp();
        let updated = diesel::update(
            tuwunel_cleanup_events::table
                .filter(tuwunel_cleanup_events::event_id.eq(&candidate.event_id))
                .filter(tuwunel_cleanup_events::status.eq(required_status)),
        )
        .set((
            tuwunel_cleanup_events::ontology_message_id.eq(candidate.ontology_message_id),
            tuwunel_cleanup_events::delete_media.eq(true),
            tuwunel_cleanup_events::commands_expected.eq(1),
            tuwunel_cleanup_events::commands_accepted.eq(0),
            tuwunel_cleanup_events::attempt_count.eq(0),
            tuwunel_cleanup_events::status.eq(STATUS_PENDING_PURGE),
            tuwunel_cleanup_events::last_command_kind.eq(Some(command_kind)),
            tuwunel_cleanup_events::last_error.eq(Some(trim_error(audit_summary))),
            tuwunel_cleanup_events::enqueued_at.eq(now),
            tuwunel_cleanup_events::last_attempted_at.eq(None::<i32>),
            tuwunel_cleanup_events::completed_at.eq(None::<i32>),
            tuwunel_cleanup_events::updated_at.eq(now),
        ))
        .execute(&mut conn)?;
        if updated != 1 {
            return Err(anyhow!(
                "historical boundary {} was not in required audit state {}",
                candidate.event_id,
                required_status
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
