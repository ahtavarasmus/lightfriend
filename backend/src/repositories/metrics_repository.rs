use crate::{
    models::user_models::{NewSiteMetric, SiteMetric},
    pg_schema::site_metrics,
    PgDbPool,
};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct MetricsRepository {
    pool: PgDbPool,
}

impl MetricsRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    /// Get a metric by key
    pub fn get_metric(&self, key: &str) -> Result<Option<SiteMetric>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        site_metrics::table
            .filter(site_metrics::metric_key.eq(key))
            .select(SiteMetric::as_select())
            .first::<SiteMetric>(&mut conn)
            .optional()
    }

    /// Upsert a metric (insert or update)
    pub fn upsert_metric(&self, key: &str, value: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        // Check if metric exists
        let existing = site_metrics::table
            .filter(site_metrics::metric_key.eq(key))
            .select(SiteMetric::as_select())
            .first::<SiteMetric>(&mut conn)
            .optional()?;

        if existing.is_some() {
            // Update existing metric
            diesel::update(site_metrics::table.filter(site_metrics::metric_key.eq(key)))
                .set((
                    site_metrics::metric_value.eq(value),
                    site_metrics::updated_at.eq(current_time),
                ))
                .execute(&mut conn)?;
        } else {
            // Insert new metric
            let new_metric = NewSiteMetric {
                metric_key: key.to_string(),
                metric_value: value.to_string(),
                updated_at: current_time,
            };

            diesel::insert_into(site_metrics::table)
                .values(&new_metric)
                .execute(&mut conn)?;
        }

        Ok(())
    }
}
