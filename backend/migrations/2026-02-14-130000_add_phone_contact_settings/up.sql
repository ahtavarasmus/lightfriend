ALTER TABLE user_settings ADD COLUMN phone_contact_notification_mode TEXT DEFAULT 'critical';
ALTER TABLE user_settings ADD COLUMN phone_contact_notification_type TEXT DEFAULT 'sms';
ALTER TABLE user_settings ADD COLUMN phone_contact_notify_on_call INTEGER NOT NULL DEFAULT 1;
