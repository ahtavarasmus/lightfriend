-- Drop critical_categories table
DROP TABLE IF EXISTS critical_categories;

-- users table: drop 3 dead fields
ALTER TABLE users DROP COLUMN discount;
ALTER TABLE users DROP COLUMN discount_tier;
ALTER TABLE users DROP COLUMN verified;

-- user_settings table: drop 9 dead fields
ALTER TABLE user_settings DROP COLUMN proactive_agent_on;
ALTER TABLE user_settings DROP COLUMN morning_digest;
ALTER TABLE user_settings DROP COLUMN day_digest;
ALTER TABLE user_settings DROP COLUMN evening_digest;
ALTER TABLE user_settings DROP COLUMN quiet_mode_until;
ALTER TABLE user_settings DROP COLUMN last_instant_digest_time;
ALTER TABLE user_settings DROP COLUMN encrypted_textbee_device_id;
ALTER TABLE user_settings DROP COLUMN encrypted_textbee_api_key;
ALTER TABLE user_settings DROP COLUMN encrypted_openrouter_api_key;
