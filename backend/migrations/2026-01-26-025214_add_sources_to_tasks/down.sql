-- SQLite does not support DROP COLUMN directly.
-- We need to recreate the table without the new columns.

-- Step 1: Create a temporary table with the original schema
CREATE TABLE tasks_backup (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    trigger TEXT NOT NULL,
    condition TEXT,
    action TEXT NOT NULL,
    notification_type TEXT,
    status TEXT,
    created_at INTEGER NOT NULL,
    completed_at INTEGER,
    is_permanent INTEGER,
    recurrence_rule TEXT,
    recurrence_time TEXT
);

-- Step 2: Copy data to backup table
INSERT INTO tasks_backup (id, user_id, trigger, condition, action, notification_type, status, created_at, completed_at, is_permanent, recurrence_rule, recurrence_time)
SELECT id, user_id, trigger, condition, action, notification_type, status, created_at, completed_at, is_permanent, recurrence_rule, recurrence_time FROM tasks;

-- Step 3: Drop the original table
DROP TABLE tasks;

-- Step 4: Rename backup to original
ALTER TABLE tasks_backup RENAME TO tasks;
