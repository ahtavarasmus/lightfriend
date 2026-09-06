use crate::pg_models::NewPgLlmUsageLog;
use crate::pg_schema::llm_usage_logs;
use crate::services::{
    ai_usage::TokenUsage,
    model_pricing::{CostSnapshot, ModelRates, PriceQuote, PricingCatalog},
};
use crate::PgDbPool;
use diesel::dsl::count;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use serde::Serialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct LlmUsageRepository {
    pool: PgDbPool,
    pub pricing: Arc<PricingCatalog>,
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

#[derive(Debug, Serialize)]
pub struct UserLlmUsageDetailed {
    pub user_id: i32,
    pub model: String,
    pub callsite: String,
    pub calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

type DetailedUsageRow = (
    i32,
    String,
    String,
    i64,
    Option<i64>,
    Option<i64>,
    Option<i64>,
);

impl LlmUsageRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self {
            pool,
            pricing: Arc::new(PricingCatalog::default()),
        }
    }

    pub fn save_price(
        &self,
        provider: &str,
        model: &str,
        quote: &PriceQuote,
    ) -> anyhow::Result<()> {
        self.save_prices(provider, &[(model.to_string(), quote.clone())])
    }

    pub fn save_prices(
        &self,
        provider: &str,
        entries: &[(String, PriceQuote)],
    ) -> anyhow::Result<()> {
        use crate::pg_schema::ai_model_prices as p;
        let mut conn = self.pool.get()?;
        conn.transaction::<_, anyhow::Error, _>(|conn| {
            for (model, quote) in entries {
                if !quote.rates.is_valid() {
                    anyhow::bail!("Invalid model price");
                }
                let rates = serde_json::to_string(&quote.rates)?;
                let fetched_at = quote.fetched_at.unwrap_or(0);
                diesel::insert_into(p::table)
                    .values((
                        p::provider.eq(provider),
                        p::model.eq(model),
                        p::rates.eq(&rates),
                        p::fetched_at.eq(fetched_at),
                    ))
                    .on_conflict((p::provider, p::model))
                    .do_update()
                    .set((p::rates.eq(&rates), p::fetched_at.eq(fetched_at)))
                    .execute(conn)?;
            }
            Ok(())
        })
    }

    pub fn load_prices(&self) -> anyhow::Result<()> {
        use crate::pg_schema::ai_model_prices as p;
        let mut conn = self.pool.get()?;
        let rows: Vec<(String, String, String, i32)> = p::table
            .select((p::provider, p::model, p::rates, p::fetched_at))
            .load(&mut conn)?;
        for (provider, model, rates, fetched_at) in rows {
            match serde_json::from_str::<ModelRates>(&rates) {
                Ok(rates) => self.pricing.insert(
                    &provider,
                    &model,
                    PriceQuote {
                        rates,
                        source: "saved_api".into(),
                        fetched_at: Some(fetched_at),
                    },
                ),
                Err(error) => {
                    tracing::warn!(%error, provider, model, "Invalid saved model pricing")
                }
            }
        }
        Ok(())
    }

    pub fn log_priced_usage(
        &self,
        user_id: i32,
        provider: &str,
        model: &str,
        callsite: &str,
        usage: &TokenUsage,
        snapshot: &CostSnapshot,
    ) -> anyhow::Result<()> {
        let mut conn = self.pool.get()?;
        let log = NewPgLlmUsageLog {
            user_id,
            provider: provider.into(),
            model: model.into(),
            callsite: callsite.into(),
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens.max(
                usage
                    .prompt_tokens
                    .max(0)
                    .saturating_add(usage.completion_tokens.max(0)),
            ),
            created_at: chrono::Utc::now().timestamp() as i32,
            cached_prompt_tokens: usage.cached_tokens(),
            provider_cost_usd: Some(snapshot.provider_cost_usd),
            customer_cost_usd: Some(snapshot.customer_cost_usd),
            pricing_snapshot: Some(serde_json::to_string(snapshot)?),
        };
        diesel::insert_into(llm_usage_logs::table)
            .values(log)
            .execute(&mut conn)?;
        Ok(())
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
            cached_prompt_tokens: None,
            provider_cost_usd: None,
            customer_cost_usd: None,
            pricing_snapshot: None,
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

    pub fn get_cost_report(
        &self,
        days: i32,
        now: i32,
    ) -> anyhow::Result<crate::services::ai_usage_report::AiUsageReport> {
        let days = days.clamp(1, 90);
        let mut conn = self.pool.get()?;
        let rows = llm_usage_logs::table
            .filter(llm_usage_logs::created_at.ge(now.saturating_sub(days * 86400)))
            .filter(llm_usage_logs::created_at.lt(now))
            .select(crate::pg_models::PgLlmUsageLog::as_select())
            .load(&mut conn)?;
        Ok(crate::services::ai_usage_report::build_report(
            &rows,
            &self.pricing,
            days,
            now,
        ))
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

    pub fn get_per_user_detailed_stats(
        &self,
        from_timestamp: i32,
    ) -> Result<Vec<UserLlmUsageDetailed>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");

        let rows: Vec<DetailedUsageRow> = llm_usage_logs::table
            .filter(llm_usage_logs::created_at.ge(from_timestamp))
            .group_by((
                llm_usage_logs::user_id,
                llm_usage_logs::model,
                llm_usage_logs::callsite,
            ))
            .select((
                llm_usage_logs::user_id,
                llm_usage_logs::model,
                llm_usage_logs::callsite,
                count(llm_usage_logs::id),
                diesel::dsl::sum(llm_usage_logs::prompt_tokens),
                diesel::dsl::sum(llm_usage_logs::completion_tokens),
                diesel::dsl::sum(llm_usage_logs::total_tokens),
            ))
            .order(diesel::dsl::sum(llm_usage_logs::total_tokens).desc())
            .load(&mut conn)?;

        Ok(rows
            .into_iter()
            .map(
                |(user_id, model, callsite, calls, pt, ct, tt)| UserLlmUsageDetailed {
                    user_id,
                    model,
                    callsite,
                    calls,
                    prompt_tokens: pt.unwrap_or(0),
                    completion_tokens: ct.unwrap_or(0),
                    total_tokens: tt.unwrap_or(0),
                },
            )
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
