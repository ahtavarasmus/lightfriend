-- This file should undo anything in `up.sql`
CREATE TABLE calls_new (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    conversation_id INTEGER NOT NULL,  -- Changed back to INTEGER
    status TEXT NOT NULL,
    analysis TEXT,
    call_duration_secs INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users (id)
);

-- Copy the data from the current table to the new table
-- Convert conversation_id back to INTEGER during the copy
INSERT INTO calls_new (id, user_id, conversation_id, status, analysis, call_duration_secs, created_at)
SELECT id, user_id, CAST(conversation_id AS INTEGER), status, analysis, call_duration_secs, created_at
FROM calls;

-- Drop the current table
DROP TABLE calls;

-- Rename the new table to the original name
ALTER TABLE calls_new RENAME TO calls;

