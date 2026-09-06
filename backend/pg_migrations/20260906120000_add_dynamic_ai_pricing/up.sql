CREATE TABLE ai_model_prices (
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    rates TEXT NOT NULL,
    fetched_at INTEGER NOT NULL,
    PRIMARY KEY (provider, model)
);

ALTER TABLE llm_usage_logs
    ADD COLUMN cached_prompt_tokens INTEGER,
    ADD COLUMN provider_cost_usd DOUBLE PRECISION,
    ADD COLUMN customer_cost_usd DOUBLE PRECISION,
    ADD COLUMN pricing_snapshot TEXT;
