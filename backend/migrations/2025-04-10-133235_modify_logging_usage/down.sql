-- This file should undo anything in `up.sql`
CREATE TABLE usage_logs_temp (
    id INTEGER,
    user_id INTEGER NOT NULL,
    conversation_id TEXT,
    status TEXT,
    activity_type TEXT NOT NULL,
    credits FLOAT,
    created_at INTEGER NOT NULL,
    success BOOLEAN,
    summary TEXT,
    recharge_threshold_timestamp INTEGER,
    zero_credits_timestamp INTEGER
);

-- Step 2: Copy data from the current table back to the temporary table, mapping matching columns
INSERT INTO usage_logs_temp (
    id,
    user_id,
    activity_type,
    credits,
    created_at,
    success,
    recharge_threshold_timestamp,
    zero_credits_timestamp
)
SELECT 
    id,
    user_id,
    activity_type,
    credits,
    created_at,
    success,
    recharge_threshold_timestamp,
    zero_credits_timestamp
FROM usage_logs;

-- Step 3: Drop the current table
DROP TABLE usage_logs;

-- Step 4: Rename the temporary table to the original name
ALTER TABLE usage_logs_temp RENAME TO usage_logs;
