-- SQLite doesn't support DROP COLUMN before 3.35.0
-- Create new table without end_time, copy data, swap
CREATE TABLE tasks_backup (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    trigger TEXT NOT NULL,
    condition TEXT,
    action TEXT NOT NULL,
    notification_type TEXT,
    status TEXT,
    created_at INTEGER NOT NULL,
    completed_at INTEGER,
    is_permanent INTEGER,
    recurrence_rule TEXT,
    recurrence_time TEXT,
    sources TEXT
);

INSERT INTO tasks_backup SELECT id, user_id, trigger, condition, action, notification_type, status, created_at, completed_at, is_permanent, recurrence_rule, recurrence_time, sources FROM tasks;
DROP TABLE tasks;
ALTER TABLE tasks_backup RENAME TO tasks;
