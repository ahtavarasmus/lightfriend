CREATE TABLE mcp_access_tokens (
    id SERIAL PRIMARY KEY,
    -- Bind credential lifetime to the user's secret-bearing profile. The
    -- password-confirmed "Delete my data" flow deletes user_secrets, so MCP
    -- access is revoked even though the login/subscription row is retained.
    user_id INT4 NOT NULL REFERENCES user_secrets(user_id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    token_prefix TEXT NOT NULL,
    label TEXT NOT NULL,
    created_at INT4 NOT NULL,
    last_used_at INT4,
    revoked_at INT4
);

CREATE INDEX mcp_access_tokens_user_idx
    ON mcp_access_tokens (user_id, created_at DESC);
