CREATE TABLE light_tool_devices (
    id SERIAL PRIMARY KEY,
    installation_id_hash TEXT NOT NULL UNIQUE,
    device_token_hash TEXT NOT NULL UNIQUE,
    user_id INT4 REFERENCES users(id) ON DELETE SET NULL,
    trial_started_at INT4 NOT NULL,
    trial_expires_at INT4 NOT NULL,
    trial_messages_used INT4 NOT NULL DEFAULT 0 CHECK (trial_messages_used >= 0),
    last_seen_at INT4 NOT NULL,
    revoked_at INT4,
    created_at INT4 NOT NULL,
    updated_at INT4 NOT NULL
);

CREATE INDEX light_tool_devices_user_idx
    ON light_tool_devices (user_id)
    WHERE user_id IS NOT NULL;
