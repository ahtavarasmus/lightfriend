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

pub fn billable_customer_cost_usd(provider_cost_usd: f64) -> f64 {
    provider_cost_usd.max(0.0) * CUSTOMER_USAGE_MARGIN
}

pub fn openai_realtime_cost_usd(usage: RealtimeTokenUsage) -> f64 {
    let text_input_per_million =
        env_rate("OPENAI_REALTIME_TEXT_INPUT_USD_PER_MILLION").unwrap_or(4.0);
    let audio_input_per_million =
        env_rate("OPENAI_REALTIME_AUDIO_INPUT_USD_PER_MILLION").unwrap_or(32.0);
    let cached_input_per_million =
        env_rate("OPENAI_REALTIME_CACHED_INPUT_USD_PER_MILLION").unwrap_or(0.40);
    let text_output_per_million =
        env_rate("OPENAI_REALTIME_TEXT_OUTPUT_USD_PER_MILLION").unwrap_or(24.0);
    let audio_output_per_million =
        env_rate("OPENAI_REALTIME_AUDIO_OUTPUT_USD_PER_MILLION").unwrap_or(64.0);

    let uncached_text = usage
        .input_text_tokens
        .saturating_sub(usage.cached_input_text_tokens);
    let uncached_audio = usage
        .input_audio_tokens
        .saturating_sub(usage.cached_input_audio_tokens);

    (uncached_text as f64 * text_input_per_million
        + uncached_audio as f64 * audio_input_per_million
        + usage.cached_input_text_tokens as f64 * cached_input_per_million
        + usage.cached_input_audio_tokens as f64 * cached_input_per_million
        + usage.output_text_tokens as f64 * text_output_per_million
        + usage.output_audio_tokens as f64 * audio_output_per_million)
        / 1_000_000.0
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
    let normalized = model.to_ascii_lowercase();
    match provider {
        AiProvider::Tinfoil if normalized.contains("kimi-k2-6") => env_rates(
            "TINFOIL_KIMI_INPUT_USD_PER_MILLION",
            "TINFOIL_KIMI_OUTPUT_USD_PER_MILLION",
            (1.50, 5.25),
        ),
        AiProvider::Tinfoil if normalized.contains("gemma4-31b") => env_rates(
            "TINFOIL_GEMMA_INPUT_USD_PER_MILLION",
            "TINFOIL_GEMMA_OUTPUT_USD_PER_MILLION",
            (0.40, 1.00),
        ),
        AiProvider::Near if normalized.contains("glm-5.1") => env_rates(
            "NEAR_GLM_INPUT_USD_PER_MILLION",
            "NEAR_GLM_OUTPUT_USD_PER_MILLION",
            (0.85, 3.30),
        ),
        AiProvider::Near if normalized.contains("gemma-4-31b") => env_rates(
            "NEAR_GEMMA_INPUT_USD_PER_MILLION",
            "NEAR_GEMMA_OUTPUT_USD_PER_MILLION",
            (0.13, 0.40),
        ),
        AiProvider::OpenRouter if normalized.contains("gpt-4o") => (2.50, 10.00),
        // A configured model should set these generic overrides when it is not one of the
        // known defaults. The fallback is intentionally conservative rather than free usage.
        AiProvider::Tinfoil => env_rates(
            "TINFOIL_INPUT_USD_PER_MILLION",
            "TINFOIL_OUTPUT_USD_PER_MILLION",
            (1.50, 5.25),
        ),
        AiProvider::Near => env_rates(
            "NEAR_AI_INPUT_USD_PER_MILLION",
            "NEAR_AI_OUTPUT_USD_PER_MILLION",
            (0.85, 3.30),
        ),
        AiProvider::OpenRouter => env_rates(
            "OPENROUTER_INPUT_USD_PER_MILLION",
            "OPENROUTER_OUTPUT_USD_PER_MILLION",
            (2.50, 10.00),
        ),
    }
}

fn env_rates(input_key: &str, output_key: &str, defaults: (f64, f64)) -> (f64, f64) {
    (
        env_rate(input_key).unwrap_or(defaults.0),
        env_rate(output_key).unwrap_or(defaults.1),
    )
}

fn env_rate(key: &str) -> Option<f64> {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
}
