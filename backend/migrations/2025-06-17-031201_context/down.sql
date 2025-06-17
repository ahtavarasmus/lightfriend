-- This file should undo anything in `up.sql`
-- Add info back to users
ALTER TABLE users ADD COLUMN info TEXT;

-- Copy data back from user_settings.info to users.info
UPDATE users 
SET info = (
    SELECT info 
    FROM user_settings 
    WHERE user_settings.user_id = users.id
);

-- Remove info from user_settings
ALTER TABLE user_settings DROP COLUMN info;

-- Remove save_context from user_settings
ALTER TABLE user_settings DROP COLUMN save_context;
