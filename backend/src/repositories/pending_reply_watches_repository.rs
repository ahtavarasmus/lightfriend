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
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        conn.transaction(|conn| {
            // Re-arming the same conversation refreshes its one-shot watch instead
            // of stacking several notifications for future messages.
            diesel::delete(
                pending_reply_watches::table
                    .filter(pending_reply_watches::platform.eq(PLATFORM_BRIDGE))
                    .filter(pending_reply_watches::user_id.eq(user_id))
                    .filter(pending_reply_watches::room_id.eq(room_id)),
            )
            .execute(conn)?;
            diesel::insert_into(pending_reply_watches::table)
                .values(NewPendingReplyWatch {
                    user_id,
                    platform: PLATFORM_BRIDGE.to_string(),
                    room_id: Some(room_id.to_string()),
                    imap_connection_id: None,
                    contact_identifier: contact_identifier.to_string(),
                    contact_display_name: display_name.to_string(),
                    created_at: now,
                    expires_at: now + DEFAULT_TTL_SECONDS as i32,
                })
                .get_result(conn)
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
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        conn.transaction(|conn| {
            diesel::delete(
                pending_reply_watches::table
                    .filter(pending_reply_watches::platform.eq(PLATFORM_EMAIL))
                    .filter(pending_reply_watches::user_id.eq(user_id))
                    .filter(pending_reply_watches::imap_connection_id.eq(imap_connection_id))
                    .filter(pending_reply_watches::contact_identifier.eq(contact_identifier)),
            )
            .execute(conn)?;
            diesel::insert_into(pending_reply_watches::table)
                .values(NewPendingReplyWatch {
                    user_id,
                    platform: PLATFORM_EMAIL.to_string(),
                    room_id: None,
                    imap_connection_id: Some(imap_connection_id),
                    contact_identifier: contact_identifier.to_string(),
                    contact_display_name: display_name.to_string(),
                    created_at: now,
                    expires_at: now + DEFAULT_TTL_SECONDS as i32,
                })
                .get_result(conn)
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

    /// Atomically claim and remove the active watch for a bridge room.
    ///
    /// Deleting before notification prevents two concurrently arriving Matrix
    /// events from both firing the same one-shot watch. The caller can restore
    /// the returned row if delivery fails.
    pub fn claim_active_bridge(
        &self,
        user_id: i32,
        room_id: &str,
    ) -> Result<Option<PendingReplyWatch>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = now_epoch();
        let mut claimed = diesel::delete(
            pending_reply_watches::table
                .filter(pending_reply_watches::platform.eq(PLATFORM_BRIDGE))
                .filter(pending_reply_watches::user_id.eq(user_id))
                .filter(pending_reply_watches::room_id.eq(room_id))
                .filter(pending_reply_watches::expires_at.gt(now)),
        )
        .get_results::<PendingReplyWatch>(&mut conn)?;
        Ok(claimed.pop())
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

    /// Atomically claim and remove the active email watch for this sender.
    pub fn claim_active_email(
        &self,
        user_id: i32,
        imap_connection_id: i32,
        sender_key: &str,
    ) -> Result<Option<PendingReplyWatch>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = now_epoch();
        let mut claimed = diesel::delete(
            pending_reply_watches::table
                .filter(pending_reply_watches::platform.eq(PLATFORM_EMAIL))
                .filter(pending_reply_watches::user_id.eq(user_id))
                .filter(pending_reply_watches::imap_connection_id.eq(imap_connection_id))
                .filter(pending_reply_watches::contact_identifier.eq(sender_key))
                .filter(pending_reply_watches::expires_at.gt(now)),
        )
        .get_results::<PendingReplyWatch>(&mut conn)?;
        Ok(claimed.pop())
    }

    /// Put a claimed watch back after a transient notification failure while
    /// preserving its original expiry.
    pub fn restore(&self, watch: &PendingReplyWatch) -> Result<PendingReplyWatch, DieselError> {
        self.create(NewPendingReplyWatch {
            user_id: watch.user_id,
            platform: watch.platform.clone(),
            room_id: watch.room_id.clone(),
            imap_connection_id: watch.imap_connection_id,
            contact_identifier: watch.contact_identifier.clone(),
            contact_display_name: watch.contact_display_name.clone(),
            created_at: watch.created_at,
            expires_at: watch.expires_at,
        })
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
