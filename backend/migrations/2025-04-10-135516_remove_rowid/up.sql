-- Your SQL goes here
-- Step 1: Create a new table with the desired structure, including status
CREATE TABLE usage_logs_new (
    id INTEGER PRIMARY KEY,  -- Explicitly set id as the primary key (no rowid)
    user_id INTEGER NOT NULL,
    sid TEXT,
    activity_type TEXT NOT NULL,
    credits FLOAT,
    created_at INTEGER NOT NULL,
    time_consumed INTEGER,
    success BOOLEAN,
    reason TEXT,
    status TEXT,  -- Reintroduced status field
    recharge_threshold_timestamp INTEGER,
    zero_credits_timestamp INTEGER
);

-- Step 2: Copy data from the current table to the new table, excluding rowid
INSERT INTO usage_logs_new (
    id,
    user_id,
    sid,
    activity_type,
    credits,
    created_at,
    time_consumed,
    success,
    reason,
    recharge_threshold_timestamp,
    zero_credits_timestamp
)
SELECT 
    id,  -- Use the existing id column, not rowid
    user_id,
    sid,
    activity_type,
    credits,
    created_at,
    time_consumed,
    success,
    reason,
    recharge_threshold_timestamp,
    zero_credits_timestamp
FROM usage_logs;

-- Step 3: Drop the old table with the unwanted rowid
DROP TABLE usage_logs;

-- Step 4: Rename the new table to the original name
ALTER TABLE usage_logs_new RENAME TO usage_logs;
