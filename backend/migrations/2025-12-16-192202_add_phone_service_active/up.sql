-- Add phone_service_active column to user_settings
-- Default to true so existing users keep service active
ALTER TABLE user_settings ADD COLUMN phone_service_active BOOLEAN NOT NULL DEFAULT 1;
