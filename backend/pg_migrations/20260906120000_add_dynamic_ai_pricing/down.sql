ALTER TABLE llm_usage_logs
    DROP COLUMN pricing_snapshot,
    DROP COLUMN customer_cost_usd,
    DROP COLUMN provider_cost_usd,
    DROP COLUMN cached_prompt_tokens;
DROP TABLE ai_model_prices;
