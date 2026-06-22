//! Centralized AI provider configuration
//!
//! Tinfoil is the primary AI provider for all users.
//! NEAR AI can be configured as an automatic backup for text LLM calls.

use openai_api_rs::v1::api::OpenAIClient;
use openai_api_rs::v1::chat_completion;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DEFAULT_OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";
const DEFAULT_TINFOIL_BASE_URL: &str = "https://inference.tinfoil.sh/v1";
const DEFAULT_NEAR_BASE_URL: &str = "https://cloud-api.near.ai/v1";
const DEFAULT_NEAR_DEFAULT_MODEL: &str = "zai-org/GLM-5.1-FP8";
const DEFAULT_NEAR_FAST_MODEL: &str = "google/gemma-4-31B-it";
const TINFOIL_FAILURE_THRESHOLD: u32 = 2;
const TINFOIL_CIRCUIT_COOLDOWN: Duration = Duration::from_secs(90);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AiProvider {
    OpenRouter,
    Tinfoil,
    Near,
}

#[derive(Debug, Clone, Copy)]
pub enum ModelPurpose {
    /// Normal conversation/tasks with tool calling (reasoning model)
    Default,
    /// Fast model for voice calls (low latency, no reasoning)
    Voice,
}

#[derive(Debug, Clone)]
pub struct AiChatOptions {
    pub preferred_provider: AiProvider,
    pub allow_fallback: bool,
    pub sticky_provider: Option<AiProvider>,
    pub reasoning_tx: Option<tokio::sync::mpsc::Sender<String>>,
}

impl Default for AiChatOptions {
    fn default() -> Self {
        Self {
            preferred_provider: AiProvider::Tinfoil,
            allow_fallback: true,
            sticky_provider: None,
            reasoning_tx: None,
        }
    }
}

pub struct AiChatResult {
    pub response: chat_completion::ChatCompletionResponse,
    pub provider: AiProvider,
    pub model: String,
    pub fallback_from: Option<AiProvider>,
}

#[derive(Debug, Clone)]
struct CircuitBreakerConfig {
    failure_threshold: u32,
    cooldown: Duration,
}

#[derive(Debug, Default)]
struct CircuitBreakerState {
    tinfoil_failures: u32,
    tinfoil_open_until: Option<Instant>,
}

/// Centralized AI configuration
#[derive(Debug, Clone)]
pub struct AiConfig {
    // OpenRouter key kept as optional fallback (only needed if OpenRouter is used directly)
    openrouter_api_key: Option<String>,
    tinfoil_api_key: Option<String>,
    near_api_key: Option<String>,
    openrouter_base_url: String,
    tinfoil_base_url: String,
    near_base_url: String,
    near_default_model: String,
    near_fast_model: String,
    circuit_breaker: Arc<Mutex<CircuitBreakerState>>,
    circuit_breaker_config: CircuitBreakerConfig,
}

impl AiConfig {
    /// Create a minimal AiConfig for tests (no actual API calls will be made)
    pub fn default_for_tests() -> Self {
        Self {
            openrouter_api_key: Some("test_openrouter_key".to_string()),
            tinfoil_api_key: Some("test_tinfoil_key".to_string()),
            near_api_key: Some("test_near_key".to_string()),
            openrouter_base_url: DEFAULT_OPENROUTER_BASE_URL.to_string(),
            tinfoil_base_url: DEFAULT_TINFOIL_BASE_URL.to_string(),
            near_base_url: DEFAULT_NEAR_BASE_URL.to_string(),
            near_default_model: DEFAULT_NEAR_DEFAULT_MODEL.to_string(),
            near_fast_model: DEFAULT_NEAR_FAST_MODEL.to_string(),
            circuit_breaker: Arc::new(Mutex::new(CircuitBreakerState::default())),
            circuit_breaker_config: CircuitBreakerConfig {
                failure_threshold: TINFOIL_FAILURE_THRESHOLD,
                cooldown: TINFOIL_CIRCUIT_COOLDOWN,
            },
        }
    }

    pub fn from_env() -> Self {
        let openrouter_api_key = std::env::var("OPENROUTER_API_KEY").ok();

        let tinfoil_api_key =
            Some(std::env::var("TINFOIL_API_KEY").expect("TINFOIL_API_KEY required"));
        let near_api_key = std::env::var("NEAR_AI_API_KEY").ok();
        let near_base_url =
            std::env::var("NEAR_AI_BASE_URL").unwrap_or_else(|_| DEFAULT_NEAR_BASE_URL.to_string());
        let near_default_model = std::env::var("NEAR_AI_DEFAULT_MODEL")
            .unwrap_or_else(|_| DEFAULT_NEAR_DEFAULT_MODEL.to_string());
        let near_fast_model = std::env::var("NEAR_AI_FAST_MODEL")
            .unwrap_or_else(|_| DEFAULT_NEAR_FAST_MODEL.to_string());

        tracing::info!(
            near_fallback_enabled = near_api_key.is_some(),
            "AI config initialized: Tinfoil primary, NEAR backup"
        );

        Self {
            openrouter_api_key,
            tinfoil_api_key,
            near_api_key,
            openrouter_base_url: DEFAULT_OPENROUTER_BASE_URL.to_string(),
            tinfoil_base_url: DEFAULT_TINFOIL_BASE_URL.to_string(),
            near_base_url,
            near_default_model,
            near_fast_model,
            circuit_breaker: Arc::new(Mutex::new(CircuitBreakerState::default())),
            circuit_breaker_config: CircuitBreakerConfig {
                failure_threshold: TINFOIL_FAILURE_THRESHOLD,
                cooldown: TINFOIL_CIRCUIT_COOLDOWN,
            },
        }
    }

