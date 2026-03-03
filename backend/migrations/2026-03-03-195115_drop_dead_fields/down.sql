-- Restore 17 dead fields across 4 tables

-- users table
ALTER TABLE users ADD COLUMN confirm_send_event TEXT;
ALTER TABLE users ADD COLUMN waiting_checks_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN free_reply BOOLEAN NOT NULL DEFAULT FALSE;

-- user_settings table
ALTER TABLE user_settings ADD COLUMN number_of_digests_locked INTEGER NOT NULL DEFAULT 0;
ALTER TABLE user_settings ADD COLUMN magic_login_token TEXT;
ALTER TABLE user_settings ADD COLUMN magic_login_token_expiration_timestamp INTEGER;
ALTER TABLE user_settings ADD COLUMN outbound_message_pricing FLOAT;
ALTER TABLE user_settings ADD COLUMN monthly_message_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE user_settings ADD COLUMN server_url TEXT;
ALTER TABLE user_settings ADD COLUMN server_ip TEXT;
ALTER TABLE user_settings ADD COLUMN encrypted_geoapify_key TEXT;
ALTER TABLE user_settings ADD COLUMN encrypted_pirate_weather_key TEXT;

-- user_info table
ALTER TABLE user_info ADD COLUMN dictionary TEXT;
ALTER TABLE user_info ADD COLUMN recent_contacts TEXT;
ALTER TABLE user_info ADD COLUMN blocker_password_vault TEXT;
ALTER TABLE user_info ADD COLUMN lockbox_password_vault TEXT;

-- imap_connection table
ALTER TABLE imap_connection ADD COLUMN expires_in INTEGER NOT NULL DEFAULT 0;
