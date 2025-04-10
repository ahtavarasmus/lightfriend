
CREATE TABLE usage_logs_temp (
    id INTEGER,
    user_id INTEGER NOT NULL,
    sid TEXT,
    activity_type TEXT NOT NULL,
    credits FLOAT,
    created_at INTEGER NOT NULL,
    time_consumed INTEGER,
    success BOOLEAN,
    reason TEXT,
    recharge_threshold_timestamp INTEGER,
    zero_credits_timestamp INTEGER
);

-- Step 2: Copy data from the old table to the new temporary table, mapping matching columns
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

-- Step 3: Drop the old table
DROP TABLE usage_logs;

-- Step 4: Recreate the table with the exact new structure and rename the temp table
ALTER TABLE usage_logs_temp RENAME TO usage_logs;
