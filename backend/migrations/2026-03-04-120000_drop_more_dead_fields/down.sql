-- Re-add critical_categories table
CREATE TABLE critical_categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    category_name TEXT NOT NULL,
    definition TEXT,
    active BOOLEAN NOT NULL DEFAULT TRUE
);

-- users table: re-add 3 fields
ALTER TABLE users ADD COLUMN discount BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE users ADD COLUMN discount_tier TEXT;
ALTER TABLE users ADD COLUMN verified BOOLEAN NOT NULL DEFAULT TRUE;

-- user_settings table: re-add 9 fields
ALTER TABLE user_settings ADD COLUMN proactive_agent_on BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE user_settings ADD COLUMN morning_digest TEXT;
ALTER TABLE user_settings ADD COLUMN day_digest TEXT;
ALTER TABLE user_settings ADD COLUMN evening_digest TEXT;
ALTER TABLE user_settings ADD COLUMN quiet_mode_until INTEGER;
ALTER TABLE user_settings ADD COLUMN last_instant_digest_time INTEGER;
ALTER TABLE user_settings ADD COLUMN encrypted_textbee_device_id TEXT;
ALTER TABLE user_settings ADD COLUMN encrypted_textbee_api_key TEXT;
ALTER TABLE user_settings ADD COLUMN encrypted_openrouter_api_key TEXT;
