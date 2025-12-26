-- Add default notify on call column to user_settings
ALTER TABLE user_settings ADD COLUMN default_notify_on_call INTEGER NOT NULL DEFAULT 1;
