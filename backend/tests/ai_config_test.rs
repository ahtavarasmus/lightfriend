use backend::{AiConfig, AiProvider, ModelPurpose};

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
