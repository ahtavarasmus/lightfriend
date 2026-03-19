//! Centralized AI provider configuration
//!
//! Tinfoil is the sole AI provider for all users.
//! OpenRouter code is kept as dead-code fallback but never selected.

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
}

/// Centralized AI configuration
#[derive(Debug, Clone)]
pub struct AiConfig {
    // OpenRouter key kept as optional fallback (only needed if OpenRouter is used directly)
    openrouter_api_key: Option<String>,
    tinfoil_api_key: Option<String>,
}

impl AiConfig {
    /// Create a minimal AiConfig for tests (no actual API calls will be made)
    pub fn default_for_tests() -> Self {
        Self {
            openrouter_api_key: Some("test_openrouter_key".to_string()),
            tinfoil_api_key: Some("test_tinfoil_key".to_string()),
        }
    }

    pub fn from_env() -> Self {
        let openrouter_api_key = std::env::var("OPENROUTER_API_KEY").ok();

        let tinfoil_api_key =
            Some(std::env::var("TINFOIL_API_KEY").expect("TINFOIL_API_KEY required"));

        tracing::info!("AI config initialized: Tinfoil (primary)");

        Self {
            openrouter_api_key,
            tinfoil_api_key,
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
            AiProvider::OpenRouter => "https://openrouter.ai/api/v1",
            AiProvider::Tinfoil => "https://inference.tinfoil.sh/v1",
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
        }
    }

    /// Get the model name for a provider and purpose
    pub fn model(&self, provider: AiProvider, purpose: ModelPurpose) -> &str {
        match (provider, purpose) {
            (AiProvider::OpenRouter, ModelPurpose::Default) => "openai/gpt-4o-2024-11-20",
            (AiProvider::Tinfoil, ModelPurpose::Default) => "kimi-k2-5",
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
                // Sanitize message content for Tinfoil
                Self::sanitize_request_body(&mut body);
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
                "prompt_tokens": 0,
                "completion_tokens": 0,
                "total_tokens": 0
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
        // Fast path: no reasoning channel -> reuse existing method unchanged
        let reasoning_tx = match reasoning_tx {
            Some(tx) => tx,
            None => return self.chat_completion(provider, request).await,
        };

        use futures::StreamExt;

        let url = format!("{}/chat/completions", self.endpoint(provider));
        let api_key = self.api_key(provider);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let mut last_error = String::new();
        for attempt in 1..=3u32 {
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
                if attempt < 3 {
                    tracing::warn!(
                        "chat_completion_streaming attempt {}/3 failed: {}",
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
                if attempt < 3 {
                    tracing::warn!(
                        "chat_completion_streaming attempt {}/3 failed: {}",
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

            if !has_data {
                last_error = "No data received from streaming response".to_string();
                if attempt < 3 {
                    tracing::warn!("chat_completion_streaming attempt {}/3: no data", attempt,);
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
                    "prompt_tokens": 0,
                    "completion_tokens": 0,
                    "total_tokens": 0
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
            message: format!("All 3 attempts failed: {}", last_error),
        })
    }
}
