-- Recreate waiting_checks table
CREATE TABLE waiting_checks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    content TEXT NOT NULL,
    service_type TEXT NOT NULL,
    noti_type TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Migrate tasks back to waiting_checks (only recurring ones with conditions)
INSERT INTO waiting_checks (user_id, content, service_type, noti_type)
SELECT
    user_id,
    condition,
    CASE trigger
        WHEN 'recurring_email' THEN 'email'
        ELSE 'messaging'
    END,
    notification_type
FROM tasks
WHERE trigger IN ('recurring_email', 'recurring_messaging')
AND condition IS NOT NULL
AND status = 'active';

-- Recreate google_tasks table
CREATE TABLE google_tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    encrypted_access_token TEXT NOT NULL,
    encrypted_refresh_token TEXT NOT NULL,
    status TEXT NOT NULL,
    last_update INTEGER NOT NULL,
    created_on INTEGER NOT NULL,
    description TEXT NOT NULL,
    expires_in INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Recreate task_notifications table
CREATE TABLE task_notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    task_id TEXT NOT NULL,
    notification_time INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Drop tasks table
DROP TABLE tasks;
