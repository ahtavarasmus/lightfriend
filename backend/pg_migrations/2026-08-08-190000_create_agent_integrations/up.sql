-- Write-only credentials for local AI-agent clients. Raw bearer and pairing
-- secrets are never persisted: only SHA-256 digests are stored.
CREATE TABLE agent_credentials (
    id SERIAL PRIMARY KEY,
    user_id INT4 NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE CHECK (char_length(token_hash) = 64),
    token_prefix TEXT NOT NULL CHECK (char_length(token_prefix) = 16),
    label TEXT NOT NULL CHECK (char_length(label) BETWEEN 1 AND 64),
    scopes TEXT NOT NULL DEFAULT 'reminders,reply_watch_email'
        CHECK (scopes = 'reminders,reply_watch_email'),
    daily_cap INT4 NOT NULL DEFAULT 20 CHECK (daily_cap BETWEEN 1 AND 50),
    daily_used INT4 NOT NULL DEFAULT 0 CHECK (daily_used BETWEEN 0 AND daily_cap),
    daily_reset_at INT4 NOT NULL,
    expires_at INT4 NOT NULL,
    created_at INT4 NOT NULL,
    last_used_at INT4,
    revoked_at INT4,
    CHECK (expires_at > created_at)
);

CREATE INDEX agent_credentials_user_idx
    ON agent_credentials (user_id, revoked_at, created_at DESC);

CREATE TABLE agent_pairing_sessions (
    id SERIAL PRIMARY KEY,
    device_code_hash TEXT NOT NULL UNIQUE CHECK (char_length(device_code_hash) = 64),
    user_code_hash TEXT NOT NULL UNIQUE CHECK (char_length(user_code_hash) = 64),
    client_name TEXT NOT NULL CHECK (char_length(client_name) BETWEEN 1 AND 64),
    created_at INT4 NOT NULL,
    expires_at INT4 NOT NULL,
    approved_by_user_id INT4 REFERENCES users(id) ON DELETE CASCADE,
    approved_at INT4,
    consumed_at INT4,
    CHECK (expires_at > created_at)
);

CREATE INDEX agent_pairing_sessions_expiry_idx
    ON agent_pairing_sessions (expires_at, consumed_at);

CREATE TABLE agent_action_idempotency (
    id SERIAL PRIMARY KEY,
    credential_id INT4 NOT NULL REFERENCES agent_credentials(id) ON DELETE CASCADE,
    action_kind TEXT NOT NULL CHECK (action_kind IN ('reminder', 'reply_watch_email')),
    key_hash TEXT NOT NULL CHECK (char_length(key_hash) = 64),
    outcome TEXT CHECK (outcome IN ('accepted', 'rejected', 'failed')),
    created_at INT4 NOT NULL,
    UNIQUE (credential_id, action_kind, key_hash)
);

CREATE INDEX agent_action_idempotency_created_idx
    ON agent_action_idempotency (created_at);

-- Deliberately excludes reminder text, email addresses, and all response data.
CREATE TABLE agent_action_audit (
    id BIGSERIAL PRIMARY KEY,
    credential_id INT4 REFERENCES agent_credentials(id) ON DELETE SET NULL,
    user_id INT4 NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    action_kind TEXT NOT NULL CHECK (action_kind IN ('reminder', 'reply_watch_email')),
    outcome TEXT NOT NULL CHECK (outcome IN ('accepted', 'rejected', 'failed')),
    created_at INT4 NOT NULL
);

CREATE INDEX agent_action_audit_user_idx
    ON agent_action_audit (user_id, created_at DESC);
