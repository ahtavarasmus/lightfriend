-- Migrate notification_type 'call_sms' to 'call' across contact tables
UPDATE contact_profiles SET notification_type = 'call' WHERE notification_type = 'call_sms';
UPDATE contact_profile_exceptions SET notification_type = 'call' WHERE notification_type = 'call_sms';
