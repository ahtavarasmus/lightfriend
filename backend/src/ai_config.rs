//! Centralized AI provider configuration
//!
//! Users can choose their preferred provider in settings:
//! - "openai" (default): Uses OpenRouter with GPT-4o (faster, smarter)
//! - "tinfoil": Uses Tinfoil for privacy-focused LLM (slower but private)

use openai_api_rs::v1::api::OpenAIClient;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AiProvider {
    OpenRouter,
    Tinfoil,
}

#[derive(Debug, Clone, Copy)]
pub enum ModelPurpose {
    /// Normal conversation/tasks with tool calling
    Default,
    /// Vision-only model (no tool calling) - used for Tinfoil two-step
    VisionOnly,
}

/// Centralized AI configuration
#[derive(Debug, Clone)]
pub struct AiConfig {
    // We keep OpenRouter key always available as fallback
    openrouter_api_key: String,
    tinfoil_api_key: Option<String>,
}

impl AiConfig {
    /// Create a minimal AiConfig for tests (no actual API calls will be made)
    pub fn default_for_tests() -> Self {
        Self {
            openrouter_api_key: "test_openrouter_key".to_string(),
            tinfoil_api_key: None,
        }
    }

    pub fn from_env() -> Self {
        let openrouter_api_key =
            std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY required");

        let tinfoil_api_key = std::env::var("TINFOIL_API_KEY").ok();

        if tinfoil_api_key.is_some() {
            tracing::info!("AI config initialized: OpenRouter + Tinfoil available");
        } else {
            tracing::info!("AI config initialized: OpenRouter only");
        }

        Self {
            openrouter_api_key,
            tinfoil_api_key,
        }
    }

    /// Determine which provider to use based on user's preference setting
    ///
    /// Determine which provider to use based on user's preference setting.
    ///
    /// preference: The user's llm_provider setting from the database
    /// - Some("tinfoil") -> use Tinfoil
    /// - Some("openai") or None -> use OpenRouter (default)
    pub fn provider_for_user_with_preference(&self, preference: Option<&str>) -> AiProvider {
        match preference {
            Some("tinfoil") if self.tinfoil_api_key.is_some() => AiProvider::Tinfoil,
            _ => AiProvider::OpenRouter,
        }
    }

    /// Get the endpoint URL for a provider
    pub fn endpoint(&self, provider: AiProvider) -> &str {
        match provider {
            AiProvider::OpenRouter => "https://openrouter.ai/api/v1",
            AiProvider::Tinfoil => "https://inference.tinfoil.sh/v1",
        }
    }

    /// Get the API key for a provider
    pub fn api_key(&self, provider: AiProvider) -> &str {
        match provider {
            AiProvider::OpenRouter => &self.openrouter_api_key,
            AiProvider::Tinfoil => self
                .tinfoil_api_key
                .as_ref()
                .expect("Tinfoil API key not configured"),
        }
    }

    /// Get the model name for a provider and purpose
    ///
    /// OpenRouter: GPT-4o for everything (supports vision + tools)
    /// Tinfoil: kimi-k2-5 for tool calling (via streaming), qwen3-vl-30b for vision
    pub fn model(&self, provider: AiProvider, purpose: ModelPurpose) -> &str {
        match (provider, purpose) {
            // OpenRouter models
            (AiProvider::OpenRouter, ModelPurpose::Default) => "openai/gpt-4o-2024-11-20",
            (AiProvider::OpenRouter, ModelPurpose::VisionOnly) => "openai/gpt-4o-2024-11-20",

            // Tinfoil models (kept for future use when tool calling is reliable)
            (AiProvider::Tinfoil, ModelPurpose::Default) => "kimi-k2-5",
            (AiProvider::Tinfoil, ModelPurpose::VisionOnly) => "qwen3-vl-30b",
        }
    }

    /// Check if a provider needs two-step vision processing
    /// (i.e., vision model can't do tool calling)
    pub fn needs_two_step_vision(&self, provider: AiProvider) -> bool {
        match provider {
            AiProvider::OpenRouter => false, // GPT-4o handles vision + tools together
            AiProvider::Tinfoil => true,     // Must describe image first, then tool-call
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
        for attempt in 1..=3u32 {
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
                        if attempt < 3 {
                            tracing::warn!(
                                "chat_completion streaming attempt {}/3 failed: {}",
                                attempt,
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
                if attempt < 3 {
                    tracing::warn!(
                        "chat_completion attempt {}/3 failed: {}",
                        attempt,
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
                    if attempt < 3 {
                        tracing::warn!(
                            "chat_completion attempt {}/3 got error body: {}",
                            attempt,
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
            message: format!("All 3 attempts failed: {}", last_error),
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
                "prompt_tokens": 0,
                "completion_tokens": 0,
                "total_tokens": 0
            }
        });

        Self::inject_missing_fields(&mut json);

        serde_json::from_value(json)
            .map_err(|e| format!("Failed to deserialize assembled response: {}", e))
    }

    /// Call vision model to describe an image (for two-step processing)
    /// Returns a text description of the image
    pub async fn describe_image(
        &self,
        provider: AiProvider,
        image_url: &str,
        user_text: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        use openai_api_rs::v1::chat_completion::{
            ChatCompletionMessage, ChatCompletionRequest, Content, ContentType, ImageUrl,
            ImageUrlType, MessageRole,
        };

        let model = self.model(provider, ModelPurpose::VisionOnly);

        // Build prompt based on whether user provided text
        let prompt = if user_text.trim().is_empty() {
            "Describe this image in detail. What do you see?".to_string()
        } else {
            format!(
                "The user asks: '{}'\n\nLook at this image and provide information to help answer their question.",
                user_text
            )
        };

        let messages = vec![ChatCompletionMessage {
            role: MessageRole::user,
            content: Content::ImageUrl(vec![
                ImageUrl {
                    r#type: ContentType::text,
                    text: Some(prompt),
                    image_url: None,
                },
                ImageUrl {
                    r#type: ContentType::image_url,
                    text: None,
                    image_url: Some(ImageUrlType {
                        url: image_url.to_string(),
                    }),
                },
            ]),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }];

        let request = ChatCompletionRequest::new(model.to_string(), messages).max_tokens(500);

        let response = self.chat_completion(provider, &request).await?;

        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_else(|| "Unable to describe image".to_string());

        Ok(content)
    }
}
