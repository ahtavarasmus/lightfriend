//! Persistence for per-country SMS provider order. Drives outbound
//! fallback in `ChannelRouter`: countries with a row use the stored
//! `provider_order` (JSON array of channel ids), countries without a
//! row inherit the router default.

use crate::{models::user_models::ProviderRoute, pg_schema::provider_routes, PgDbPool};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ProviderRoutesRepository {
    pool: PgDbPool,
}

impl ProviderRoutesRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    pub fn list_all(&self) -> Result<Vec<ProviderRoute>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        provider_routes::table
            .order(provider_routes::country_code.asc())
            .select(ProviderRoute::as_select())
            .load::<ProviderRoute>(&mut conn)
    }

    /// Insert or update the order for a country. `provider_order_json` must be
    /// a JSON array of channel id strings — the caller is responsible for
    /// validating that the ids are registered channels.
    pub fn upsert(&self, country_code: &str, provider_order_json: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;
        let row = ProviderRoute {
            country_code: country_code.to_string(),
            provider_order: provider_order_json.to_string(),
            updated_at: now,
        };
        diesel::insert_into(provider_routes::table)
            .values(&row)
            .on_conflict(provider_routes::country_code)
            .do_update()
            .set((
                provider_routes::provider_order.eq(&row.provider_order),
                provider_routes::updated_at.eq(row.updated_at),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn delete(&self, country_code: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(
            provider_routes::table.filter(provider_routes::country_code.eq(country_code)),
        )
        .execute(&mut conn)?;
        Ok(())
    }
}
