CREATE TABLE usage_logs_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    conversation_id TEXT,
    status TEXT,
    activity_type TEXT NOT NULL,
    credits REAL NOT NULL DEFAULT 0.0,
    created_at INTEGER NOT NULL,
    success BOOLEAN NOT NULL DEFAULT FALSE,
    summary TEXT
);

INSERT INTO usage_logs_new (
    id,
    user_id,
    conversation_id,
    status,
    activity_type,
    credits,
    created_at,
    success,
    summary
)
SELECT 
    id,
    user_id,
    conversation_id,
    status,
    activity_type,
    COALESCE(credits, 0.0),
    created_at,
    COALESCE(success, FALSE),
    summary
FROM usage_logs;

DROP TABLE usage_logs;
ALTER TABLE usage_logs_new RENAME TO usage_logs;
