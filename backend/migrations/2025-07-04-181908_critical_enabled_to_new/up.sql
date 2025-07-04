-- Your SQL goes here
-- Migration: Convert critical_enabled from bool to Option<String>
-- This migration transforms the critical_enabled field from boolean to nullable string
-- where true becomes "sms" and false becomes NULL

-- Step 1: Add the new column as nullable string
ALTER TABLE user_settings ADD COLUMN critical_enabled_new TEXT;

-- Step 2: Populate the new column based on existing boolean values
UPDATE user_settings 
SET critical_enabled_new = CASE 
    WHEN critical_enabled = 1 THEN 'sms'
    WHEN critical_enabled = 0 THEN NULL
    ELSE NULL
END;

-- Step 3: Drop the old boolean column
ALTER TABLE user_settings DROP COLUMN critical_enabled;

-- Step 4: Rename the new column to replace the old one
ALTER TABLE user_settings RENAME COLUMN critical_enabled_new TO critical_enabled;

-- Verify the migration (optional - remove in production)
-- SELECT critical_enabled, COUNT(*) as count 
-- FROM user_settings 
-- GROUP BY critical_enabled;
