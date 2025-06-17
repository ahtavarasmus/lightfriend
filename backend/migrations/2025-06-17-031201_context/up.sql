-- Your SQL goes here
-- Add save_context to user_settings
ALTER TABLE user_settings ADD COLUMN save_context INTEGER DEFAULT 5;

-- Add info to user_settings
ALTER TABLE user_settings ADD COLUMN info TEXT;

-- Copy data from users.info to user_settings.info
UPDATE user_settings 
SET info = (
    SELECT info 
    FROM users 
    WHERE users.id = user_settings.user_id
);

-- Remove info column from users
ALTER TABLE users DROP COLUMN info;
