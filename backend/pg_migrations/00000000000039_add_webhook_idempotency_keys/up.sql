-- Per-token idempotency record so retried webhook calls don't send
-- duplicate SMS. Lookup is keyed on (token_id, idempotency_key).
--
-- response_sid:
--   NULL = the original request is still in-flight; concurrent
--          replays should be rejected with 409 so the client can wait
--          and retry rather than racing the original.
--   non-NULL = the original request completed; replays return this
--              SID without re-billing or re-consuming the daily cap.
--
-- Lookup is TTL-gated in code (24h) so reusing a key after a day
-- starts fresh; rows older than that are inert but kept until a
-- periodic cleanup removes them. With the default 50/day cap, even a
-- year of usage tops out around 18k rows per token — small enough to
-- leave purging to a later cron job.
CREATE TABLE IF NOT EXISTS webhook_idempotency_keys (
    id SERIAL PRIMARY KEY,
    token_id INT4 NOT NULL REFERENCES webhook_tokens(id) ON DELETE CASCADE,
    idempotency_key TEXT NOT NULL,
    response_sid TEXT,
    created_at INT4 NOT NULL,
    UNIQUE (token_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_webhook_idempotency_created_at
    ON webhook_idempotency_keys (created_at);
