use backend::services::usage_pricing::{
    billable_customer_cost_usd, openai_realtime_cost_usd, openai_realtime_usage_from_event,
    text_llm_cost_usd, RealtimeTokenUsage,
};
use backend::AiProvider;

#[test]
fn tinfoil_kimi_cost_uses_input_and_output_rates() {
    let cost = text_llm_cost_usd(AiProvider::Tinfoil, "kimi-k2-6", 38_532, 1_235);
    assert!((cost - 0.06428175).abs() < 0.00000001);
    assert!((billable_customer_cost_usd(cost) - 0.083566275).abs() < 0.00000001);
}

#[test]
fn realtime_cost_separates_cached_audio_and_text_tokens() {
    let cost = openai_realtime_cost_usd(RealtimeTokenUsage {
        input_text_tokens: 1_000,
        input_audio_tokens: 6_000,
        cached_input_text_tokens: 200,
        cached_input_audio_tokens: 1_000,
        output_text_tokens: 500,
        output_audio_tokens: 4_000,
    });
    assert!((cost - 0.43168).abs() < 0.00000001);
}

#[test]
fn realtime_response_done_parser_reads_detailed_usage() {
    let event = serde_json::json!({
        "type": "response.done",
        "response": {
            "id": "resp_123",
            "usage": {
                "input_token_details": {
                    "text_tokens": 1000,
                    "audio_tokens": 6000,
                    "cached_tokens": 1200,
                    "cached_tokens_details": {
                        "text_tokens": 200,
                        "audio_tokens": 1000
                    }
                },
                "output_token_details": {
                    "text_tokens": 500,
                    "audio_tokens": 4000
                }
            }
        }
    });

    let (response_id, usage) = openai_realtime_usage_from_event(&event).unwrap();
    assert_eq!(response_id, "resp_123");
    assert_eq!(usage.cached_input_audio_tokens, 1_000);
    assert_eq!(usage.output_audio_tokens, 4_000);
}
