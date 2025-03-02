-- This file should undo anything in `up.sql`
-- First create a temporary table with the old schema
CREATE TABLE usage_logs_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    activity_type TEXT NOT NULL,
    iq_used INTEGER NOT NULL,
    iq_cost_per_euro INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    success BOOLEAN NOT NULL,
    summary TEXT
);

-- Copy the data from the new table to the old table, converting credits back to iq
INSERT INTO usage_logs_new 
SELECT 
    id,
    user_id,
    activity_type,
    credits AS INTEGER as iq_used,
    created_at,
    success,
    summary
FROM usage_logs;

-- Drop the new table
DROP TABLE usage_logs;

-- Rename the old table to the original name
ALTER TABLE usage_logs_new RENAME TO usage_logs;


