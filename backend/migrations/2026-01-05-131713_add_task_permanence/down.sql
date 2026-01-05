-- SQLite doesn't support DROP COLUMN directly, so we need to recreate the table
-- Create temporary table without new columns
CREATE TABLE tasks_backup (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    trigger TEXT NOT NULL,
    condition TEXT,
    action TEXT NOT NULL,
    notification_type TEXT DEFAULT 'sms',
    status TEXT DEFAULT 'active',
    created_at INTEGER NOT NULL,
    completed_at INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Copy data
INSERT INTO tasks_backup SELECT id, user_id, trigger, condition, action, notification_type, status, created_at, completed_at FROM tasks;

-- Drop old table and rename
DROP TABLE tasks;
ALTER TABLE tasks_backup RENAME TO tasks;

-- Recreate indexes
CREATE INDEX idx_tasks_once ON tasks(trigger, status) WHERE trigger LIKE 'once_%' AND status = 'active';
CREATE INDEX idx_tasks_recurring ON tasks(trigger, status) WHERE trigger LIKE 'recurring_%' AND status = 'active';
CREATE INDEX idx_tasks_user ON tasks(user_id, status);
