CREATE TABLE ont_events (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    description TEXT NOT NULL,
    notify_at INTEGER,
    expires_at INTEGER,
    status TEXT NOT NULL DEFAULT 'active',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX idx_ont_events_user_id ON ont_events(user_id);
CREATE INDEX idx_ont_events_notify ON ont_events(notify_at) WHERE status = 'active';
CREATE INDEX idx_ont_events_expires ON ont_events(expires_at) WHERE status = 'active';
