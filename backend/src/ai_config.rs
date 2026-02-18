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
    /// Currently forced to OpenRouter - Tinfoil models have unreliable tool calling.
    /// Tinfoil preference is ignored until their models properly support function calling.
    pub fn provider_for_user_with_preference(&self, _preference: Option<&str>) -> AiProvider {
        AiProvider::OpenRouter
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
    /// Tinfoil: disabled for now - tool calling unreliable on available models
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
    /// This bypasses the openai-api-rs library's response parsing which requires
    /// fields that some providers (e.g. Tinfoil) don't return.
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

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(openai_api_rs::v1::error::APIError::CustomError {
                message: format!("{}: {}", status, text),
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
                return Err(openai_api_rs::v1::error::APIError::CustomError {
                    message: format!("Provider error ({}): {}", err_type, msg),
                });
            }
        }

        // Parse with tolerance for missing optional fields
        // Some providers (Tinfoil) omit "object" which openai-api-rs requires
        let mut json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
            openai_api_rs::v1::error::APIError::CustomError {
                message: format!("Failed to parse JSON: {} / response {}", e, text),
            }
        })?;

        // Inject missing fields that openai-api-rs v5.2.7 requires as non-optional
        // Tinfoil omits several fields (object, created, model, usage)
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

        serde_json::from_value(json).map_err(|e| openai_api_rs::v1::error::APIError::CustomError {
            message: format!("Failed to parse response: {}", e),
        })
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
