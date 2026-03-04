use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::{
    pg_models::{NewPgItem, PgItem},
    pg_schema::items,
    PgDbPool,
};

/// Maximum active items per user (safety limit)
const MAX_ITEMS_PER_USER: i64 = 100;

pub struct ItemRepository {
    pub pool: PgDbPool,
}

impl ItemRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    /// Insert a new item. Returns the new item's id.
    pub fn create_item(&self, new_item: &NewPgItem) -> Result<i32, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Check item limit
        let count: i64 = items::table
            .filter(items::user_id.eq(new_item.user_id))
            .count()
            .get_result(&mut conn)?;

        if count >= MAX_ITEMS_PER_USER {
            return Err(DieselError::DatabaseError(
                diesel::result::DatabaseErrorKind::CheckViolation,
                Box::new(format!(
                    "Item limit reached: maximum {} items per user",
                    MAX_ITEMS_PER_USER
                )),
            ));
        }

        let item: PgItem = diesel::insert_into(items::table)
            .values(new_item)
            .get_result(&mut conn)?;

        Ok(item.id)
    }

    /// Get all items for a user, ordered by priority desc, created_at desc.
    pub fn get_items(&self, user_id: i32) -> Result<Vec<PgItem>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        items::table
            .filter(items::user_id.eq(user_id))
            .order(items::priority.desc())
            .then_order_by(items::created_at.desc())
            .load::<PgItem>(&mut conn)
    }

    /// Get a single item with ownership check.
    pub fn get_item(&self, id: i32, user_id: i32) -> Result<Option<PgItem>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        items::table
            .filter(items::id.eq(id))
            .filter(items::user_id.eq(user_id))
            .first::<PgItem>(&mut conn)
            .optional()
    }

    /// Get items where due_at <= now (scheduler hot path, no LLM).
    pub fn get_triggered_items(&self, now: i32) -> Result<Vec<PgItem>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        items::table
            .filter(items::due_at.is_not_null())
            .filter(items::due_at.le(now))
            .load::<PgItem>(&mut conn)
    }

    /// Get tracking items for a user (summary contains [type:tracking]).
    pub fn get_tracking_items(&self, user_id: i32) -> Result<Vec<PgItem>, DieselError> {
        let all = self.get_items(user_id)?;
        Ok(all
            .into_iter()
            .filter(|item| item.summary.contains("[type:tracking]"))
            .collect())
    }

    /// Get all items for dashboard display, ordered by priority desc, created_at desc.
    pub fn get_dashboard_items(&self, user_id: i32) -> Result<Vec<PgItem>, DieselError> {
        self.get_items(user_id)
    }

    /// Dedup check: does an item with this source_id already exist for this user?
    pub fn item_exists_by_source(
        &self,
        user_id: i32,
        source_id_val: &str,
    ) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count: i64 = items::table
            .filter(items::user_id.eq(user_id))
            .filter(items::source_id.eq(source_id_val))
            .count()
            .get_result(&mut conn)?;
        Ok(count > 0)
    }

    /// Atomic dedup create: insert only if no item with the same source_id exists for this user.
    /// Returns Ok(Some(id)) if inserted, Ok(None) if already exists.
    pub fn create_item_if_not_exists(
        &self,
        new_item: &NewPgItem,
    ) -> Result<Option<i32>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        conn.transaction(|conn| {
            // Check dedup inside transaction
            if let Some(ref sid) = new_item.source_id {
                let count: i64 = items::table
                    .filter(items::user_id.eq(new_item.user_id))
                    .filter(items::source_id.eq(sid))
                    .count()
                    .get_result(conn)?;
                if count > 0 {
                    return Ok(None);
                }
            }

            // Check item limit
            let count: i64 = items::table
                .filter(items::user_id.eq(new_item.user_id))
                .count()
                .get_result(conn)?;

            if count >= MAX_ITEMS_PER_USER {
                return Err(DieselError::DatabaseError(
                    diesel::result::DatabaseErrorKind::CheckViolation,
                    Box::new(format!(
                        "Item limit reached: maximum {} items per user",
                        MAX_ITEMS_PER_USER
                    )),
                ));
            }

            let item: PgItem = diesel::insert_into(items::table)
                .values(new_item)
                .get_result(conn)?;

            Ok(Some(item.id))
        })
    }

    /// Update due_at (snooze or reschedule).
    pub fn update_due_at(&self, id: i32, due_at: Option<i32>) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = diesel::update(items::table.filter(items::id.eq(id)))
            .set(items::due_at.eq(due_at))
            .execute(&mut conn)?;
        Ok(count > 0)
    }

    /// Update priority (escalation).
    pub fn update_priority(&self, id: i32, priority: i32) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = diesel::update(items::table.filter(items::id.eq(id)))
            .set(items::priority.eq(priority))
            .execute(&mut conn)?;
        Ok(count > 0)
    }

    /// Update summary (context updates from LLM or condition match).
    pub fn update_summary(&self, id: i32, summary: &str) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = diesel::update(items::table.filter(items::id.eq(id)))
            .set(items::summary.eq(summary))
            .execute(&mut conn)?;
        Ok(count > 0)
    }

    /// Bulk update after LLM processing: summary, due_at, priority.
    pub fn update_item(
        &self,
        id: i32,
        user_id: i32,
        summary: &str,
        due_at: Option<i32>,
        priority: i32,
    ) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = diesel::update(
            items::table
                .filter(items::id.eq(id))
                .filter(items::user_id.eq(user_id)),
        )
        .set((
            items::summary.eq(summary),
            items::due_at.eq(due_at),
            items::priority.eq(priority),
        ))
        .execute(&mut conn)?;
        Ok(count > 0)
    }

    /// Delete an item (complete/dismiss).
    pub fn delete_item(&self, id: i32, user_id: i32) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = diesel::delete(
            items::table
                .filter(items::id.eq(id))
                .filter(items::user_id.eq(user_id)),
        )
        .execute(&mut conn)?;
        Ok(count > 0)
    }

    /// Delete items with a matching source_id for a user (e.g., room dismissal).
    pub fn delete_items_by_source(
        &self,
        user_id: i32,
        source_id_val: &str,
    ) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = diesel::delete(
            items::table
                .filter(items::user_id.eq(user_id))
                .filter(items::source_id.eq(source_id_val)),
        )
        .execute(&mut conn)?;
        Ok(count)
    }

    /// Delete items whose source_id starts with a given prefix (e.g. "msg_whatsapp_!room:").
    pub fn delete_items_by_source_prefix(
        &self,
        user_id: i32,
        source_id_prefix: &str,
    ) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let pattern = format!("{}%", source_id_prefix);
        let count = diesel::delete(
            items::table
                .filter(items::user_id.eq(user_id))
                .filter(items::source_id.like(pattern)),
        )
        .execute(&mut conn)?;
        Ok(count)
    }

    /// Get items whose source_id starts with a given prefix (e.g. "email_").
    pub fn get_items_by_source_prefix(
        &self,
        user_id: i32,
        source_id_prefix: &str,
    ) -> Result<Vec<PgItem>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let pattern = format!("{}%", source_id_prefix);
        items::table
            .filter(items::user_id.eq(user_id))
            .filter(items::source_id.like(pattern))
            .load::<PgItem>(&mut conn)
    }

    /// Cleanup: delete items older than a given timestamp.
    pub fn delete_old_items(&self, before_ts: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = diesel::delete(items::table.filter(items::created_at.lt(before_ts)))
            .execute(&mut conn)?;
        Ok(count)
    }

    /// Auto-expire tracking items with due_at >7 days in the past.
    pub fn delete_expired_tracking_items(&self, now: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let cutoff = now - (7 * 86400); // 7 days ago
        let count = diesel::delete(
            items::table
                .filter(items::summary.like("%[type:tracking]%"))
                .filter(items::due_at.is_not_null())
                .filter(items::due_at.lt(cutoff)),
        )
        .execute(&mut conn)?;
        Ok(count)
    }
}
