CREATE TABLE light_tool_pairing_sessions (
    user_id INT4 PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at INT4 NOT NULL,
    consumed_at INT4,
    consumed_by_device_id INT4 REFERENCES light_tool_devices(id) ON DELETE SET NULL,
    created_at INT4 NOT NULL
);

CREATE INDEX light_tool_pairing_sessions_expires_idx
    ON light_tool_pairing_sessions (expires_at);
