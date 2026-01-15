-- Create message_status_log table to track SMS delivery metadata (no message content)
CREATE TABLE message_status_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_sid TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    direction TEXT NOT NULL,
    to_number TEXT NOT NULL,
    from_number TEXT,
    status TEXT NOT NULL,
    error_code TEXT,
    error_message TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX idx_message_status_user ON message_status_log(user_id);
CREATE INDEX idx_message_status_sid ON message_status_log(message_sid);
CREATE INDEX idx_message_status_created ON message_status_log(created_at);
