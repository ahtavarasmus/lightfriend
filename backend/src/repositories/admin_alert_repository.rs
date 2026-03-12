use crate::{
    models::user_models::{AdminAlert, DisabledAlertType, NewAdminAlert, NewDisabledAlertType},
    pg_schema::{admin_alerts, disabled_alert_types},
    PgDbPool,
};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct AdminAlertRepository {
    pool: PgDbPool,
}

impl AdminAlertRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    /// Create a new alert in the database
    pub fn create_alert(
        &self,
        alert_type: &str,
        severity: &str,
        message: &str,
        location: &str,
        module: &str,
    ) -> Result<i32, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_alert = NewAdminAlert {
            alert_type: alert_type.to_string(),
            severity: severity.to_string(),
            message: message.to_string(),
            location: location.to_string(),
            module: module.to_string(),
            acknowledged: 0,
            created_at: current_time,
        };

        let id: i32 = diesel::insert_into(admin_alerts::table)
            .values(&new_alert)
            .returning(admin_alerts::id)
            .get_result(&mut conn)?;

        Ok(id)
    }

    /// Get alerts with pagination and optional severity filter
    pub fn get_alerts(
        &self,
        limit: i64,
        offset: i64,
        severity_filter: Option<&str>,
    ) -> Result<Vec<AdminAlert>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let mut query = admin_alerts::table
            .order(admin_alerts::created_at.desc())
            .into_boxed();

        if let Some(severity) = severity_filter {
            query = query.filter(admin_alerts::severity.eq(severity));
        }

        query
            .limit(limit)
            .offset(offset)
            .select(AdminAlert::as_select())
            .load::<AdminAlert>(&mut conn)
    }

    /// Get count of unacknowledged alerts
    pub fn get_unacknowledged_count(&self) -> Result<i64, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        admin_alerts::table
            .filter(admin_alerts::acknowledged.eq(0))
            .count()
            .get_result(&mut conn)
    }

    /// Acknowledge a single alert
    pub fn acknowledge_alert(&self, alert_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::update(admin_alerts::table.filter(admin_alerts::id.eq(alert_id)))
            .set(admin_alerts::acknowledged.eq(1))
            .execute(&mut conn)?;

        Ok(())
    }

    /// Acknowledge all alerts
    pub fn acknowledge_all(&self) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let updated = diesel::update(admin_alerts::table.filter(admin_alerts::acknowledged.eq(0)))
            .set(admin_alerts::acknowledged.eq(1))
            .execute(&mut conn)?;

        Ok(updated)
    }

    /// Check if an alert type is disabled
    pub fn is_alert_type_disabled(&self, alert_type: &str) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let count: i64 = disabled_alert_types::table
            .filter(disabled_alert_types::alert_type.eq(alert_type))
            .count()
            .get_result(&mut conn)?;

        Ok(count > 0)
    }

    /// Disable an alert type
    pub fn disable_alert_type(&self, alert_type: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_disabled = NewDisabledAlertType {
            alert_type: alert_type.to_string(),
            disabled_at: current_time,
        };

        // Use ON CONFLICT DO NOTHING to handle duplicates gracefully
        diesel::insert_into(disabled_alert_types::table)
            .values(&new_disabled)
            .on_conflict(disabled_alert_types::alert_type)
            .do_nothing()
            .execute(&mut conn)?;

        Ok(())
    }

    /// Enable an alert type (remove from disabled list)
    pub fn enable_alert_type(&self, alert_type: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        diesel::delete(
            disabled_alert_types::table.filter(disabled_alert_types::alert_type.eq(alert_type)),
        )
        .execute(&mut conn)?;

        Ok(())
    }

    /// Get all disabled alert types
    pub fn get_disabled_types(&self) -> Result<Vec<DisabledAlertType>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        disabled_alert_types::table
            .order(disabled_alert_types::disabled_at.desc())
            .select(DisabledAlertType::as_select())
            .load::<DisabledAlertType>(&mut conn)
    }

    /// Delete alerts older than a given timestamp
    pub fn delete_old_alerts(&self, before_timestamp: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let deleted = diesel::delete(
            admin_alerts::table.filter(admin_alerts::created_at.lt(before_timestamp)),
        )
        .execute(&mut conn)?;

        Ok(deleted)
    }

    /// Get total count of alerts (optionally filtered by severity)
    pub fn get_total_count(&self, severity_filter: Option<&str>) -> Result<i64, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let mut query = admin_alerts::table.into_boxed();

        if let Some(severity) = severity_filter {
            query = query.filter(admin_alerts::severity.eq(severity));
        }

        query.count().get_result(&mut conn)
    }
}