    /// Always returns Tinfoil. The preference parameter is accepted for
    /// backward compatibility but ignored.
    pub fn provider_for_user_with_preference(&self, _preference: Option<&str>) -> AiProvider {
        AiProvider::Tinfoil
    }

    /// Get the endpoint URL for a provider
    pub fn endpoint(&self, provider: AiProvider) -> &str {
        match provider {
            AiProvider::OpenRouter => &self.openrouter_base_url,
            AiProvider::Tinfoil => &self.tinfoil_base_url,
            AiProvider::Near => &self.near_base_url,
        }
    }

    /// Get the API key for a provider
    pub fn api_key(&self, provider: AiProvider) -> &str {
        match provider {
            AiProvider::OpenRouter => self
                .openrouter_api_key
                .as_ref()
                .expect("OPENROUTER_API_KEY not set but OpenRouter provider requested"),
            AiProvider::Tinfoil => self
                .tinfoil_api_key
                .as_ref()
                .expect("Tinfoil API key not configured"),
            AiProvider::Near => self
                .near_api_key
                .as_ref()
                .expect("NEAR_AI_API_KEY not set but NEAR provider requested"),
        }
    }

    /// Get the model name for a provider and purpose
    pub fn model(&self, provider: AiProvider, purpose: ModelPurpose) -> &str {
        match (provider, purpose) {
            (AiProvider::OpenRouter, ModelPurpose::Default) => "openai/gpt-4o-2024-11-20",
            (AiProvider::Tinfoil, ModelPurpose::Default) => "kimi-k2-6",
            (AiProvider::Near, ModelPurpose::Default) => &self.near_default_model,
            // Voice: fast non-reasoning model for low-latency responses
            (AiProvider::Tinfoil, ModelPurpose::Voice) => "gemma4-31b",
            (AiProvider::OpenRouter, ModelPurpose::Voice) => "openai/gpt-4o-2024-11-20",
            (AiProvider::Near, ModelPurpose::Voice) => &self.near_fast_model,
        }
    }

    /// Create an OpenAI-compatible client for a specific provider
    pub fn create_client(
        &self,
        provider: AiProvider,
    ) -> Result<OpenAIClient, Box<dyn std::error::Error>> {
        OpenAIClient::builder()
            .with_endpoint(self.endpoint(provider))
            .with_api_key(self.api_key(provider))
            .build()
    }

