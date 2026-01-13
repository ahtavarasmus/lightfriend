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
        let openrouter_api_key = std::env::var("OPENROUTER_API_KEY")
            .expect("OPENROUTER_API_KEY required");

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
            AiProvider::Tinfoil => self.tinfoil_api_key.as_ref()
                .expect("Tinfoil API key not configured"),
        }
    }

    /// Get the model name for a provider and purpose
    ///
    /// OpenRouter: GPT-4o for everything (supports vision + tools)
    /// Tinfoil:
    ///   - Default: qwen3-coder-480b (tools, no vision)
    ///   - VisionOnly: qwen3-vl-30b (vision, no tools)
    pub fn model(&self, provider: AiProvider, purpose: ModelPurpose) -> &str {
        match (provider, purpose) {
            // OpenRouter models
            (AiProvider::OpenRouter, ModelPurpose::Default) => "openai/gpt-4o-2024-11-20",
            (AiProvider::OpenRouter, ModelPurpose::VisionOnly) => "openai/gpt-4o-2024-11-20",

            // Tinfoil models
            (AiProvider::Tinfoil, ModelPurpose::Default) => "qwen3-coder-480b",
            (AiProvider::Tinfoil, ModelPurpose::VisionOnly) => "qwen3-vl-30b",
        }
    }

    /// Check if a provider needs two-step vision processing
    /// (i.e., vision model can't do tool calling)
    pub fn needs_two_step_vision(&self, provider: AiProvider) -> bool {
        match provider {
            AiProvider::OpenRouter => false,  // GPT-4o handles vision + tools together
            AiProvider::Tinfoil => true,      // Must describe image first, then tool-call
        }
    }

    /// Create an OpenAI-compatible client for a specific provider
    pub fn create_client(&self, provider: AiProvider) -> Result<OpenAIClient, Box<dyn std::error::Error>> {
        OpenAIClient::builder()
            .with_endpoint(self.endpoint(provider))
            .with_api_key(self.api_key(provider))
            .build()
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
            ChatCompletionMessage, ChatCompletionRequest, Content,
            ImageUrl, ImageUrlType, ContentType, MessageRole,
        };

        let client = self.create_client(provider)?;
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

        let messages = vec![
            ChatCompletionMessage {
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
            },
        ];

        let request = ChatCompletionRequest::new(model.to_string(), messages)
            .max_tokens(500);

        let response = client.chat_completion(request).await?;

        let content = response.choices.first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_else(|| "Unable to describe image".to_string());

        Ok(content)
    }
}
