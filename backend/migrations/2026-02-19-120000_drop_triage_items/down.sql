CREATE TABLE triage_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    item_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    summary TEXT NOT NULL,
    suggested_action TEXT,
    reasoning TEXT,
    context_json TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    source_type TEXT,
    source_id TEXT,
    created_at INTEGER NOT NULL,
    snooze_until INTEGER,
    expires_at INTEGER
);
