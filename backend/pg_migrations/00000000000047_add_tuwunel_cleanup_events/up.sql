-- Durable audit trail for Tuwunel cleanup commands sent after bridge events
-- are persisted into Lightfriend's canonical ontology store.
CREATE TABLE IF NOT EXISTS tuwunel_cleanup_events (
    id SERIAL PRIMARY KEY,
    user_id INT4 NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ontology_message_id INT8 NOT NULL,
    service TEXT NOT NULL,
    room_id TEXT NOT NULL,
    event_id TEXT NOT NULL UNIQUE,
    delete_media BOOLEAN NOT NULL DEFAULT FALSE,
    commands_expected INT4 NOT NULL DEFAULT 1,
    commands_accepted INT4 NOT NULL DEFAULT 0,
    attempt_count INT4 NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    last_command_kind TEXT,
    last_admin_room_id TEXT,
    last_admin_command_event_id TEXT,
    last_error TEXT,
    enqueued_at INT4 NOT NULL,
    last_attempted_at INT4,
    completed_at INT4,
    updated_at INT4 NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tuwunel_cleanup_events_status
    ON tuwunel_cleanup_events (status, updated_at);

CREATE INDEX IF NOT EXISTS idx_tuwunel_cleanup_events_user_time
    ON tuwunel_cleanup_events (user_id, enqueued_at DESC);
