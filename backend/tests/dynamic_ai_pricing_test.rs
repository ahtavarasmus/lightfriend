use backend::services::{
    ai_usage::{merge_stream_usage, TokenUsage},
    ai_usage_report::build_report,
    model_pricing::{parse_catalog, refresh_provider_prices, PricingCatalog},
};
use backend::{AiConfig, AiProvider, LlmUsageRepository};
use serde_json::json;

#[test]
fn catalogs_normalize_units_and_keep_free_models_but_reject_invalid_prices() {
    let tinfoil = parse_catalog(
        "tinfoil",
        &json!({"data": [
            {"id":"new-model", "pricing":{"inputTokenPricePer1M":0.3,"outputTokenPricePer1M":0.7,"cachedInputTokenPricePer1M":0.06}},
            {"id":"free", "pricing":{"inputTokenPricePer1M":0,"outputTokenPricePer1M":0}},
            {"id":"bad", "pricing":{"inputTokenPricePer1M":-1,"outputTokenPricePer1M":1}},
            {"id":"missing", "pricing":{"inputTokenPricePer1M":1}}
        ]}),
        100,
    );
    assert_eq!(tinfoil.len(), 2);
    assert_eq!(tinfoil[0].1.rates.cached_input_per_million, Some(0.06));
    assert_eq!(tinfoil[1].1.rates.input_per_million, 0.0);
    for provider in ["near", "openrouter"] {
        let prices = parse_catalog(
            provider,
            &json!({"data":[{"id":"model", "pricing":{
                "prompt":"0.0000014", "completion":"0.0000044", "input_cache_read":"0.00000026", "request":"0.002"
            }}]}),
            100,
        );
        assert!((prices[0].1.rates.input_per_million - 1.4).abs() < 1e-10);
        assert_eq!(prices[0].1.rates.output_per_million, 4.4);
        assert_eq!(prices[0].1.rates.request_usd, 0.002);
    }
}

#[test]
fn cache_discount_search_fees_and_reported_cost_are_not_double_counted() {
    let catalog = PricingCatalog::default();
    let usage: TokenUsage = serde_json::from_value(json!({"prompt_tokens":1_000_000,
        "completion_tokens":100_000, "prompt_tokens_details":{"cached_tokens":800_000}}))
    .unwrap();
    let cost = catalog.cost("tinfoil", "deepseek-v4-1-flash", &usage);
    assert!((cost.provider_cost_usd - 0.178).abs() < 1e-10);
    assert!(cost.usage_complete);
    let search = catalog.cost(
        "openrouter",
        "perplexity/sonar-reasoning-pro",
        &TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 1000,
            ..Default::default()
        },
    );
    assert!((search.provider_cost_usd - 0.015).abs() < 1e-10);
    let reported = catalog.cost(
        "openrouter",
        "perplexity/sonar-reasoning-pro",
        &TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 1000,
            cost: Some(0.012),
            ..Default::default()
        },
    );
    assert_eq!(reported.provider_cost_usd, 0.012);
    assert!(reported.reported_cost);
    assert!(reported.usage_complete);
    let unknown = catalog.cost("tinfoil", "new-model-without-price", &usage);
    assert_eq!(unknown.quote.source, "default_provider");
    assert!(unknown.provider_cost_usd > 0.0);
}

#[test]
fn streamed_usage_preserves_caching_and_cost_without_summing_cumulative_counts() {
    let mut usage = json!({});
    merge_stream_usage(
        &mut usage,
        &json!({"prompt_tokens":100,"completion_tokens":10,
        "prompt_tokens_details":{"cached_tokens":80}, "cost":0.002}),
    );
    merge_stream_usage(
        &mut usage,
        &json!({"prompt_tokens":100,"completion_tokens":20,"total_tokens":120,"prompt_tokens_details":{"audio_tokens":0}}),
    );
    merge_stream_usage(&mut usage, &serde_json::Value::Null);
    let usage: TokenUsage = serde_json::from_value(usage).unwrap();
    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 20);
    assert_eq!(usage.cached_tokens(), Some(80));
    assert_eq!(usage.cost, Some(0.002));
}

fn historical_row(user_id: i32, now: i32) -> backend::pg_models::PgLlmUsageLog {
    backend::pg_models::PgLlmUsageLog {
        id: 1,
        user_id,
        provider: "tinfoil".into(),
        model: "gemma4-31b".into(),
        callsite: "urgency_classification".into(),
        prompt_tokens: 1_000_000,
        completion_tokens: 0,
        total_tokens: 1_000_000,
        created_at: now - 86400,
        cached_prompt_tokens: None,
        provider_cost_usd: None,
        customer_cost_usd: None,
        pricing_snapshot: None,
    }
}

#[test]
fn report_preserves_historical_snapshots_and_uses_the_entire_window() {
    let catalog = PricingCatalog::default();
    let now = 1_800_000_000;
    let mut saved = historical_row(1, now);
    let snapshot = catalog.cost(
        "tinfoil",
        "gemma4-31b",
        &TokenUsage {
            prompt_tokens: 1_000_000,
            ..Default::default()
        },
    );
    saved.pricing_snapshot = Some(serde_json::to_string(&snapshot).unwrap());
    saved.provider_cost_usd = Some(snapshot.provider_cost_usd);
    saved.customer_cost_usd = Some(snapshot.customer_cost_usd);
    let new = parse_catalog(
        "tinfoil",
        &json!({"data":[{"id":"gemma4-31b","pricing":{
        "inputTokenPricePer1M":2,"outputTokenPricePer1M":4}}]}),
        now,
    );
    catalog.insert("tinfoil", &new[0].0, new[0].1.clone());
    let mut outside = historical_row(3, now);
    outside.created_at = now; // half-open interval excludes future/current-second writes
    let report = build_report(&[saved, historical_row(2, now), outside], &catalog, 14, now);
    assert_eq!(report.total_calls, 2);
    assert_eq!(report.costs.historical_estimate_calls, 1);
    assert_eq!(report.costs.provider_cost_usd, 2.4);
    assert!((report.average_tokens_per_day - 2_000_000.0 / 14.0).abs() < 1e-10);
    assert_eq!(report.active_users, 2);
    let encoded = serde_json::to_string(&report).unwrap();
    let decoded: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded["total_tokens"], 2_000_000);
    assert_eq!(decoded["provider_cost_usd"], 2.4);
    assert_eq!(report.per_user[0].user_id, Some(2)); // sorted by cost
    assert_eq!(report.by_model[0].provider.as_deref(), Some("tinfoil"));
}

