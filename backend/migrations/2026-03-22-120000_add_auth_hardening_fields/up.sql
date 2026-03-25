ALTER TABLE users
    ADD COLUMN IF NOT EXISTS refresh_token_hash TEXT,
    ADD COLUMN IF NOT EXISTS refresh_token_compromised BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS magic_token_expires_at INTEGER;
