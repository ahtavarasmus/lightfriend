-- Convert all "call_sms" notification type values to "call"
UPDATE user_settings SET notification_type = 'call' WHERE notification_type = 'call_sms';
UPDATE user_settings SET default_notification_type = 'call' WHERE default_notification_type = 'call_sms';
UPDATE user_settings SET phone_contact_notification_type = 'call' WHERE phone_contact_notification_type = 'call_sms';
UPDATE user_settings SET critical_enabled = 'call' WHERE critical_enabled = 'call_sms';
UPDATE contact_profiles SET notification_type = 'call' WHERE notification_type = 'call_sms';
UPDATE contact_profile_exceptions SET notification_type = 'call' WHERE notification_type = 'call_sms';
UPDATE tasks SET notification_type = 'call' WHERE notification_type = 'call_sms';
UPDATE priority_senders SET noti_type = 'call' WHERE noti_type = 'call_sms';
