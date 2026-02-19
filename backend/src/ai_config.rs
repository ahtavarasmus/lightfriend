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
            tinfoil_api_key: Some("test_tinfoil_key".to_string()),
        }
    }

    pub fn from_env() -> Self {
        let openrouter_api_key =
            std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY required");

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
    /// Tinfoil: kimi-k2-5 for everything (supports vision + tools in single call)
    pub fn model(&self, provider: AiProvider, purpose: ModelPurpose) -> &str {
        match (provider, purpose) {
            // OpenRouter models
            (AiProvider::OpenRouter, ModelPurpose::Default) => "openai/gpt-4o-2024-11-20",
            (AiProvider::OpenRouter, ModelPurpose::VisionOnly) => "openai/gpt-4o-2024-11-20",

            // Tinfoil: single model for all purposes
            (AiProvider::Tinfoil, _) => "kimi-k2-5",
        }
    }

    /// Check if a provider needs two-step vision processing.
    /// Both providers now support vision + tools in a single call.
    pub fn needs_two_step_vision(&self, _provider: AiProvider) -> bool {
        false
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

        let response = client.chat_completion(request).await?;

        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_else(|| "Unable to describe image".to_string());

        Ok(content)
    }
}
