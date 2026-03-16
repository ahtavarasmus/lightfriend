//! Centralized AI provider configuration
//!
//! Provider priority: Tinfoil > Anthropic > OpenRouter.
//! Anthropic uses a different API format (Messages API) which is converted
//! from/to the OpenAI-compatible format used internally.

use openai_api_rs::v1::api::OpenAIClient;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AiProvider {
    OpenRouter,
    Tinfoil,
    Anthropic,
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
    anthropic_api_key: Option<String>,
}

impl AiConfig {
    /// Create a minimal AiConfig for tests (no actual API calls will be made)
    pub fn default_for_tests() -> Self {
        Self {
            openrouter_api_key: Some("test_openrouter_key".to_string()),
            tinfoil_api_key: Some("test_tinfoil_key".to_string()),
            anthropic_api_key: Some("test_anthropic_key".to_string()),
        }
    }

    pub fn from_env() -> Self {
        let openrouter_api_key = std::env::var("OPENROUTER_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());
        let tinfoil_api_key = std::env::var("TINFOIL_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());
        let anthropic_api_key = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());

        let providers: Vec<&str> = [
            tinfoil_api_key.as_ref().map(|_| "Tinfoil"),
            anthropic_api_key.as_ref().map(|_| "Anthropic"),
            openrouter_api_key.as_ref().map(|_| "OpenRouter"),
        ]
        .into_iter()
        .flatten()
        .collect();

        if providers.is_empty() {
            panic!("At least one AI provider API key must be set (TINFOIL_API_KEY, ANTHROPIC_API_KEY, or OPENROUTER_API_KEY)");
        }

        tracing::info!(
            "AI config initialized: {} (priority order)",
            providers.join(" > ")
        );

        Self {
            openrouter_api_key,
            tinfoil_api_key,
            anthropic_api_key,
        }
    }

    /// Select provider by priority: Tinfoil > Anthropic > OpenRouter.
    /// The preference parameter is accepted for backward compatibility but ignored.
    pub fn provider_for_user_with_preference(&self, _preference: Option<&str>) -> AiProvider {
        if self.tinfoil_api_key.is_some() {
            AiProvider::Tinfoil
        } else if self.anthropic_api_key.is_some() {
            AiProvider::Anthropic
        } else {
            AiProvider::OpenRouter
        }
    }

    /// Get the endpoint URL for a provider
    pub fn endpoint(&self, provider: AiProvider) -> &str {
        match provider {
            AiProvider::OpenRouter => "https://api.groq.com/openai/v1",
            AiProvider::Tinfoil => "https://inference.tinfoil.sh/v1",
            AiProvider::Anthropic => "https://api.anthropic.com/v1",
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
            AiProvider::Anthropic => self
                .anthropic_api_key
                .as_ref()
                .expect("ANTHROPIC_API_KEY not set but Anthropic provider requested"),
        }
    }

    /// Get the model name for a provider and purpose
    pub fn model(&self, provider: AiProvider, purpose: ModelPurpose) -> &str {
        match (provider, purpose) {
            (AiProvider::OpenRouter, ModelPurpose::Default) => "llama-3.1-8b-instant",
            (AiProvider::Tinfoil, ModelPurpose::Default) => "llama3-3-70b",
            (AiProvider::Anthropic, ModelPurpose::Default) => "claude-sonnet-4-20250514",
        }
    }

    /// Create an OpenAI-compatible client for a specific provider.
    /// Not supported for Anthropic (uses custom HTTP path instead).
    pub fn create_client(
        &self,
        provider: AiProvider,
    ) -> Result<OpenAIClient, Box<dyn std::error::Error>> {
        if provider == AiProvider::Anthropic {
            // Return a dummy OpenAI client for Anthropic.
            // Actual API calls go through ai_config.chat_completion() which
            // handles Anthropic natively. This client is only needed to satisfy
            // AgentContext's type requirement.
            return OpenAIClient::builder()
                .with_endpoint("https://api.anthropic.com/v1")
                .with_api_key(self.api_key(provider))
                .build();
        }
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

    /// Convert an OpenAI-format request body to Anthropic Messages API format.
    fn convert_openai_to_anthropic_request(
        openai_body: &serde_json::Value,
        model: &str,
    ) -> serde_json::Value {
        let max_tokens = openai_body
            .get("max_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(4096);

        let mut anthropic = serde_json::json!({
            "model": model,
            "max_tokens": max_tokens,
            "stream": true,
        });

        // Extract system messages and convert the rest
        let mut system_parts: Vec<String> = Vec::new();
        let mut messages: Vec<serde_json::Value> = Vec::new();

        if let Some(msgs) = openai_body.get("messages").and_then(|m| m.as_array()) {
            for msg in msgs {
                let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
                match role {
                    "system" => {
                        if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                            system_parts.push(content.to_string());
                        }
                    }
                    "assistant" => {
                        let mut content_blocks: Vec<serde_json::Value> = Vec::new();

                        // Add text content if present
                        if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
                            if !text.is_empty() {
                                content_blocks.push(serde_json::json!({
                                    "type": "text",
                                    "text": text
                                }));
                            }
                        }

                        // Convert tool_calls to tool_use content blocks
                        if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array())
                        {
                            for tc in tool_calls {
                                let id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                let name = tc
                                    .get("function")
                                    .and_then(|f| f.get("name"))
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("");
                                let args_str = tc
                                    .get("function")
                                    .and_then(|f| f.get("arguments"))
                                    .and_then(|a| a.as_str())
                                    .unwrap_or("{}");
                                let input: serde_json::Value =
                                    serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));

                                content_blocks.push(serde_json::json!({
                                    "type": "tool_use",
                                    "id": id,
                                    "name": name,
                                    "input": input
                                }));
                            }
                        }

                        if content_blocks.is_empty() {
                            content_blocks.push(serde_json::json!({
                                "type": "text",
                                "text": ""
                            }));
                        }

                        messages.push(serde_json::json!({
                            "role": "assistant",
                            "content": content_blocks
                        }));
                    }
                    "tool" => {
                        // Anthropic requires tool_result in a user message
                        let tool_use_id = msg
                            .get("tool_call_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");

                        let tool_result = serde_json::json!({
                            "type": "tool_result",
                            "tool_use_id": tool_use_id,
                            "content": content
                        });

                        // Merge with previous user message if it only contains tool_results
                        let merged = if let Some(last) = messages.last_mut() {
                            if last.get("role").and_then(|r| r.as_str()) == Some("user") {
                                if let Some(content_arr) =
                                    last.get_mut("content").and_then(|c| c.as_array_mut())
                                {
                                    if content_arr.iter().all(|b| {
                                        b.get("type").and_then(|t| t.as_str())
                                            == Some("tool_result")
                                    }) {
                                        content_arr.push(tool_result.clone());
                                        true
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        if !merged {
                            messages.push(serde_json::json!({
                                "role": "user",
                                "content": [tool_result]
                            }));
                        }
                    }
                    "user" => {
                        let content = msg.get("content").cloned().unwrap_or(serde_json::json!(""));
                        messages.push(serde_json::json!({
                            "role": "user",
                            "content": content
                        }));
                    }
                    _ => {}
                }
            }
        }

        if !system_parts.is_empty() {
            anthropic["system"] = serde_json::json!(system_parts.join("\n\n"));
        }
        anthropic["messages"] = serde_json::json!(messages);

        // Convert tools
        if let Some(tools) = openai_body.get("tools").and_then(|t| t.as_array()) {
            let anthropic_tools: Vec<serde_json::Value> = tools
                .iter()
                .filter_map(|tool| {
                    let func = tool.get("function")?;
                    let name = func.get("name")?;
                    let description = func
                        .get("description")
                        .cloned()
                        .unwrap_or(serde_json::json!(""));
                    let parameters = func
                        .get("parameters")
                        .cloned()
                        .unwrap_or(serde_json::json!({"type": "object"}));

                    Some(serde_json::json!({
                        "name": name,
                        "description": description,
                        "input_schema": parameters
                    }))
                })
                .collect();

            if !anthropic_tools.is_empty() {
                anthropic["tools"] = serde_json::json!(anthropic_tools);
            }
        }

        // Convert tool_choice
        if let Some(tc) = openai_body.get("tool_choice") {
            if let Some(tc_str) = tc.as_str() {
                match tc_str {
                    "required" => {
                        anthropic["tool_choice"] = serde_json::json!({"type": "any"});
                    }
                    "auto" => {
                        anthropic["tool_choice"] = serde_json::json!({"type": "auto"});
                    }
                    _ => {}
                }
            } else if tc.is_object() {
                if let Some(name) = tc
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                {
                    anthropic["tool_choice"] = serde_json::json!({"type": "tool", "name": name});
                }
            }
        }

        anthropic
    }

    /// Assemble a ChatCompletionResponse from Anthropic SSE streaming text.
    fn assemble_anthropic_sse_response(
        sse_text: &str,
    ) -> Result<openai_api_rs::v1::chat_completion::ChatCompletionResponse, String> {
        let mut response_id = String::new();
        let mut response_model = String::new();
        let mut content = String::new();
        let mut finish_reason: Option<String> = None;
        // tool_calls: block_index -> (id, name, arguments_json)
        let mut tool_calls: std::collections::BTreeMap<u64, (String, String, String)> =
            std::collections::BTreeMap::new();
        let mut has_data = false;

        for line in sse_text.lines() {
            let line = line.trim();
            let data = match line.strip_prefix("data: ") {
                Some(d) => d,
                None => continue,
            };

            let event: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            has_data = true;

            let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

            match event_type {
                "message_start" => {
                    if let Some(msg) = event.get("message") {
                        if let Some(id) = msg.get("id").and_then(|v| v.as_str()) {
                            response_id = id.to_string();
                        }
                        if let Some(model) = msg.get("model").and_then(|v| v.as_str()) {
                            response_model = model.to_string();
                        }
                    }
                }
                "content_block_start" => {
                    let index = event.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
                    if let Some(block) = event.get("content_block") {
                        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        if block_type == "tool_use" {
                            let id = block
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let name = block
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            tool_calls.insert(index, (id, name, String::new()));
                        }
                    }
                }
                "content_block_delta" => {
                    let index = event.get("index").and_then(|i| i.as_u64()).unwrap_or(0);
                    if let Some(delta) = event.get("delta") {
                        let delta_type = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        match delta_type {
                            "text_delta" => {
                                if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                    content.push_str(text);
                                }
                            }
                            "input_json_delta" => {
                                if let Some(json) =
                                    delta.get("partial_json").and_then(|j| j.as_str())
                                {
                                    if let Some(tc) = tool_calls.get_mut(&index) {
                                        tc.2.push_str(json);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "message_delta" => {
                    if let Some(delta) = event.get("delta") {
                        if let Some(reason) = delta.get("stop_reason").and_then(|r| r.as_str()) {
                            finish_reason = Some(match reason {
                                "tool_use" => "tool_calls".to_string(),
                                "end_turn" => "stop".to_string(),
                                "max_tokens" => "length".to_string(),
                                other => other.to_string(),
                            });
                        }
                    }
                }
                "error" => {
                    let err_msg = event
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown error");
                    return Err(format!("Anthropic API error: {}", err_msg));
                }
                _ => {}
            }
        }

        if !has_data {
            return Err("No data received from Anthropic streaming response".to_string());
        }

        let content = content.trim().to_string();
        let message_content = if content.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::json!(content)
        };

        let mut message = serde_json::json!({
            "role": "assistant",
            "content": message_content,
        });

        if !tool_calls.is_empty() {
            let tc_array: Vec<serde_json::Value> = tool_calls
                .into_iter()
                .map(|(_, (id, name, args))| {
                    serde_json::json!({
                        "id": id,
                        "type": "function",
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
            "id": if response_id.is_empty() { "msg-anthropic".to_string() } else { response_id },
            "object": "chat.completion",
            "created": 0,
            "model": if response_model.is_empty() { "claude-sonnet-4-20250514".to_string() } else { response_model },
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
            .map_err(|e| format!("Failed to deserialize Anthropic response: {}", e))
    }

    /// Anthropic-specific chat completion: converts request, calls Messages API,
    /// parses SSE response. Retries up to 3 times.
    async fn anthropic_chat_completion(
        &self,
        request: &openai_api_rs::v1::chat_completion::ChatCompletionRequest,
    ) -> Result<
        openai_api_rs::v1::chat_completion::ChatCompletionResponse,
        openai_api_rs::v1::error::APIError,
    > {
        let anthropic_url = format!("{}/messages", self.endpoint(AiProvider::Anthropic));
        let api_key = self.api_key(AiProvider::Anthropic);
        let model = self.model(AiProvider::Anthropic, ModelPurpose::Default);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let openai_body = serde_json::to_value(request).map_err(|e| {
            openai_api_rs::v1::error::APIError::CustomError {
                message: format!("Failed to serialize request: {}", e),
            }
        })?;
        let anthropic_body = Self::convert_openai_to_anthropic_request(&openai_body, model);

        tracing::debug!(
            "Anthropic request: {}",
            serde_json::to_string_pretty(&anthropic_body).unwrap_or_default()
        );

        let mut last_error = String::new();
        for attempt in 1..=3u32 {
            let response = client
                .post(&anthropic_url)
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&anthropic_body)
                .send()
                .await?;

            let status = response.status();
            let text = response.text().await.unwrap_or_default();

            if !status.is_success() {
                last_error = format!("{}: {}", status, text);
                if attempt < 3 {
                    tracing::warn!("Anthropic attempt {}/3 failed: {}", attempt, last_error);
                    tokio::time::sleep(tokio::time::Duration::from_millis(500 * attempt as u64))
                        .await;
                    continue;
                }
                return Err(openai_api_rs::v1::error::APIError::CustomError {
                    message: last_error,
                });
            }

            match Self::assemble_anthropic_sse_response(&text) {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = format!("Anthropic SSE error: {}", e);
                    if attempt < 3 {
                        tracing::warn!("Anthropic attempt {}/3 SSE error: {}", attempt, last_error);
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

        Err(openai_api_rs::v1::error::APIError::CustomError {
            message: format!("All 3 Anthropic attempts failed: {}", last_error),
        })
    }

    /// Make a chat completion request directly via reqwest.
    /// For Tinfoil: uses streaming (SSE) to avoid a bug where non-streaming + tools + long
    /// content fails. SSE chunks are collected and reassembled into a normal response.
    /// For Anthropic: handled separately via anthropic_chat_completion().
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
        // Anthropic uses a completely different API format
        if provider == AiProvider::Anthropic {
            return self.anthropic_chat_completion(request).await;
        }

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
        // Anthropic doesn't expose reasoning tokens; use the non-streaming path
        // which internally streams and assembles the response
        if provider == AiProvider::Anthropic {
            return self.chat_completion(provider, request).await;
        }

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
