-- Re-add the notify_on_climate_ready column to user_settings
ALTER TABLE user_settings ADD COLUMN notify_on_climate_ready INTEGER NOT NULL DEFAULT 1;
