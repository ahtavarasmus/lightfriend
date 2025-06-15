-- This file should undo anything in `up.sql`
-- Add columns back to users table
ALTER TABLE users ADD COLUMN notify BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE users ADD COLUMN notification_type TEXT;
ALTER TABLE users ADD COLUMN timezone TEXT;
ALTER TABLE users ADD COLUMN timezone_auto BOOLEAN;
ALTER TABLE users ADD COLUMN agent_language TEXT NOT NULL DEFAULT 'en';

-- Copy data back to users table
UPDATE users
SET 
    notify = (SELECT notify FROM user_settings WHERE user_settings.user_id = users.id),
    notification_type = (SELECT notification_type FROM user_settings WHERE user_settings.user_id = users.id),
    timezone = (SELECT timezone FROM user_settings WHERE user_settings.user_id = users.id),
    timezone_auto = (SELECT timezone_auto FROM user_settings WHERE user_settings.user_id = users.id),
    agent_language = (SELECT agent_language FROM user_settings WHERE user_settings.user_id = users.id);

-- Drop user_settings table
DROP TABLE user_settings;
