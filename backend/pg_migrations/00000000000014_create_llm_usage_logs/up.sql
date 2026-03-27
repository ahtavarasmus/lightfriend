CREATE TABLE llm_usage_logs (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    callsite TEXT NOT NULL,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_llm_usage_logs_user_id ON llm_usage_logs(user_id);
CREATE INDEX idx_llm_usage_logs_created_at ON llm_usage_logs(created_at);
