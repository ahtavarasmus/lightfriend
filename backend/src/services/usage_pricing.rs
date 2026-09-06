use crate::AiProvider;

pub const CUSTOMER_USAGE_MARGIN: f64 = 1.30;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RealtimeTokenUsage {
    pub input_text_tokens: u64,
    pub input_audio_tokens: u64,
    pub cached_input_text_tokens: u64,
    pub cached_input_audio_tokens: u64,
    pub output_text_tokens: u64,
    pub output_audio_tokens: u64,
}

pub fn text_llm_cost_usd(
    provider: AiProvider,
    model: &str,
    prompt_tokens: i32,
    completion_tokens: i32,
) -> f64 {
    let (input_per_million, output_per_million) = text_token_rates(provider, model);
    let input = prompt_tokens.max(0) as f64 * input_per_million / 1_000_000.0;
    let output = completion_tokens.max(0) as f64 * output_per_million / 1_000_000.0;
    input + output
}

pub fn customer_usage_multiplier() -> f64 {
    env_rate("AI_USAGE_MULTIPLIER")
        .filter(|v| *v >= 1.0)
        .unwrap_or(CUSTOMER_USAGE_MARGIN)
}

pub fn billable_customer_cost_usd(provider_cost_usd: f64) -> f64 {
    provider_cost_usd.max(0.0) * customer_usage_multiplier()
}

pub fn openai_realtime_cost_usd(usage: RealtimeTokenUsage) -> f64 {
    openai_realtime_cost_usd_for_model("gpt-realtime-2", usage)
        .expect("the built-in gpt-realtime-2 pricing is always available")
}

pub fn openai_realtime_cost_usd_for_model(
    model: &str,
    usage: RealtimeTokenUsage,
) -> Result<f64, String> {
    let defaults = match model.trim().to_ascii_lowercase().as_str() {
        "gpt-realtime-2" => Some((4.0, 32.0, 0.40, 24.0, 64.0)),
        _ => None,
    };
    let rate = |key: &str, default: Option<f64>| {
        env_rate(key).or(default).ok_or_else(|| {
            format!(
                "{} must be configured when OPENAI_REALTIME_MODEL is '{}'",
                key, model
            )
        })
    };
    let text_input_per_million = rate(
        "OPENAI_REALTIME_TEXT_INPUT_USD_PER_MILLION",
        defaults.map(|rates| rates.0),
    )?;
    let audio_input_per_million = rate(
        "OPENAI_REALTIME_AUDIO_INPUT_USD_PER_MILLION",
        defaults.map(|rates| rates.1),
    )?;
    let cached_input_per_million = rate(
        "OPENAI_REALTIME_CACHED_INPUT_USD_PER_MILLION",
        defaults.map(|rates| rates.2),
    )?;
    let text_output_per_million = rate(
        "OPENAI_REALTIME_TEXT_OUTPUT_USD_PER_MILLION",
        defaults.map(|rates| rates.3),
    )?;
    let audio_output_per_million = rate(
        "OPENAI_REALTIME_AUDIO_OUTPUT_USD_PER_MILLION",
        defaults.map(|rates| rates.4),
    )?;

    let uncached_text = usage
        .input_text_tokens
        .saturating_sub(usage.cached_input_text_tokens);
    let uncached_audio = usage
        .input_audio_tokens
        .saturating_sub(usage.cached_input_audio_tokens);

    Ok((uncached_text as f64 * text_input_per_million
        + uncached_audio as f64 * audio_input_per_million
        + usage.cached_input_text_tokens as f64 * cached_input_per_million
        + usage.cached_input_audio_tokens as f64 * cached_input_per_million
        + usage.output_text_tokens as f64 * text_output_per_million
        + usage.output_audio_tokens as f64 * audio_output_per_million)
        / 1_000_000.0)
}

pub fn openai_realtime_usage_from_event(
    event: &serde_json::Value,
) -> Option<(String, RealtimeTokenUsage)> {
    if event.get("type").and_then(|value| value.as_str()) != Some("response.done") {
        return None;
    }
    let response = event.get("response")?;
    let usage = response.get("usage")?;
    let input = usage.get("input_token_details")?;
    let output = usage.get("output_token_details")?;
    let cached = input.get("cached_tokens_details");
    let cached_total = token_count(input, "cached_tokens");
    let mut cached_text = cached
        .map(|details| token_count(details, "text_tokens"))
        .unwrap_or(0);
    let cached_audio = cached
        .map(|details| token_count(details, "audio_tokens"))
        .unwrap_or(0);
    if cached_text == 0 && cached_audio == 0 {
        // Both cached modalities have the same Realtime price. Assigning an undifferentiated
        // total to text preserves the cached charge on older response payloads.
        cached_text = cached_total;
    }

    Some((
        response.get("id")?.as_str()?.to_string(),
        RealtimeTokenUsage {
            input_text_tokens: token_count(input, "text_tokens"),
            input_audio_tokens: token_count(input, "audio_tokens"),
            cached_input_text_tokens: cached_text,
            cached_input_audio_tokens: cached_audio,
            output_text_tokens: token_count(output, "text_tokens"),
            output_audio_tokens: token_count(output, "audio_tokens"),
        },
    ))
}

fn token_count(value: &serde_json::Value, key: &str) -> u64 {
    value.get(key).and_then(|value| value.as_u64()).unwrap_or(0)
}

fn text_token_rates(provider: AiProvider, model: &str) -> (f64, f64) {
    let rates =
        super::model_pricing::fallback_quote(crate::AiConfig::provider_name(provider), model).rates;
    (rates.input_per_million, rates.output_per_million)
}

fn env_rate(key: &str) -> Option<f64> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
}
