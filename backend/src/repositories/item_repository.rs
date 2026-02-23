use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::{
    models::user_models::{Item, NewItem},
    schema::items,
    DbPool,
};

/// Maximum active items per user (safety limit)
const MAX_ITEMS_PER_USER: i64 = 100;

pub struct ItemRepository {
    pub pool: DbPool,
}

impl ItemRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Insert a new item. Returns the new item's id.
    pub fn create_item(&self, new_item: &NewItem) -> Result<i32, DieselError> {
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

        diesel::insert_into(items::table)
            .values(new_item)
            .execute(&mut conn)?;

        let item_id: Option<i32> = items::table
            .filter(items::user_id.eq(new_item.user_id))
            .order(items::id.desc())
            .select(items::id)
            .first(&mut conn)?;

        Ok(item_id.unwrap_or(0))
    }

    /// Get all items for a user, ordered by priority desc, created_at desc.
    pub fn get_items(&self, user_id: i32) -> Result<Vec<Item>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        items::table
            .filter(items::user_id.eq(user_id))
            .order(items::priority.desc())
            .then_order_by(items::created_at.desc())
            .load::<Item>(&mut conn)
    }

    /// Get a single item with ownership check.
    pub fn get_item(&self, id: i32, user_id: i32) -> Result<Option<Item>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        items::table
            .filter(items::id.eq(id))
            .filter(items::user_id.eq(user_id))
            .first::<Item>(&mut conn)
            .optional()
    }

    /// Get items where next_check_at <= now (scheduler hot path, no LLM).
    pub fn get_triggered_items(&self, now: i32) -> Result<Vec<Item>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        items::table
            .filter(items::next_check_at.is_not_null())
            .filter(items::next_check_at.le(now))
            .load::<Item>(&mut conn)
    }

    /// Get monitor items (monitor = true) for email/message matching.
    pub fn get_monitor_items(&self, user_id: i32) -> Result<Vec<Item>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        items::table
            .filter(items::user_id.eq(user_id))
            .filter(items::monitor.eq(true))
            .load::<Item>(&mut conn)
    }

    /// Get all items for dashboard display, ordered by priority desc, created_at desc.
    pub fn get_dashboard_items(&self, user_id: i32) -> Result<Vec<Item>, DieselError> {
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

    /// Update next_check_at (snooze or reschedule).
    pub fn update_next_check_at(
        &self,
        id: i32,
        next_check_at: Option<i32>,
    ) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = diesel::update(items::table.filter(items::id.eq(id)))
            .set(items::next_check_at.eq(next_check_at))
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

    /// Bulk update after LLM processing: summary, next_check_at, priority.
    pub fn update_item(
        &self,
        id: i32,
        user_id: i32,
        summary: &str,
        next_check_at: Option<i32>,
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
            items::next_check_at.eq(next_check_at),
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

    /// Cleanup: delete items older than a given timestamp.
    pub fn delete_old_items(&self, before_ts: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let count = diesel::delete(items::table.filter(items::created_at.lt(before_ts)))
            .execute(&mut conn)?;
        Ok(count)
    }

    /// Auto-expire stale monitors: monitors with next_check_at >7 days in the past.
    /// They are stale and should be cleaned up.
    pub fn delete_stale_monitors(&self, now: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let cutoff = now - (7 * 86400); // 7 days ago
        let count = diesel::delete(
            items::table
                .filter(items::monitor.eq(true))
                .filter(items::next_check_at.is_not_null())
                .filter(items::next_check_at.lt(cutoff)),
        )
        .execute(&mut conn)?;
        Ok(count)
    }
}
