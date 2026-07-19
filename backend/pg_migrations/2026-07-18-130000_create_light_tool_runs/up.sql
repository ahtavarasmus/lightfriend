CREATE TABLE light_tool_runs (
    id TEXT PRIMARY KEY,
    device_id INT4 NOT NULL REFERENCES light_tool_devices(id) ON DELETE CASCADE,
    client_message_id TEXT NOT NULL,
    encrypted_user_message TEXT NOT NULL,
    encrypted_activity_text TEXT,
    encrypted_assistant_message TEXT,
    encrypted_error_message TEXT,
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'running', 'completed', 'failed')),
    created_at INT4 NOT NULL,
    updated_at INT4 NOT NULL,
    completed_at INT4,
    UNIQUE (device_id, client_message_id)
);

CREATE INDEX light_tool_runs_device_created_idx
    ON light_tool_runs (device_id, created_at DESC);

CREATE INDEX light_tool_runs_pending_idx
    ON light_tool_runs (status, updated_at)
    WHERE status IN ('queued', 'running');
