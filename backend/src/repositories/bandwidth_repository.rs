use crate::pg_models::NewBridgeBandwidthLog;
use crate::pg_schema::bridge_bandwidth_logs;
use crate::PgDbPool;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct BandwidthRepository {
    pool: PgDbPool,
}

#[derive(Debug, Serialize)]
pub struct UserBandwidthUsage {
    pub user_id: i32,
    pub bridge_type: String,
    pub bytes_inbound: i64,
    pub bytes_outbound: i64,
    pub bytes_total: i64,
}

impl BandwidthRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    pub fn log_bandwidth(
        &self,
        user_id: i32,
        bridge_type: &str,
        direction: &str,
        bytes_estimate: i32,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_log = NewBridgeBandwidthLog {
            user_id,
            bridge_type: bridge_type.to_string(),
            direction: direction.to_string(),
            bytes_estimate,
            created_at: now,
        };

        diesel::insert_into(bridge_bandwidth_logs::table)
            .values(&new_log)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_per_user_stats(
        &self,
        from_timestamp: i32,
    ) -> Result<Vec<UserBandwidthUsage>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Load raw rows and aggregate in Rust (need to split by direction)
        let rows: Vec<(i32, String, String, i32)> = bridge_bandwidth_logs::table
            .filter(bridge_bandwidth_logs::created_at.ge(from_timestamp))
            .select((
                bridge_bandwidth_logs::user_id,
                bridge_bandwidth_logs::bridge_type,
                bridge_bandwidth_logs::direction,
                bridge_bandwidth_logs::bytes_estimate,
            ))
            .load(&mut conn)?;

        // Aggregate by (user_id, bridge_type)
        let mut map: std::collections::HashMap<(i32, String), (i64, i64)> =
            std::collections::HashMap::new();

        for (user_id, bridge_type, direction, bytes) in rows {
            let entry = map.entry((user_id, bridge_type)).or_insert((0, 0));
            if direction == "inbound" {
                entry.0 += bytes as i64;
            } else {
                entry.1 += bytes as i64;
            }
        }

        let mut results: Vec<UserBandwidthUsage> = map
            .into_iter()
            .map(
                |((user_id, bridge_type), (bytes_in, bytes_out))| UserBandwidthUsage {
                    user_id,
                    bridge_type,
                    bytes_inbound: bytes_in,
                    bytes_outbound: bytes_out,
                    bytes_total: bytes_in + bytes_out,
                },
            )
            .collect();

        results.sort_by(|a, b| b.bytes_total.cmp(&a.bytes_total));
        Ok(results)
    }

    pub fn get_total_bandwidth(&self, from_timestamp: i32) -> Result<i64, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let total: Option<i64> = bridge_bandwidth_logs::table
            .filter(bridge_bandwidth_logs::created_at.ge(from_timestamp))
            .select(diesel::dsl::sum(bridge_bandwidth_logs::bytes_estimate))
            .first(&mut conn)?;

        Ok(total.unwrap_or(0))
    }
}
