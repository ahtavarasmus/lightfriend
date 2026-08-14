use backend::{AiConfig, AiProvider, ModelPurpose};
use openai_api_rs::v1::chat_completion;

#[test]
fn tinfoil_default_model_uses_deepseek_v4_flash() {
    let config = AiConfig::default_for_tests();

    assert_eq!(
        config.model(AiProvider::Tinfoil, ModelPurpose::Default),
        "deepseek-v4-flash"
    );
    assert_eq!(
        AiConfig::reasoning_effort_for_model("deepseek-v4-flash"),
        Some("medium")
    );
}

#[test]
fn tinfoil_voice_model_uses_gemma4() {
    let config = AiConfig::default_for_tests();

    assert_eq!(
        config.model(AiProvider::Tinfoil, ModelPurpose::Voice),
        "gemma4-31b"
    );
}

#[test]
fn near_defaults_are_configured_for_tests() {
    let config = AiConfig::default_for_tests();

    assert_eq!(
        config.model(AiProvider::Near, ModelPurpose::Default),
        "zai-org/GLM-5.1-FP8"
    );
    assert_eq!(
        config.model(AiProvider::Near, ModelPurpose::Voice),
        "google/gemma-4-31B-it"
    );
    assert_eq!(
        config.endpoint(AiProvider::Near),
        "https://cloud-api.near.ai/v1"
    );
}

#[test]
fn image_requests_use_vision_capable_models() {
    let config = AiConfig::default_for_tests();
    let request = chat_completion::ChatCompletionRequest::new(
        String::new(),
        vec![chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::ImageUrl(vec![chat_completion::ImageUrl {
                r#type: chat_completion::ContentType::image_url,
                text: None,
                image_url: Some(chat_completion::ImageUrlType {
                    url: "data:image/png;base64,test".to_string(),
                }),
            }]),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }],
    );

    assert_eq!(
        config.model_for_request(AiProvider::Tinfoil, ModelPurpose::Default, &request),
        "gemma4-31b"
    );
    assert_eq!(
        config.model_for_request(AiProvider::Near, ModelPurpose::Default, &request),
        "google/gemma-4-31B-it"
    );
}
