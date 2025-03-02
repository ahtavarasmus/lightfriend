-- Your SQL goes here
-- First create a temporary table with the new schema
CREATE TABLE usage_logs_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    activity_type TEXT NOT NULL,
    credits REAL NOT NULL,
    created_at INTEGER NOT NULL,
    success BOOLEAN NOT NULL,
    summary TEXT
);

-- Copy the data from the old table to the new table, converting iq to credits
INSERT INTO usage_logs_new 
SELECT 
    id,
    user_id,
    activity_type,
    CAST(iq_used AS REAL) / CAST(iq_cost_per_euro AS REAL) as credits,
    created_at,
    success,
    summary
FROM usage_logs;

-- Drop the old table
DROP TABLE usage_logs;

-- Rename the new table to the original name
ALTER TABLE usage_logs_new RENAME TO usage_logs;


