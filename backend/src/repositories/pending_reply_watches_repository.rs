//! One-shot reply watches. The AI tools `send_chat_message`,
//! `send_email`, and `respond_to_email` arm a row here when the user
//! asked to be told about the reply. The inbound message handlers
//! (`handle_bridge_message`, `insert_email_into_ontology`) look up
//! matching rows and notify+delete on first match.

use crate::{
    models::user_models::{NewPendingReplyWatch, PendingReplyWatch},
    pg_schema::pending_reply_watches,
    PgDbPool,
};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use std::time::{SystemTime, UNIX_EPOCH};

pub const PLATFORM_BRIDGE: &str = "bridge";
pub const PLATFORM_EMAIL: &str = "email";

/// 24 hours.
pub const DEFAULT_TTL_SECONDS: i64 = 24 * 60 * 60;

pub struct PendingReplyWatchesRepository {
    pool: PgDbPool,
}

impl PendingReplyWatchesRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, watch: NewPendingReplyWatch) -> Result<PendingReplyWatch, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(pending_reply_watches::table)
            .values(&watch)
            .get_result::<PendingReplyWatch>(&mut conn)
    }

    /// Arm a bridge watch. `display_name` is what we'll use to label
    /// the inbound notification SMS ("Reply from John: ...").
    pub fn arm_bridge(
        &self,
        user_id: i32,
        room_id: &str,
        contact_identifier: &str,
        display_name: &str,
    ) -> Result<PendingReplyWatch, DieselError> {
        let now = now_epoch();
        self.create(NewPendingReplyWatch {
            user_id,
            platform: PLATFORM_BRIDGE.to_string(),
            room_id: Some(room_id.to_string()),
            imap_connection_id: None,
            contact_identifier: contact_identifier.to_string(),
            contact_display_name: display_name.to_string(),
            created_at: now,
            expires_at: now + DEFAULT_TTL_SECONDS as i32,
        })
    }

    /// Arm an email watch. `contact_identifier` should be the normalized
    /// recipient email (matches the `sender_key` produced by IMAP ingest).
    pub fn arm_email(
        &self,
        user_id: i32,
        imap_connection_id: i32,
        contact_identifier: &str,
        display_name: &str,
    ) -> Result<PendingReplyWatch, DieselError> {
        let now = now_epoch();
        self.create(NewPendingReplyWatch {
            user_id,
            platform: PLATFORM_EMAIL.to_string(),
            room_id: None,
            imap_connection_id: Some(imap_connection_id),
            contact_identifier: contact_identifier.to_string(),
            contact_display_name: display_name.to_string(),
            created_at: now,
            expires_at: now + DEFAULT_TTL_SECONDS as i32,
        })
    }

    /// Find an active (non-expired) bridge watch for this (user, room).
    pub fn find_active_bridge(
        &self,
        user_id: i32,
        room_id: &str,
    ) -> Result<Option<PendingReplyWatch>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = now_epoch();
        pending_reply_watches::table
            .filter(pending_reply_watches::platform.eq(PLATFORM_BRIDGE))
            .filter(pending_reply_watches::user_id.eq(user_id))
            .filter(pending_reply_watches::room_id.eq(room_id))
            .filter(pending_reply_watches::expires_at.gt(now))
            .select(PendingReplyWatch::as_select())
            .first::<PendingReplyWatch>(&mut conn)
            .optional()
    }

    /// Find an active email watch for (user, account, sender). `sender_key`
    /// is the already-normalized sender email produced by IMAP ingest.
    pub fn find_active_email(
        &self,
        user_id: i32,
        imap_connection_id: i32,
        sender_key: &str,
    ) -> Result<Option<PendingReplyWatch>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = now_epoch();
        pending_reply_watches::table
            .filter(pending_reply_watches::platform.eq(PLATFORM_EMAIL))
            .filter(pending_reply_watches::user_id.eq(user_id))
            .filter(pending_reply_watches::imap_connection_id.eq(imap_connection_id))
            .filter(pending_reply_watches::contact_identifier.eq(sender_key))
            .filter(pending_reply_watches::expires_at.gt(now))
            .select(PendingReplyWatch::as_select())
            .first::<PendingReplyWatch>(&mut conn)
            .optional()
    }

    pub fn delete(&self, id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(pending_reply_watches::table.filter(pending_reply_watches::id.eq(id)))
            .execute(&mut conn)?;
        Ok(())
    }

    /// Best-effort cleanup of expired rows. Safe to call on any cadence;
    /// queries already filter by `expires_at > now`.
    pub fn delete_expired(&self) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = now_epoch();
        diesel::delete(
            pending_reply_watches::table.filter(pending_reply_watches::expires_at.le(now)),
        )
        .execute(&mut conn)
    }
}

fn now_epoch() -> i32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32
}
