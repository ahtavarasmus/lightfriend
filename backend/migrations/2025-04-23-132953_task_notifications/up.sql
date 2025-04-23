-- Your SQL goes here
CREATE TABLE task_notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    task_id TEXT NOT NULL,  -- Google Task ID
    notified_at INTEGER NOT NULL,  -- Unix timestamp
    UNIQUE(user_id, task_id)
);