    pub fn provider_name(provider: AiProvider) -> &'static str {
        match provider {
            AiProvider::OpenRouter => "openrouter",
            AiProvider::Tinfoil => "tinfoil",
            AiProvider::Near => "near",
        }
    }

    fn provider_configured(&self, provider: AiProvider) -> bool {
        match provider {
            AiProvider::OpenRouter => self.openrouter_api_key.is_some(),
            AiProvider::Tinfoil => self.tinfoil_api_key.is_some(),
            AiProvider::Near => self.near_api_key.is_some(),
        }
    }

    fn is_tinfoil_circuit_open(&self) -> bool {
        let mut state = match self.circuit_breaker.lock() {
            Ok(state) => state,
            Err(_) => return false,
        };

        match state.tinfoil_open_until {
            Some(until) if Instant::now() < until => true,
            Some(_) => {
                state.tinfoil_open_until = None;
                state.tinfoil_failures = 0;
                false
            }
            None => false,
        }
    }

    fn record_provider_success(&self, provider: AiProvider) {
        if provider != AiProvider::Tinfoil {
            return;
        }
        if let Ok(mut state) = self.circuit_breaker.lock() {
            state.tinfoil_failures = 0;
            state.tinfoil_open_until = None;
        }
    }

    fn record_provider_failure(&self, provider: AiProvider, failure_category: &str) {
        if provider != AiProvider::Tinfoil {
            return;
        }
        if let Ok(mut state) = self.circuit_breaker.lock() {
            state.tinfoil_failures = state.tinfoil_failures.saturating_add(1);
            if state.tinfoil_failures >= self.circuit_breaker_config.failure_threshold {
                let open_until = Instant::now() + self.circuit_breaker_config.cooldown;
                state.tinfoil_open_until = Some(open_until);
                tracing::warn!(
                    provider = Self::provider_name(provider),
                    failure_category,
                    failures = state.tinfoil_failures,
                    cooldown_secs = self.circuit_breaker_config.cooldown.as_secs(),
                    "AI provider circuit opened"
                );
            }
        }
    }

    fn provider_attempts(
        &self,
        preferred_provider: AiProvider,
        sticky_provider: Option<AiProvider>,
        allow_fallback: bool,
    ) -> Vec<AiProvider> {
        if let Some(provider) = sticky_provider {
            return if self.provider_configured(provider) {
                vec![provider]
            } else {
                vec![]
            };
        }

        if preferred_provider != AiProvider::Tinfoil || !allow_fallback {
            return if self.provider_configured(preferred_provider) {
                vec![preferred_provider]
            } else {
                vec![]
            };
        }

        let near_configured = self.provider_configured(AiProvider::Near);
        if near_configured && self.is_tinfoil_circuit_open() {
            return vec![AiProvider::Near];
        }

        let mut providers = vec![AiProvider::Tinfoil];
        if near_configured {
            providers.push(AiProvider::Near);
        }
        providers
    }

    fn failure_category(error: &openai_api_rs::v1::error::APIError) -> &'static str {
        let lower = format!("{:?}", error).to_lowercase();
        if lower.contains("timeout") || lower.contains("timed out") {
            "timeout"
        } else if lower.contains("429") {
            "rate_limited"
        } else if lower.contains("500")
            || lower.contains("502")
            || lower.contains("503")
            || lower.contains("504")
        {
            "server_error"
        } else if lower.contains("provider error") {
            "provider_error_body"
        } else if lower.contains("sse assembly")
            || lower.contains("streaming error")
            || lower.contains("no data")
            || lower.contains("failed to read streaming")
        {
            "stream_error"
        } else if lower.contains("failed to parse") || lower.contains("deserialize") {
            "malformed_response"
        } else if lower.contains("connect") || lower.contains("dns") || lower.contains("reqwest") {
            "transport_error"
        } else {
            "provider_error"
        }
    }

    fn fallback_eligible(error: &openai_api_rs::v1::error::APIError) -> bool {
        matches!(
            Self::failure_category(error),
            "timeout"
                | "rate_limited"
                | "server_error"
                | "provider_error_body"
                | "stream_error"
                | "malformed_response"
                | "transport_error"
                | "provider_error"
        )
    }

    pub async fn chat_completion_with_fallback(
        &self,
        usage_repo: Option<&Arc<crate::repositories::llm_usage_repository::LlmUsageRepository>>,
        user_id: i32,
        purpose: ModelPurpose,
        callsite: &str,
        request_template: &chat_completion::ChatCompletionRequest,
        options: AiChatOptions,
    ) -> Result<AiChatResult, openai_api_rs::v1::error::APIError> {
        let providers = self.provider_attempts(
            options.preferred_provider,
            options.sticky_provider,
            options.allow_fallback,
        );

        if providers.is_empty() {
            return Err(openai_api_rs::v1::error::APIError::CustomError {
                message: "No configured AI providers available".to_string(),
            });
        }

        let mut fallback_from: Option<AiProvider> = None;
        let mut last_error: Option<openai_api_rs::v1::error::APIError> = None;

        for (idx, provider) in providers.iter().copied().enumerate() {
            let model = self.model(provider, purpose).to_string();
            let mut request = request_template.clone();
            request.model = model.clone();

            let result = self
                .chat_completion_streaming_with_attempts(
                    provider,
                    &request,
                    options.reasoning_tx.clone(),
                    1,
                )
                .await;

            match result {
                Ok(response) => {
                    self.record_provider_success(provider);
                    if let Some(repo) = usage_repo {
                        log_llm_usage(
                            repo,
                            user_id,
                            Self::provider_name(provider),
                            &model,
                            callsite,
                            &response,
                        );
                    }
                    if let Some(from) = fallback_from {
                        tracing::warn!(
                            user_id,
                            callsite,
                            fallback_from = Self::provider_name(from),
                            fallback_to = Self::provider_name(provider),
                            model,
                            "AI provider fallback succeeded"
                        );
                    }
                    return Ok(AiChatResult {
                        response,
                        provider,
                        model,
                        fallback_from,
                    });
                }
                Err(err) => {
                    let category = Self::failure_category(&err);
                    if Self::fallback_eligible(&err) {
                        self.record_provider_failure(provider, category);
                    }

                    let can_fallback = idx + 1 < providers.len() && provider == AiProvider::Tinfoil;
                    if can_fallback && Self::fallback_eligible(&err) {
                        let next = providers[idx + 1];
                        tracing::warn!(
                            user_id,
                            callsite,
                            fallback_from = Self::provider_name(provider),
                            fallback_to = Self::provider_name(next),
                            failure_category = category,
                            error = ?err,
                            "AI provider fallback triggered"
                        );
                        fallback_from = Some(provider);
                        last_error = Some(err);
                        continue;
                    }

                    return Err(err);
                }
            }
        }

        Err(
            last_error.unwrap_or_else(|| openai_api_rs::v1::error::APIError::CustomError {
                message: "AI provider fallback exhausted".to_string(),
            }),
        )
    }

    /// Sanitize message content for Tinfoil API.
    /// Replaces characters known to cause 500 errors (e.g. `@` in email addresses).
    fn sanitize_content(s: &str) -> String {
        s.replace('@', "(at)")
    }

    /// Apply sanitization to all message content in a serialized request body.
    /// Only used for Tinfoil provider.
    fn sanitize_request_body(body: &mut serde_json::Value) {
        if let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) {
            for msg in messages {
                if let Some(content) = msg
                    .get("content")
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string())
                {
                    let sanitized = Self::sanitize_content(&content);
                    msg["content"] = serde_json::Value::String(sanitized);
                }
            }
        }
    }

    /// Apply model-specific request overrides for providers that expose extra controls.
    fn apply_model_specific_request_overrides(body: &mut serde_json::Value) {
        let model = body
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or_default();

        if model == "gemma4-31b" || model == "kimi-k2-6" {
            if let Some(obj) = body.as_object_mut() {
                obj.insert("enable_thinking".into(), serde_json::json!(false));
            }
        }
    }

    /// Make a chat completion request directly via reqwest.
    /// For Tinfoil: uses streaming (SSE) to avoid a bug where non-streaming + tools + long
    /// content fails. SSE chunks are collected and reassembled into a normal response.
    /// For OpenRouter: uses standard non-streaming mode.
    /// Retries up to 3 times on transient errors.
    pub async fn chat_completion(
        &self,
        provider: AiProvider,
        request: &openai_api_rs::v1::chat_completion::ChatCompletionRequest,
    ) -> Result<
        openai_api_rs::v1::chat_completion::ChatCompletionResponse,
        openai_api_rs::v1::error::APIError,
    > {
        self.chat_completion_with_attempts(provider, request, 3)
            .await
    }

    async fn chat_completion_with_attempts(
        &self,
        provider: AiProvider,
        request: &openai_api_rs::v1::chat_completion::ChatCompletionRequest,
        max_attempts: u32,
    ) -> Result<
        openai_api_rs::v1::chat_completion::ChatCompletionResponse,
        openai_api_rs::v1::error::APIError,
    > {
        let url = format!("{}/chat/completions", self.endpoint(provider));
        let api_key = self.api_key(provider);
        let use_streaming = provider == AiProvider::Tinfoil;

        // Longer timeout for Tinfoil (reasoning model can take 30-60s on large inputs)
        let timeout_secs = if use_streaming { 120 } else { 60 };
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let mut last_error = String::new();
        for attempt in 1..=max_attempts {
            // For Tinfoil: serialize to Value and inject stream:true
            let response = if use_streaming {
                let mut body = serde_json::to_value(request).map_err(|e| {
                    openai_api_rs::v1::error::APIError::CustomError {
                        message: format!("Failed to serialize request: {}", e),
                    }
                })?;
                if let Some(obj) = body.as_object_mut() {
                    obj.insert("stream".into(), serde_json::json!(true));
                    // Remove max_tokens for reasoning models - reasoning tokens count
                    // against the budget, causing finish_reason:length before the
                    // actual tool call is produced
                    obj.remove("max_tokens");
                }
                // Sanitize message content for Tinfoil
                Self::sanitize_request_body(&mut body);
                Self::apply_model_specific_request_overrides(&mut body);
                client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await?
            } else {
                client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(request)
                    .send()
                    .await?
            };

            let status = response.status();

            // Streaming path: collect SSE chunks and assemble response
            if use_streaming && status.is_success() {
                let sse_text = response.text().await.map_err(|e| {
                    openai_api_rs::v1::error::APIError::CustomError {
                        message: format!("Failed to read streaming response: {}", e),
                    }
                })?;

                match Self::assemble_sse_response(&sse_text) {
                    Ok(chat_response) => return Ok(chat_response),
                    Err(e) => {
                        last_error = format!("SSE assembly error: {}", e);
                        if attempt < max_attempts {
                            tracing::warn!(
                                "chat_completion streaming attempt {}/{} failed: {}",
                                attempt,
                                max_attempts,
                                last_error
                            );
                            tokio::time::sleep(tokio::time::Duration::from_millis(
                                500 * attempt as u64,
                            ))
                            .await;
                            continue;
                        }
                        return Err(openai_api_rs::v1::error::APIError::CustomError {
                            message: last_error,
                        });
                    }
                }
            }

            // Non-streaming path (OpenRouter, or streaming error status)
            let text = response.text().await.unwrap_or_default();

            if !status.is_success() {
                last_error = format!("{}: {}", status, text);
                if attempt < max_attempts {
                    tracing::warn!(
                        "chat_completion attempt {}/{} failed: {}",
                        attempt,
                        max_attempts,
                        last_error
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt as u64))
                        .await;
                    continue;
                }
                return Err(openai_api_rs::v1::error::APIError::CustomError {
                    message: last_error,
                });
            }

            // Some providers (Tinfoil) return HTTP 200 but with an error body
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(err_obj) = parsed.get("error").and_then(|e| e.as_object()) {
                    let msg = err_obj
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown error");
                    let err_type = err_obj
                        .get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("unknown");
                    last_error = format!("Provider error ({}): {}", err_type, msg);
                    if attempt < max_attempts {
                        tracing::warn!(
                            "chat_completion attempt {}/{} got error body: {}",
                            attempt,
                            max_attempts,
                            last_error
                        );
                        tokio::time::sleep(tokio::time::Duration::from_millis(
                            500 * attempt as u64,
                        ))
                        .await;
                        continue;
                    }
                    return Err(openai_api_rs::v1::error::APIError::CustomError {
                        message: last_error,
                    });
                }
            }

            // Success - parse the non-streaming response
            let mut json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
                openai_api_rs::v1::error::APIError::CustomError {
                    message: format!("Failed to parse JSON: {} / response {}", e, text),
                }
            })?;

            // Inject missing fields that openai-api-rs requires as non-optional
            Self::inject_missing_fields(&mut json);

            return serde_json::from_value(json).map_err(|e| {
                openai_api_rs::v1::error::APIError::CustomError {
                    message: format!("Failed to parse response: {}", e),
                }
            });
        }

        // All retries exhausted
        Err(openai_api_rs::v1::error::APIError::CustomError {
            message: format!("All {} attempts failed: {}", max_attempts, last_error),
        })
    }

    /// Inject fields that openai-api-rs requires but some providers omit
    fn inject_missing_fields(json: &mut serde_json::Value) {
        if let Some(obj) = json.as_object_mut() {
            if !obj.contains_key("object") {
                obj.insert("object".into(), serde_json::json!("chat.completion"));
            }
            if !obj.contains_key("created") {
                obj.insert("created".into(), serde_json::json!(0));
            }
            if !obj.contains_key("model") {
                obj.insert("model".into(), serde_json::json!("unknown"));
            }
            if !obj.contains_key("usage") {
                obj.insert(
                    "usage".into(),
                    serde_json::json!({
                        "prompt_tokens": 0,
                        "completion_tokens": 0,
                        "total_tokens": 0
                    }),
                );
            }
        }
    }

    /// Assemble a ChatCompletionResponse from SSE text (streaming chunks).
    /// Parses "data: {...}" lines, accumulates content/tool_calls deltas,
    /// and builds a complete response matching the non-streaming format.
    fn assemble_sse_response(
        sse_text: &str,
    ) -> Result<openai_api_rs::v1::chat_completion::ChatCompletionResponse, String> {
        let mut response_id = String::new();
        let mut response_model = String::new();
        let mut content = String::new();
        let mut role = "assistant".to_string();
        let mut finish_reason: Option<String> = None;
        // tool_calls: index -> (id, type, name, arguments)
        let mut tool_calls: std::collections::BTreeMap<i64, (String, String, String, String)> =
            std::collections::BTreeMap::new();
        let mut has_data = false;
        let mut usage_prompt_tokens: i64 = 0;
        let mut usage_completion_tokens: i64 = 0;
        let mut usage_total_tokens: i64 = 0;

        for line in sse_text.lines() {
            let line = line.trim();
            let data = match line.strip_prefix("data: ") {
                Some(d) if d.trim() != "[DONE]" => d,
                _ => continue,
            };

            let chunk: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            has_data = true;

            // Capture usage from the final SSE chunk (sent when stream_options.include_usage=true)
            if let Some(usage) = chunk.get("usage") {
                if let Some(pt) = usage.get("prompt_tokens").and_then(|v| v.as_i64()) {
                    usage_prompt_tokens = pt;
                }
                if let Some(ct) = usage.get("completion_tokens").and_then(|v| v.as_i64()) {
                    usage_completion_tokens = ct;
                }
                if let Some(tt) = usage.get("total_tokens").and_then(|v| v.as_i64()) {
                    usage_total_tokens = tt;
                }
            }

            // Check for error in chunk
            if chunk.get("error").is_some() {
                return Err(format!("Streaming error: {}", data));
            }

            if response_id.is_empty() {
                if let Some(id) = chunk.get("id").and_then(|v| v.as_str()) {
                    response_id = id.to_string();
                }
            }
            if response_model.is_empty() {
                if let Some(model) = chunk.get("model").and_then(|v| v.as_str()) {
                    response_model = model.to_string();
                }
            }

            let choices = match chunk.get("choices").and_then(|v| v.as_array()) {
                Some(c) => c,
                None => continue,
            };

            for choice in choices {
                if let Some(delta) = choice.get("delta") {
                    if let Some(r) = delta.get("role").and_then(|v| v.as_str()) {
                        role = r.to_string();
                    }
                    if let Some(c) = delta.get("content").and_then(|v| v.as_str()) {
                        content.push_str(c);
                    }
                    if let Some(tcs) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                        for tc in tcs {
                            let idx = tc.get("index").and_then(|v| v.as_i64()).unwrap_or(0);
                            let entry = tool_calls.entry(idx).or_insert_with(|| {
                                (
                                    String::new(),
                                    "function".to_string(),
                                    String::new(),
                                    String::new(),
                                )
                            });
                            if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                                if entry.0.is_empty() {
                                    entry.0 = id.to_string();
                                }
                            }
                            if let Some(t) = tc.get("type").and_then(|v| v.as_str()) {
                                entry.1 = t.to_string();
                            }
                            if let Some(func) = tc.get("function") {
                                if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
                                    if entry.2.is_empty() {
                                        entry.2 = name.to_string();
                                    }
                                }
                                if let Some(args) = func.get("arguments").and_then(|v| v.as_str()) {
                                    entry.3.push_str(args);
                                }
                            }
                        }
                    }
                }
                if let Some(fr) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                    finish_reason = Some(fr.to_string());
                }
            }
        }

        if !has_data {
            return Err("No data received from streaming response".to_string());
        }

        // Strip Kimi K2.5 tool-call markers that leak into the content text
        // The model sometimes emits tool calls both as proper tool_calls deltas
        // AND as text markers in the content stream
        if let Some(pos) = content.find("<|tool_calls_section_begin|>") {
            content.truncate(pos);
        }
        let content = content.trim().to_string();

        // Build assembled response JSON matching non-streaming format
        let message_content = if content.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::json!(content)
        };

        let mut message = serde_json::json!({
            "role": role,
            "content": message_content,
        });

        if !tool_calls.is_empty() {
            let tc_array: Vec<serde_json::Value> = tool_calls
                .into_iter()
                .map(|(_, (id, tc_type, name, args))| {
                    serde_json::json!({
                        "id": id,
                        "type": tc_type,
                        "function": {
                            "name": name,
                            "arguments": args
                        }
                    })
                })
                .collect();
            message
                .as_object_mut()
                .unwrap()
                .insert("tool_calls".into(), serde_json::json!(tc_array));
        }

        let mut json = serde_json::json!({
            "id": if response_id.is_empty() { "chatcmpl-stream".to_string() } else { response_id },
            "object": "chat.completion",
            "created": 0,
            "model": if response_model.is_empty() { "unknown".to_string() } else { response_model },
            "choices": [{
                "index": 0,
                "message": message,
                "finish_reason": finish_reason,
            }],
            "usage": {
                "prompt_tokens": usage_prompt_tokens,
                "completion_tokens": usage_completion_tokens,
                "total_tokens": usage_total_tokens
            }
        });

        Self::inject_missing_fields(&mut json);

        serde_json::from_value(json)
            .map_err(|e| format!("Failed to deserialize assembled response: {}", e))
    }

    /// Take the last ~80 chars of accumulated reasoning, trim to word boundary.
    fn format_reasoning_snippet(reasoning: &str) -> String {
        let trimmed = reasoning.trim();
        let chars: Vec<char> = trimmed.chars().collect();
        if chars.len() <= 80 {
            return trimmed.to_string();
        }
        let tail: String = chars[chars.len() - 80..].iter().collect();
        // Find first space to land on a word boundary
        if let Some(pos) = tail.find(' ') {
            format!("...{}", &tail[pos + 1..])
        } else {
            format!("...{}", tail)
        }
    }

    /// Like `chat_completion` but incrementally processes SSE chunks so that
    /// reasoning tokens (`delta.reasoning` / `delta.reasoning_content`) can be
    /// forwarded to a caller via `reasoning_tx`.
    ///
    /// When `reasoning_tx` is None this delegates to the existing
    /// `chat_completion` (SMS path - zero behavior change).
    pub async fn chat_completion_streaming(
        &self,
        provider: AiProvider,
        request: &openai_api_rs::v1::chat_completion::ChatCompletionRequest,
        reasoning_tx: Option<tokio::sync::mpsc::Sender<String>>,
    ) -> Result<
        openai_api_rs::v1::chat_completion::ChatCompletionResponse,
        openai_api_rs::v1::error::APIError,
    > {
        self.chat_completion_streaming_with_attempts(provider, request, reasoning_tx, 3)
            .await
    }

    async fn chat_completion_streaming_with_attempts(
        &self,
        provider: AiProvider,
        request: &openai_api_rs::v1::chat_completion::ChatCompletionRequest,
        reasoning_tx: Option<tokio::sync::mpsc::Sender<String>>,
        max_attempts: u32,
    ) -> Result<
        openai_api_rs::v1::chat_completion::ChatCompletionResponse,
        openai_api_rs::v1::error::APIError,
    > {
        // Fast path: no reasoning channel -> reuse existing method unchanged
        let reasoning_tx = match reasoning_tx {
            Some(tx) => tx,
            None => {
                return self
                    .chat_completion_with_attempts(provider, request, max_attempts)
                    .await
            }
        };

        if provider != AiProvider::Tinfoil {
            return self
                .chat_completion_with_attempts(provider, request, max_attempts)
                .await;
        }

        use futures::StreamExt;

        let url = format!("{}/chat/completions", self.endpoint(provider));
        let api_key = self.api_key(provider);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let mut last_error = String::new();
        for attempt in 1..=max_attempts {
            let mut body = serde_json::to_value(request).map_err(|e| {
                openai_api_rs::v1::error::APIError::CustomError {
                    message: format!("Failed to serialize request: {}", e),
                }
            })?;
            if let Some(obj) = body.as_object_mut() {
                obj.insert("stream".into(), serde_json::json!(true));
                obj.remove("max_tokens");
            }
            Self::sanitize_request_body(&mut body);
            Self::apply_model_specific_request_overrides(&mut body);

            let response = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await?;

            let status = response.status();
            if !status.is_success() {
                let text = response.text().await.unwrap_or_default();
                last_error = format!("{}: {}", status, text);
                if attempt < max_attempts {
                    tracing::warn!(
                        "chat_completion_streaming attempt {}/{} failed: {}",
                        attempt,
                        max_attempts,
                        last_error
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt as u64))
                        .await;
                    continue;
                }
                return Err(openai_api_rs::v1::error::APIError::CustomError {
                    message: last_error,
                });
            }

            // Incremental SSE processing via bytes_stream
            let mut stream = response.bytes_stream();
            let mut buf = String::new();
            let mut reasoning = String::new();
            let mut content = String::new();
            let mut role = "assistant".to_string();
            let mut finish_reason: Option<String> = None;
            let mut response_id = String::new();
            let mut response_model = String::new();
            let mut tool_calls: std::collections::BTreeMap<i64, (String, String, String, String)> =
                std::collections::BTreeMap::new();
            let mut has_data = false;
            let mut last_reasoning_send = std::time::Instant::now();
            let mut stream_error: Option<String> = None;
            let mut usage_prompt_tokens: i64 = 0;
            let mut usage_completion_tokens: i64 = 0;
            let mut usage_total_tokens: i64 = 0;

            while let Some(chunk_result) = stream.next().await {
                let bytes = match chunk_result {
                    Ok(b) => b,
                    Err(e) => {
                        stream_error = Some(format!("Stream read error: {}", e));
                        break;
                    }
                };
                buf.push_str(&String::from_utf8_lossy(&bytes));

                // Process complete lines
                while let Some(newline_pos) = buf.find('\n') {
                    let line = buf[..newline_pos].trim().to_string();
                    buf = buf[newline_pos + 1..].to_string();

                    let data = match line.strip_prefix("data: ") {
                        Some(d) if d.trim() == "[DONE]" => continue,
                        Some(d) => d.to_string(),
                        None => continue,
                    };

                    let chunk: serde_json::Value = match serde_json::from_str(&data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    has_data = true;

                    // Capture usage from the final SSE chunk
                    if let Some(usage) = chunk.get("usage") {
                        if let Some(pt) = usage.get("prompt_tokens").and_then(|v| v.as_i64()) {
                            usage_prompt_tokens = pt;
                        }
                        if let Some(ct) = usage.get("completion_tokens").and_then(|v| v.as_i64()) {
                            usage_completion_tokens = ct;
                        }
                        if let Some(tt) = usage.get("total_tokens").and_then(|v| v.as_i64()) {
                            usage_total_tokens = tt;
                        }
                    }

                    if chunk.get("error").is_some() {
                        stream_error = Some(format!("Streaming error: {}", data));
                        break;
                    }

                    if response_id.is_empty() {
                        if let Some(id) = chunk.get("id").and_then(|v| v.as_str()) {
                            response_id = id.to_string();
                        }
                    }
                    if response_model.is_empty() {
                        if let Some(model) = chunk.get("model").and_then(|v| v.as_str()) {
                            response_model = model.to_string();
                        }
                    }

                    let choices = match chunk.get("choices").and_then(|v| v.as_array()) {
                        Some(c) => c,
                        None => continue,
                    };

                    for choice in choices {
                        if let Some(delta) = choice.get("delta") {
                            if let Some(r) = delta.get("role").and_then(|v| v.as_str()) {
                                role = r.to_string();
                            }
                            if let Some(c) = delta.get("content").and_then(|v| v.as_str()) {
                                content.push_str(c);
                            }
                            // Capture reasoning tokens (kimi uses "reasoning" or "reasoning_content")
                            if let Some(r) = delta
                                .get("reasoning")
                                .or_else(|| delta.get("reasoning_content"))
                                .and_then(|v| v.as_str())
                            {
                                reasoning.push_str(r);
                            }
                            if let Some(tcs) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                                for tc in tcs {
                                    let idx = tc.get("index").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let entry = tool_calls.entry(idx).or_insert_with(|| {
                                        (
                                            String::new(),
                                            "function".to_string(),
                                            String::new(),
                                            String::new(),
                                        )
                                    });
                                    if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                                        if entry.0.is_empty() {
                                            entry.0 = id.to_string();
                                        }
                                    }
                                    if let Some(t) = tc.get("type").and_then(|v| v.as_str()) {
                                        entry.1 = t.to_string();
                                    }
                                    if let Some(func) = tc.get("function") {
                                        if let Some(name) =
                                            func.get("name").and_then(|v| v.as_str())
                                        {
                                            if entry.2.is_empty() {
                                                entry.2 = name.to_string();
                                            }
                                        }
                                        if let Some(args) =
                                            func.get("arguments").and_then(|v| v.as_str())
                                        {
                                            entry.3.push_str(args);
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(fr) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                            finish_reason = Some(fr.to_string());
                        }
                    }
                }

                // Throttled reasoning send: every ~1.5 seconds
                if !reasoning.is_empty()
                    && last_reasoning_send.elapsed() >= std::time::Duration::from_millis(1500)
                {
                    let snippet = Self::format_reasoning_snippet(&reasoning);
                    let _ = reasoning_tx.try_send(snippet);
                    last_reasoning_send = std::time::Instant::now();
                }

                if stream_error.is_some() {
                    break;
                }
            }

            // Final reasoning send after stream ends (catch any unsent reasoning)
            if !reasoning.is_empty() {
                let snippet = Self::format_reasoning_snippet(&reasoning);
                let _ = reasoning_tx.try_send(snippet);
            }

            if let Some(err) = stream_error {
                last_error = err;
                if attempt < max_attempts {
                    tracing::warn!(
                        "chat_completion_streaming attempt {}/{} failed: {}",
                        attempt,
                        max_attempts,
                        last_error
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt as u64))
                        .await;
                    continue;
                }
                return Err(openai_api_rs::v1::error::APIError::CustomError {
                    message: last_error,
                });
            }

            if !has_data {
                last_error = "No data received from streaming response".to_string();
                if attempt < max_attempts {
                    tracing::warn!(
                        "chat_completion_streaming attempt {}/{}: no data",
                        attempt,
                        max_attempts
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt as u64))
                        .await;
                    continue;
                }
                return Err(openai_api_rs::v1::error::APIError::CustomError {
                    message: last_error,
                });
            }

            // Strip Kimi K2.5 tool-call markers that leak into content text
            if let Some(pos) = content.find("<|tool_calls_section_begin|>") {
                content.truncate(pos);
            }
            let content = content.trim().to_string();

            // Build assembled response JSON
            let message_content = if content.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!(content)
            };

            let mut message = serde_json::json!({
                "role": role,
                "content": message_content,
            });

            if !tool_calls.is_empty() {
                let tc_array: Vec<serde_json::Value> = tool_calls
                    .into_iter()
                    .map(|(_, (id, tc_type, name, args))| {
                        serde_json::json!({
                            "id": id,
                            "type": tc_type,
                            "function": {
                                "name": name,
                                "arguments": args
                            }
                        })
                    })
                    .collect();
                message
                    .as_object_mut()
                    .unwrap()
                    .insert("tool_calls".into(), serde_json::json!(tc_array));
            }

            let mut json = serde_json::json!({
                "id": if response_id.is_empty() { "chatcmpl-stream".to_string() } else { response_id },
                "object": "chat.completion",
                "created": 0,
                "model": if response_model.is_empty() { "unknown".to_string() } else { response_model },
                "choices": [{
                    "index": 0,
                    "message": message,
                    "finish_reason": finish_reason,
                }],
                "usage": {
                    "prompt_tokens": usage_prompt_tokens,
                    "completion_tokens": usage_completion_tokens,
                    "total_tokens": usage_total_tokens
                }
            });

            Self::inject_missing_fields(&mut json);

            // Log full reasoning for prompt improvement analysis
            if !reasoning.is_empty() {
                tracing::info!(
                    reasoning_chars = reasoning.len(),
                    "\n=== MODEL REASONING ({} chars) ===\n{}\n=== END REASONING ===",
                    reasoning.len(),
                    reasoning
                );
            }

            return serde_json::from_value(json).map_err(|e| {
                openai_api_rs::v1::error::APIError::CustomError {
                    message: format!("Failed to parse streaming response: {}", e),
                }
            });
        }

        Err(openai_api_rs::v1::error::APIError::CustomError {
            message: format!("All {} attempts failed: {}", max_attempts, last_error),
        })
    }
}

/// Fire-and-forget helper to log LLM usage from a ChatCompletionResponse.
/// Extracts token counts from the response's `usage` field and writes to the repository.
/// Logs a warning on failure but never panics.
pub fn log_llm_usage(
    repo: &std::sync::Arc<crate::repositories::llm_usage_repository::LlmUsageRepository>,
    user_id: i32,
    provider: &str,
    model: &str,
    callsite: &str,
    response: &openai_api_rs::v1::chat_completion::ChatCompletionResponse,
) {
    let u = &response.usage;
    let (pt, ct, tt) = (u.prompt_tokens, u.completion_tokens, u.total_tokens);

    if let Err(e) = repo.log_usage(user_id, provider, model, callsite, pt, ct, tt) {
        tracing::warn!("Failed to log LLM usage for callsite {}: {}", callsite, e);
    }
}
