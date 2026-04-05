use crate::pg_models::NewPgLlmUsageLog;
use crate::pg_schema::llm_usage_logs;
use crate::PgDbPool;
use diesel::dsl::count;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct LlmUsageRepository {
    pool: PgDbPool,
}

#[derive(Debug, Serialize)]
pub struct LlmUsageStats {
    pub total_calls: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub by_callsite: Vec<CallsiteBreakdown>,
    pub by_model: Vec<ModelBreakdown>,
}

#[derive(Debug, Serialize)]
pub struct CallsiteBreakdown {
    pub callsite: String,
    pub calls: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize)]
pub struct ModelBreakdown {
    pub model: String,
    pub calls: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Serialize)]
pub struct DailyLlmStat {
    pub date: String,
    pub calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
}

#[derive(Debug, Serialize)]
pub struct UserLlmUsage {
    pub user_id: i32,
    pub calls: i64,
    pub total_tokens: i64,
}

impl LlmUsageRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn log_usage(
        &self,
        user_id: i32,
        provider: &str,
        model: &str,
        callsite: &str,
        prompt_tokens: i32,
        completion_tokens: i32,
        total_tokens: i32,
    ) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let new_log = NewPgLlmUsageLog {
            user_id,
            provider: provider.to_string(),
            model: model.to_string(),
            callsite: callsite.to_string(),
            prompt_tokens,
            completion_tokens,
            total_tokens,
            created_at: now,
        };

        diesel::insert_into(llm_usage_logs::table)
            .values(&new_log)
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn get_stats(&self, from_timestamp: i32) -> Result<LlmUsageStats, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Totals
        let totals: (i64, Option<i64>, Option<i64>, Option<i64>) = llm_usage_logs::table
            .filter(llm_usage_logs::created_at.ge(from_timestamp))
            .select((
                count(llm_usage_logs::id),
                diesel::dsl::sum(llm_usage_logs::prompt_tokens),
                diesel::dsl::sum(llm_usage_logs::completion_tokens),
                diesel::dsl::sum(llm_usage_logs::total_tokens),
            ))
            .first(&mut conn)?;

        // By callsite
        let by_callsite_raw: Vec<(String, i64, Option<i64>)> = llm_usage_logs::table
            .filter(llm_usage_logs::created_at.ge(from_timestamp))
            .group_by(llm_usage_logs::callsite)
            .select((
                llm_usage_logs::callsite,
                count(llm_usage_logs::id),
                diesel::dsl::sum(llm_usage_logs::total_tokens),
            ))
            .order(count(llm_usage_logs::id).desc())
            .load(&mut conn)?;

        // By model
        let by_model_raw: Vec<(String, i64, Option<i64>)> = llm_usage_logs::table
            .filter(llm_usage_logs::created_at.ge(from_timestamp))
            .group_by(llm_usage_logs::model)
            .select((
                llm_usage_logs::model,
                count(llm_usage_logs::id),
                diesel::dsl::sum(llm_usage_logs::total_tokens),
            ))
            .order(count(llm_usage_logs::id).desc())
            .load(&mut conn)?;

        Ok(LlmUsageStats {
            total_calls: totals.0,
            total_prompt_tokens: totals.1.unwrap_or(0),
            total_completion_tokens: totals.2.unwrap_or(0),
            total_tokens: totals.3.unwrap_or(0),
            by_callsite: by_callsite_raw
                .into_iter()
                .map(|(callsite, calls, tokens)| CallsiteBreakdown {
                    callsite,
                    calls,
                    total_tokens: tokens.unwrap_or(0),
                })
                .collect(),
            by_model: by_model_raw
                .into_iter()
                .map(|(model, calls, tokens)| ModelBreakdown {
                    model,
                    calls,
                    total_tokens: tokens.unwrap_or(0),
                })
                .collect(),
        })
    }

    pub fn get_per_user_stats(
        &self,
        from_timestamp: i32,
    ) -> Result<Vec<UserLlmUsage>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let rows: Vec<(i32, i64, Option<i64>)> = llm_usage_logs::table
            .filter(llm_usage_logs::created_at.ge(from_timestamp))
            .group_by(llm_usage_logs::user_id)
            .select((
                llm_usage_logs::user_id,
                count(llm_usage_logs::id),
                diesel::dsl::sum(llm_usage_logs::total_tokens),
            ))
            .order(diesel::dsl::sum(llm_usage_logs::total_tokens).desc())
            .load(&mut conn)?;

        Ok(rows
            .into_iter()
            .map(|(user_id, calls, tokens)| UserLlmUsage {
                user_id,
                calls,
                total_tokens: tokens.unwrap_or(0),
            })
            .collect())
    }

    /// Get total tokens used by a specific user since a given timestamp.
    pub fn get_user_tokens_since(
        &self,
        user_id: i32,
        since_timestamp: i32,
    ) -> Result<i64, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let total: Option<i64> = llm_usage_logs::table
            .filter(llm_usage_logs::user_id.eq(user_id))
            .filter(llm_usage_logs::created_at.ge(since_timestamp))
            .select(diesel::dsl::sum(llm_usage_logs::total_tokens))
            .first(&mut conn)?;

        Ok(total.unwrap_or(0))
    }

    pub fn get_daily_stats(&self, from_timestamp: i32) -> Result<Vec<DailyLlmStat>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        // Load raw rows and aggregate in Rust (simpler than SQL date math with integer timestamps)
        let rows: Vec<(i32, i32, i32)> = llm_usage_logs::table
            .filter(llm_usage_logs::created_at.ge(from_timestamp))
            .select((
                llm_usage_logs::created_at,
                llm_usage_logs::prompt_tokens,
                llm_usage_logs::completion_tokens,
            ))
            .load(&mut conn)?;

        let mut daily: std::collections::BTreeMap<i32, (i64, i64, i64)> =
            std::collections::BTreeMap::new();

        for (ts, pt, ct) in rows {
            let day = (ts / 86400) * 86400;
            let entry = daily.entry(day).or_insert((0, 0, 0));
            entry.0 += 1;
            entry.1 += pt as i64;
            entry.2 += ct as i64;
        }

        Ok(daily
            .into_iter()
            .map(|(day, (calls, prompt_tokens, completion_tokens))| {
                let date = chrono::DateTime::from_timestamp(day as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                DailyLlmStat {
                    date,
                    calls,
                    prompt_tokens,
                    completion_tokens,
                }
            })
            .collect())
    }
}
