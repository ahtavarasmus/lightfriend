//! Provider price catalogs. Prices are keyed by provider and exact model ID.
use super::ai_usage::TokenUsage;
use crate::{AiConfig, AiProvider};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRates {
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub cached_input_per_million: Option<f64>,
    pub request_usd: f64,
    pub search_usd: f64,
}

impl ModelRates {
    pub fn is_valid(&self) -> bool {
        [
            self.input_per_million,
            self.output_per_million,
            self.request_usd,
            self.search_usd,
        ]
        .into_iter()
        .all(|v| v.is_finite() && v >= 0.0)
            && self
                .cached_input_per_million
                .is_none_or(|v| v.is_finite() && v >= 0.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceQuote {
    pub rates: ModelRates,
    pub source: String,
    pub fetched_at: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSnapshot {
    pub quote: PriceQuote,
    pub markup: f64,
    pub provider_cost_usd: f64,
    /// This is a projected charge, not proof that the user was billed.
    pub customer_cost_usd: f64,
    pub reported_cost: bool,
    pub usage_complete: bool,
}

#[derive(Debug, Default)]
pub struct PricingCatalog {
    prices: RwLock<HashMap<(String, String), PriceQuote>>,
}

impl PricingCatalog {
    pub fn insert(&self, provider: &str, model: &str, quote: PriceQuote) {
        if !quote.rates.is_valid() {
            return;
        }
        self.prices
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert((provider.to_string(), model.to_string()), quote);
    }

    pub fn quote(&self, provider: &str, model: &str) -> PriceQuote {
        self.prices
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(provider.to_string(), model.to_string()))
            .cloned()
            .unwrap_or_else(|| fallback_quote(provider, model))
    }

    pub fn cost(&self, provider: &str, model: &str, usage: &TokenUsage) -> CostSnapshot {
        cost_from_quote(provider, self.quote(provider, model), usage)
    }
}

pub fn cost_from_quote(provider: &str, quote: PriceQuote, usage: &TokenUsage) -> CostSnapshot {
    let rates = &quote.rates;
    let input = usage.prompt_tokens.max(0) as f64;
    let output = usage.completion_tokens.max(0) as f64;
    let cached = usage.cached_tokens().unwrap_or(0) as f64;
    // Provider-reported OpenRouter cost includes route-specific and search charges.
    let reported = (provider == "openrouter")
        .then_some(usage.cost)
        .flatten()
        .filter(|v| v.is_finite() && *v >= 0.0);
    let cost = reported.unwrap_or_else(|| {
        ((input - cached) * rates.input_per_million
            + cached
                * rates
                    .cached_input_per_million
                    .unwrap_or(rates.input_per_million)
            + output * rates.output_per_million)
            / 1_000_000.0
            + rates.request_usd
            + rates.search_usd
    });
    let markup = super::usage_pricing::customer_usage_multiplier();
    let usage_complete = reported.is_some()
        || (input + output > 0.0
            && (rates.cached_input_per_million.is_none() || usage.cached_tokens().is_some())
            && rates.search_usd == 0.0);
    CostSnapshot {
        quote,
        markup,
        provider_cost_usd: cost,
        customer_cost_usd: cost * markup,
        reported_cost: reported.is_some(),
        usage_complete,
    }
}

fn number(v: Option<&Value>) -> Option<f64> {
    let v = v?;
    v.as_f64()
        .or_else(|| v.as_str()?.parse().ok())
        .filter(|n| n.is_finite() && *n >= 0.0)
}

/// Invalid entries are skipped individually; a broken catalog never erases saved rates.
pub fn parse_catalog(provider: &str, value: &Value, fetched_at: i32) -> Vec<(String, PriceQuote)> {
    let Some(models) = value.get("data").and_then(Value::as_array) else {
        return vec![];
    };
    models
        .iter()
        .filter_map(|m| {
            let model = m.get("id")?.as_str()?.trim();
            if model.is_empty() {
                return None;
            }
            let p = m.get("pricing")?;
            let (input, output, cached, request, search) = if provider == "tinfoil" {
                (
                    number(p.get("inputTokenPricePer1M"))?,
                    number(p.get("outputTokenPricePer1M"))?,
                    number(p.get("cachedInputTokenPricePer1M")),
                    number(p.get("requestPrice")).unwrap_or(0.0),
                    0.0,
                )
            } else {
                (
                    number(p.get("prompt"))? * 1_000_000.0,
                    number(p.get("completion"))? * 1_000_000.0,
                    number(p.get("input_cache_read")).map(|n| n * 1_000_000.0),
                    number(p.get("request")).unwrap_or(0.0),
                    number(p.get("web_search")).unwrap_or_else(|| {
                        if provider == "openrouter" && model == "perplexity/sonar-reasoning-pro" {
                            0.005
                        } else {
                            0.0
                        }
                    }),
                )
            };
            // Guard conversions as well as input parsing against overflow.
            if !input.is_finite() || !output.is_finite() || cached.is_some_and(|v| !v.is_finite()) {
                return None;
            }
            Some((
                model.to_string(),
                PriceQuote {
                    rates: ModelRates {
                        input_per_million: input,
                        output_per_million: output,
                        cached_input_per_million: cached,
                        request_usd: request,
                        search_usd: search,
                    },
                    source: "provider_api".into(),
                    fetched_at: Some(fetched_at),
                },
            ))
        })
        .collect()
}

/// Last-resort estimates used only when the catalog has no saved rate for this model.
pub fn fallback_quote(provider: &str, model: &str) -> PriceQuote {
    let (input, output, cached, search, known) = match (provider, model) {
        ("tinfoil", "deepseek-v4-flash") => (0.30, 0.70, Some(0.06), 0.0, true),
        ("tinfoil", "gemma4-31b") => (0.40, 1.00, None, 0.0, true),
        ("tinfoil", "kimi-k2-6") => (1.50, 5.25, None, 0.0, true),
        ("near", "zai-org/GLM-5.1-FP8") => (1.40, 4.40, Some(0.26), 0.0, true),
        ("near", "google/gemma-4-31B-it") => (0.13, 0.40, None, 0.0, true),
        ("openrouter", "perplexity/sonar-reasoning-pro") => (2.0, 8.0, None, 0.005, true),
        ("openrouter", "openai/gpt-4o-2024-11-20") => (2.50, 10.0, None, 0.0, true),
        ("near", _) => (1.40, 4.40, None, 0.0, false),
        ("openrouter", _) => (2.50, 10.0, None, 0.0, false),
        _ => (1.50, 5.25, None, 0.0, false),
    };
    PriceQuote {
        rates: ModelRates {
            input_per_million: input,
            output_per_million: output,
            cached_input_per_million: cached,
            request_usd: 0.0,
            search_usd: search,
        },
        source: if known {
            "default_model"
        } else {
            "default_provider"
        }
        .into(),
        fetched_at: None,
    }
}

pub async fn refresh_provider_prices(
    config: &AiConfig,
    repository: &crate::LlmUsageRepository,
    provider: AiProvider,
) -> anyhow::Result<()> {
    if !config.provider_configured(provider) {
        return Ok(());
    }
    let name = AiConfig::provider_name(provider);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let value: Value = client
        .get(format!(
            "{}/models",
            config.endpoint(provider).trim_end_matches('/')
        ))
        .bearer_auth(config.api_key(provider))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let entries = parse_catalog(name, &value, chrono::Utc::now().timestamp() as i32);
    if entries.is_empty() {
        anyhow::bail!("No valid model prices returned");
    }
    let repo = repository.clone();
    let to_save = entries.clone();
    let saved = tokio::task::spawn_blocking(move || repo.save_prices(name, &to_save))
        .await
        .map_err(anyhow::Error::from)
        .and_then(|result| result);
    for (model, quote) in entries {
        config.pricing.insert(name, &model, quote);
    }
    saved
}

pub async fn refresh_prices(config: &AiConfig, repository: &crate::LlmUsageRepository) {
    let futures = [
        AiProvider::Tinfoil,
        AiProvider::Near,
        AiProvider::OpenRouter,
    ]
    .map(|provider| async move {
        if let Err(error) = refresh_provider_prices(config, repository, provider).await {
            tracing::warn!(provider = AiConfig::provider_name(provider), %error,
                "Price refresh failed; retaining saved rates and defaults");
        }
    });
    futures::future::join_all(futures).await;
}

pub async fn start_pricing_refresh(config: AiConfig, repository: Arc<crate::LlmUsageRepository>) {
    let repo = repository.clone();
    if let Err(error) = tokio::task::spawn_blocking(move || repo.load_prices())
        .await
        .map_err(anyhow::Error::from)
        .and_then(|result| result)
    {
        tracing::warn!(%error, "Failed to load saved AI prices; using defaults");
    }
    refresh_prices(&config, &repository).await;
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            refresh_prices(&config, &repository).await;
        }
    });
}
