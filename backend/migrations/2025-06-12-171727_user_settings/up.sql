-- Your SQL goes here
-- Create new user_settings table
CREATE TABLE user_settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL UNIQUE,
    notify BOOLEAN NOT NULL DEFAULT true,
    notification_type TEXT,
    timezone TEXT,
    timezone_auto BOOLEAN,
    agent_language TEXT NOT NULL,
    sub_country TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Copy existing data from users table to user_settings
INSERT INTO user_settings (user_id, notify, notification_type, timezone, timezone_auto, agent_language)
SELECT id, notify, notification_type, timezone, timezone_auto, agent_language
FROM users;

-- Remove columns from users table
CREATE TABLE users_new (
    id INTEGER PRIMARY KEY NOT NULL,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    nickname TEXT,
    time_to_live INTEGER,
    verified BOOLEAN NOT NULL,
    credits REAL NOT NULL,
    info TEXT,
    preferred_number TEXT,
    charge_when_under BOOLEAN NOT NULL,
    charge_back_to REAL,
    stripe_customer_id TEXT,
    stripe_payment_method_id TEXT,
    stripe_checkout_session_id TEXT,
    matrix_username TEXT,
    encrypted_matrix_access_token TEXT,
    sub_tier TEXT,
    msgs_left INTEGER NOT NULL,
    matrix_device_id TEXT,
    credits_left REAL NOT NULL,
    encrypted_matrix_password TEXT,
    encrypted_matrix_secret_storage_recovery_key TEXT,
    last_credits_notification INTEGER,
    confirm_send_event BOOLEAN NOT NULL,
    discount BOOLEAN NOT NULL DEFAULT false,
    discount_tier TEXT
);

-- Copy data to new users table
INSERT INTO users_new SELECT 
    id,
    email,
    password_hash,
    phone_number,
    nickname,
    time_to_live,
    verified,
    credits,
    info,
    preferred_number,
    charge_when_under,
    charge_back_to,
    stripe_customer_id,
    stripe_payment_method_id,
    stripe_checkout_session_id,
    matrix_username,
    encrypted_matrix_access_token,
    sub_tier,
    msgs_left,
    matrix_device_id,
    credits_left,
    encrypted_matrix_password,
    encrypted_matrix_secret_storage_recovery_key,
    last_credits_notification,
    confirm_send_event,
    discount,
    discount_tier
FROM users;

-- Drop old table and rename new one
DROP TABLE users;
ALTER TABLE users_new RENAME TO users;
