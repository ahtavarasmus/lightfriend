//! One consistent reporting window; historical rows are estimates, never rewritten charges.
use crate::{
    pg_models::PgLlmUsageLog,
    services::{
        ai_usage::TokenUsage,
        model_pricing::{CostSnapshot, PricingCatalog},
    },
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Default, Serialize)]
pub struct UsageTotals {
    pub calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub cached_prompt_tokens: i64,
    pub provider_cost_usd: f64,
    pub customer_cost_usd: f64,
    pub historical_estimate_calls: i64,
    pub fallback_price_calls: i64,
    pub incomplete_usage_calls: i64,
    pub missing_usage_calls: i64,
}

impl UsageTotals {
    fn add(&mut self, row: &PgLlmUsageLog, cost: &CostSnapshot, historical: bool) {
        self.calls += 1;
        self.prompt_tokens += row.prompt_tokens.max(0) as i64;
        self.completion_tokens += row.completion_tokens.max(0) as i64;
        self.total_tokens += row.total_tokens.max(0) as i64;
        self.cached_prompt_tokens += row.cached_prompt_tokens.unwrap_or(0).max(0) as i64;
        self.provider_cost_usd += cost.provider_cost_usd;
        self.customer_cost_usd += cost.customer_cost_usd;
        self.historical_estimate_calls += i64::from(historical);
        self.fallback_price_calls +=
            i64::from(!cost.reported_cost && cost.quote.source.starts_with("default_"));
        self.incomplete_usage_calls += i64::from(!cost.usage_complete);
        self.missing_usage_calls +=
            i64::from(row.prompt_tokens == 0 && row.completion_tokens == 0 && !cost.reported_cost);
    }
}

#[derive(Serialize)]
pub struct Breakdown {
    pub user_id: Option<i32>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub callsite: Option<String>,
    pub date: Option<String>,
    #[serde(flatten)]
    pub totals: UsageTotals,
}

#[derive(Serialize)]
pub struct AiUsageReport {
    pub days: i32,
    pub from_timestamp: i32,
    pub to_timestamp: i32,
    pub total_calls: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub active_users: usize,
    pub average_tokens_per_day: f64,
    pub projected_monthly_provider_cost_usd: f64,
    pub projected_monthly_customer_cost_usd: f64,
    #[serde(flatten)]
    pub costs: UsageTotals,
    pub by_callsite: Vec<Breakdown>,
    pub by_model: Vec<Breakdown>,
    pub per_user: Vec<Breakdown>,
    pub per_user_detailed: Vec<Breakdown>,
    pub daily_stats: Vec<Breakdown>,
    pub current_prices: Vec<CurrentModelPrice>,
}

#[derive(Serialize)]
pub struct CurrentModelPrice {
    pub provider: String,
    pub model: String,
    #[serde(flatten)]
    pub quote: crate::services::model_pricing::PriceQuote,
}

pub fn build_report(
    rows: &[PgLlmUsageLog],
    prices: &PricingCatalog,
    days: i32,
    now: i32,
) -> AiUsageReport {
    let days = days.clamp(1, 90);
    let from = now.saturating_sub(days * 86400);
    let mut totals = UsageTotals::default();
    let mut callsites = BTreeMap::<String, UsageTotals>::new();
    let mut models = BTreeMap::<(String, String), UsageTotals>::new();
    let mut users = BTreeMap::<i32, UsageTotals>::new();
    let mut details = BTreeMap::<(i32, String, String, String), UsageTotals>::new();
    let mut daily = BTreeMap::<String, UsageTotals>::new();
    for row in rows
        .iter()
        .filter(|r| r.created_at >= from && r.created_at < now)
    {
        let recorded = row
            .pricing_snapshot
            .as_ref()
            .and_then(|s| serde_json::from_str::<CostSnapshot>(s).ok());
        let historical = recorded.is_none();
        let mut cost = recorded.unwrap_or_else(|| {
            prices.cost(
                &row.provider,
                &row.model,
                &TokenUsage {
                    prompt_tokens: row.prompt_tokens,
                    completion_tokens: row.completion_tokens,
                    total_tokens: row.total_tokens,
                    prompt_cache_hit_tokens: row.cached_prompt_tokens,
                    ..Default::default()
                },
            )
        });
        if !historical {
            cost.provider_cost_usd = row.provider_cost_usd.unwrap_or(cost.provider_cost_usd);
            cost.customer_cost_usd = row.customer_cost_usd.unwrap_or(cost.customer_cost_usd);
        }
        totals.add(row, &cost, historical);
        callsites
            .entry(row.callsite.clone())
            .or_default()
            .add(row, &cost, historical);
        models
            .entry((row.provider.clone(), row.model.clone()))
            .or_default()
            .add(row, &cost, historical);
        users
            .entry(row.user_id)
            .or_default()
            .add(row, &cost, historical);
        details
            .entry((
                row.user_id,
                row.provider.clone(),
                row.model.clone(),
                row.callsite.clone(),
            ))
            .or_default()
            .add(row, &cost, historical);
        let date = chrono::DateTime::from_timestamp(row.created_at as i64, 0)
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();
        daily.entry(date).or_default().add(row, &cost, historical);
    }
    let row = |totals| Breakdown {
        user_id: None,
        provider: None,
        model: None,
        callsite: None,
        date: None,
        totals,
    };
    fn by_cost(mut rows: Vec<Breakdown>) -> Vec<Breakdown> {
        rows.sort_by(|a, b| {
            b.totals
                .provider_cost_usd
                .total_cmp(&a.totals.provider_cost_usd)
        });
        rows
    }
    let current_prices = models
        .keys()
        .map(|(provider, model)| CurrentModelPrice {
            provider: provider.clone(),
            model: model.clone(),
            quote: prices.quote(provider, model),
        })
        .collect();
    AiUsageReport {
        days,
        from_timestamp: from,
        to_timestamp: now,
        total_calls: totals.calls,
        total_prompt_tokens: totals.prompt_tokens,
        total_completion_tokens: totals.completion_tokens,
        active_users: users.len(),
        average_tokens_per_day: totals.total_tokens as f64 / days as f64,
        projected_monthly_provider_cost_usd: totals.provider_cost_usd * 30.0 / days as f64,
        projected_monthly_customer_cost_usd: totals.customer_cost_usd * 30.0 / days as f64,
        costs: totals,
        current_prices,
        by_callsite: by_cost(
            callsites
                .into_iter()
                .map(|(key, t)| Breakdown {
                    callsite: Some(key),
                    ..row(t)
                })
                .collect(),
        ),
        by_model: by_cost(
            models
                .into_iter()
                .map(|((p, m), t)| Breakdown {
                    provider: Some(p),
                    model: Some(m),
                    ..row(t)
                })
                .collect(),
        ),
        per_user: by_cost(
            users
                .into_iter()
                .map(|(u, t)| Breakdown {
                    user_id: Some(u),
                    ..row(t)
                })
                .collect(),
        ),
        per_user_detailed: by_cost(
            details
                .into_iter()
                .map(|((u, p, m, c), t)| Breakdown {
                    user_id: Some(u),
                    provider: Some(p),
                    model: Some(m),
                    callsite: Some(c),
                    ..row(t)
                })
                .collect(),
        ),
        daily_stats: daily
            .into_iter()
            .map(|(d, t)| Breakdown {
                date: Some(d),
                ..row(t)
            })
            .collect(),
    }
}
