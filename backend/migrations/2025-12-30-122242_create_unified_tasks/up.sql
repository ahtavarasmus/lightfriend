-- Create unified tasks table
CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,

    -- TRIGGER (single encoded field)
    trigger TEXT NOT NULL,           -- "once_<timestamp>" or "recurring_email" or "recurring_messaging"

    -- CONDITION & ACTION (natural language - AI interprets at runtime)
    condition TEXT,                  -- NULL or natural language
    action TEXT NOT NULL,            -- natural language

    -- CONFIG
    notification_type TEXT DEFAULT 'sms',  -- "sms" or "call"

    -- STATE
    status TEXT DEFAULT 'active',    -- "active", "completed", "cancelled"
    created_at INTEGER NOT NULL,
    completed_at INTEGER,

    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Index for querying due "once" tasks
CREATE INDEX idx_tasks_once ON tasks(trigger, status)
    WHERE trigger LIKE 'once_%' AND status = 'active';

-- Index for recurring tasks
CREATE INDEX idx_tasks_recurring ON tasks(trigger, status)
    WHERE trigger LIKE 'recurring_%' AND status = 'active';

-- Index for user tasks
CREATE INDEX idx_tasks_user ON tasks(user_id, status);

-- Migrate existing waiting_checks to new tasks format
INSERT INTO tasks (
    user_id,
    trigger,
    condition,
    action,
    notification_type,
    status,
    created_at
)
SELECT
    user_id,
    CASE service_type
        WHEN 'email' THEN 'recurring_email'
        ELSE 'recurring_messaging'
    END,
    content,                                -- condition (natural language)
    'notify me about this',                 -- action
    COALESCE(noti_type, 'sms'),
    'active',
    strftime('%s', 'now')
FROM waiting_checks;

-- Drop old tables
DROP TABLE waiting_checks;
DROP TABLE IF EXISTS task_notifications;
DROP TABLE IF EXISTS google_tasks;
