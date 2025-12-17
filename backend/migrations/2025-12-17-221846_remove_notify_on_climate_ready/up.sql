-- Remove the notify_on_climate_ready column from user_settings
-- This setting is no longer needed as notifications are now controlled per-request
ALTER TABLE user_settings DROP COLUMN notify_on_climate_ready;
