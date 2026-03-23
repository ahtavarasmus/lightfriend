CREATE TABLE ont_events (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    description TEXT NOT NULL,
    remind_at INTEGER,
    due_at INTEGER,
    status TEXT NOT NULL DEFAULT 'active',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX idx_ont_events_user_id ON ont_events(user_id);
CREATE INDEX idx_ont_events_remind ON ont_events(remind_at) WHERE status = 'active';
CREATE INDEX idx_ont_events_due ON ont_events(due_at) WHERE status = 'active';
