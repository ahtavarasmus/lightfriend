-- Per-user webhook tokens for triggering an SMS to the user's own phone.
-- A leaked token can only target the owning user's phone_number; it cannot
-- be used as an open SMS relay, which keeps the abuse surface bounded.
--
-- token_hash is SHA-256 of the raw bearer string ("lf_<32 hex>"). The raw
-- token is shown to the user exactly once at creation time; the DB stores
-- only the hash, so a DB read does not let an attacker forge requests.
--
-- daily_cap / daily_sent / daily_reset_at bound cost even if a token leaks:
-- the cap is enforced via a single atomic UPDATE so two concurrent requests
-- cannot both slip past the boundary.
CREATE TABLE IF NOT EXISTS webhook_tokens (
    id SERIAL PRIMARY KEY,
    user_id INT4 NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    token_prefix TEXT NOT NULL,
    label TEXT NOT NULL,
    daily_cap INT4 NOT NULL DEFAULT 50,
    daily_sent INT4 NOT NULL DEFAULT 0,
    daily_reset_at INT4 NOT NULL,
    last_used_at INT4,
    revoked_at INT4,
    created_at INT4 NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_webhook_tokens_user_id ON webhook_tokens (user_id);
