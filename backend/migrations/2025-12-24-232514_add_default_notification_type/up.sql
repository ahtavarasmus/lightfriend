-- Add default notification type column to user_settings
ALTER TABLE user_settings ADD COLUMN default_notification_type TEXT DEFAULT 'sms';
