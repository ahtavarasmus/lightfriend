use backend::{AiConfig, AiProvider, ModelPurpose};

#[test]
fn tinfoil_voice_model_uses_gemma4() {
    let config = AiConfig::default_for_tests();

    assert_eq!(
        config.model(AiProvider::Tinfoil, ModelPurpose::Voice),
        "gemma4-31b"
    );
}
