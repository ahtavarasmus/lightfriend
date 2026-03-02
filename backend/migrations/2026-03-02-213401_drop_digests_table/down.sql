CREATE TABLE digests (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    time TEXT NOT NULL,
    tools TEXT NOT NULL,
    tool_params TEXT,
    enabled INTEGER NOT NULL DEFAULT 0,
    last_sent_at INTEGER,
    created_at INTEGER NOT NULL
);
