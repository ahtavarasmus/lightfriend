-- Create contact_profiles table
CREATE TABLE contact_profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    nickname TEXT NOT NULL,
    whatsapp_chat TEXT,
    telegram_chat TEXT,
    signal_chat TEXT,
    email_addresses TEXT,
    notification_mode TEXT NOT NULL DEFAULT 'critical',
    notification_type TEXT NOT NULL DEFAULT 'sms',
    notify_on_call INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Add default_notification_mode to user_settings
ALTER TABLE user_settings ADD COLUMN default_notification_mode TEXT DEFAULT 'critical';

-- Migrate existing priority_senders to contact_profiles
INSERT INTO contact_profiles (user_id, nickname, whatsapp_chat, telegram_chat, signal_chat, email_addresses, notification_mode, notification_type, notify_on_call, created_at)
SELECT
    user_id,
    sender as nickname,
    CASE WHEN service_type = 'whatsapp' THEN sender ELSE NULL END,
    CASE WHEN service_type = 'telegram' THEN sender ELSE NULL END,
    CASE WHEN service_type = 'signal' THEN sender ELSE NULL END,
    CASE WHEN service_type = 'imap' THEN sender ELSE NULL END,
    CASE WHEN noti_mode = 'all' THEN 'all' ELSE 'critical' END,
    COALESCE(noti_type, 'sms'),
    1,
    CAST(strftime('%s', 'now') AS INTEGER)
FROM priority_senders;
