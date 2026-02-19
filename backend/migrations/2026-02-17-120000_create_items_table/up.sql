CREATE TABLE items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    summary TEXT NOT NULL,
    monitor BOOLEAN NOT NULL DEFAULT 0,
    due_at INTEGER,
    next_check_at INTEGER,
    priority INTEGER NOT NULL DEFAULT 0,
    source_id TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX idx_items_user ON items(user_id);
CREATE INDEX idx_items_next_check ON items(next_check_at);
CREATE INDEX idx_items_source ON items(user_id, source_id);
