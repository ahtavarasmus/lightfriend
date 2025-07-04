-- This file should undo anything in `up.sql`
-- ROLLBACK MIGRATION: Convert critical_enabled from Option<String> back to bool
-- This migration reverses the previous migration, converting the critical_enabled field 
-- from nullable string back to boolean where any non-null string becomes true

-- Step 1: Add the old boolean column back
ALTER TABLE user_settings ADD COLUMN critical_enabled_old INTEGER NOT NULL DEFAULT 0;

-- Step 2: Populate the boolean column based on string values
UPDATE user_settings 
SET critical_enabled_old = CASE 
    WHEN critical_enabled IS NOT NULL AND critical_enabled != '' THEN 1
    WHEN critical_enabled IS NULL OR critical_enabled = '' THEN 0
    ELSE 0
END;

-- Step 3: Drop the string column
ALTER TABLE user_settings DROP COLUMN critical_enabled;

-- Step 4: Rename the boolean column back to original name
ALTER TABLE user_settings RENAME COLUMN critical_enabled_old TO critical_enabled;

-- Verify the rollback (optional - remove in production)
-- SELECT critical_enabled, COUNT(*) as count 
-- FROM user_settings 
-- GROUP BY critical_enabled;
