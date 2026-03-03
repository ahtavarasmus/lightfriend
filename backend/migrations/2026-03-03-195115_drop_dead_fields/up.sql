-- Drop 17 dead fields across 4 tables

-- users table (3 fields)
ALTER TABLE users DROP COLUMN confirm_send_event;
ALTER TABLE users DROP COLUMN waiting_checks_count;
ALTER TABLE users DROP COLUMN free_reply;

-- user_settings table (9 fields)
ALTER TABLE user_settings DROP COLUMN number_of_digests_locked;
ALTER TABLE user_settings DROP COLUMN magic_login_token;
ALTER TABLE user_settings DROP COLUMN magic_login_token_expiration_timestamp;
ALTER TABLE user_settings DROP COLUMN outbound_message_pricing;
ALTER TABLE user_settings DROP COLUMN monthly_message_count;
ALTER TABLE user_settings DROP COLUMN server_url;
ALTER TABLE user_settings DROP COLUMN server_ip;
ALTER TABLE user_settings DROP COLUMN encrypted_geoapify_key;
ALTER TABLE user_settings DROP COLUMN encrypted_pirate_weather_key;

-- user_info table (4 fields)
ALTER TABLE user_info DROP COLUMN dictionary;
ALTER TABLE user_info DROP COLUMN recent_contacts;
ALTER TABLE user_info DROP COLUMN blocker_password_vault;
ALTER TABLE user_info DROP COLUMN lockbox_password_vault;

-- imap_connection table (1 field)
ALTER TABLE imap_connection DROP COLUMN expires_in;
