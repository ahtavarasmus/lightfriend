CREATE TABLE light_tool_push_outbox (
    device_id INT4 PRIMARY KEY REFERENCES light_tool_devices(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    version INT8 NOT NULL,
    attempt_count INT4 NOT NULL,
    next_attempt_at INT4 NOT NULL,
    lease_until INT4 NOT NULL,
    created_at INT4 NOT NULL,
    updated_at INT4 NOT NULL
);

CREATE INDEX light_tool_push_outbox_due_idx
    ON light_tool_push_outbox (next_attempt_at, lease_until);