#[tokio::test]
async fn refresh_survives_outages_and_saved_prices_survive_restart() {
    use diesel::r2d2::{ConnectionManager, Pool};
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };
    let url = std::env::var("TEST_PG_DATABASE_URL")
        .expect("set TEST_PG_DATABASE_URL to an isolated migrated test database");
    let pool = Pool::builder()
        .max_size(2)
        .build(ConnectionManager::<diesel::PgConnection>::new(url))
        .unwrap();
    let repository = LlmUsageRepository::new(pool.clone());
    let server = MockServer::start().await;
    let mut config =
        AiConfig::default_for_tests().with_provider_endpoint(AiProvider::Tinfoil, server.uri());
    config.pricing = repository.pricing.clone();
    Mock::given(method("GET")).and(path("/models")).respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "data":[{"id":"test-refresh-model", "pricing":{"inputTokenPricePer1M":0.12,"outputTokenPricePer1M":0.45}}]
    }))).mount(&server).await;
    refresh_provider_prices(&config, &repository, AiProvider::Tinfoil)
        .await
        .unwrap();
    assert_eq!(
        config.pricing.quote("tinfoil", "test-refresh-model").source,
        "provider_api"
    );
    server.reset().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    assert!(
        refresh_provider_prices(&config, &repository, AiProvider::Tinfoil)
            .await
            .is_err()
    );
    assert_eq!(
        config
            .pricing
            .quote("tinfoil", "test-refresh-model")
            .rates
            .input_per_million,
        0.12
    );
    let restarted = LlmUsageRepository::new(pool);
    restarted.load_prices().unwrap();
    let quote = restarted.pricing.quote("tinfoil", "test-refresh-model");
    assert_eq!(quote.source, "saved_api");
    assert_eq!(quote.rates.input_per_million, 0.12);
    let usage = TokenUsage {
        prompt_tokens: 1_000_000,
        completion_tokens: 100,
        total_tokens: 1_000_100,
        prompt_cache_hit_tokens: Some(0),
        ..Default::default()
    };
    let snapshot = restarted
        .pricing
        .cost("tinfoil", "test-refresh-model", &usage);
    // No user FK exists on llm_usage_logs: use a unique ID without modifying accounts.
    let user_id = -(rand::random::<u16>() as i32) - 1;
    restarted
        .log_priced_usage(
            user_id,
            "tinfoil",
            "test-refresh-model",
            "pricing_test",
            &usage,
            &snapshot,
        )
        .unwrap();
    let report = restarted
        .get_cost_report(14, chrono::Utc::now().timestamp() as i32 + 1)
        .unwrap();
    let user = report
        .per_user
        .iter()
        .find(|u| u.user_id == Some(user_id))
        .unwrap();
    assert_eq!(user.totals.historical_estimate_calls, 0);
    assert!((user.totals.provider_cost_usd - 0.120045).abs() < 1e-10);
    assert_eq!(user.totals.customer_cost_usd, snapshot.customer_cost_usd);
    server.reset().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":[]})))
        .mount(&server)
        .await;
    assert!(
        refresh_provider_prices(&config, &repository, AiProvider::Tinfoil)
            .await
            .is_err()
    );
    assert_eq!(
        config
            .pricing
            .quote("tinfoil", "test-refresh-model")
            .rates
            .input_per_million,
        0.12
    );
}

#[tokio::test]
async fn streaming_chat_requests_usage_and_preserves_discount_details() {
    use wiremock::{
        matchers::{body_partial_json, method, path},
        Mock, MockServer, ResponseTemplate,
    };
    let server = MockServer::start().await;
    let config =
        AiConfig::default_for_tests().with_provider_endpoint(AiProvider::Tinfoil, server.uri());
    let stream = concat!(
        "data: {\"id\":\"test\",\"model\":\"deepseek-v4-1-flash\",\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"Hello\"},\"finish_reason\":\"stop\"}]}\n\n",
        "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":20,\"total_tokens\":120,\"prompt_tokens_details\":{\"cached_tokens\":80}}}\n\n",
        "data: [DONE]\n\n"
    );
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(body_partial_json(
            json!({"stream_options":{"include_usage":true}}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(stream))
        .expect(2)
        .mount(&server)
        .await;
    let request = openai_api_rs::v1::chat_completion::ChatCompletionRequest::new(
        "deepseek-v4-1-flash".into(),
        vec![],
    );
    let buffered = config
        .chat_completion(AiProvider::Tinfoil, &request)
        .await
        .unwrap();
    let incremental = config
        .chat_completion_streaming(AiProvider::Tinfoil, &request, None)
        .await
        .unwrap();
    for response in [buffered, incremental] {
        assert_eq!(response.usage.cached_tokens(), Some(80));
        assert_eq!(response.usage.total_tokens, 120);
    }
}
