-- Your SQL goes here
-- Create a temporary table with the new schema
CREATE TABLE users_new (
    id INTEGER PRIMARY KEY NOT NULL,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    nickname TEXT,
    time_to_live INTEGER,
    verified BOOLEAN NOT NULL,
    credits REAL NOT NULL,
    notify BOOLEAN NOT NULL,
    info TEXT,
    preferred_number TEXT,
    debug_logging_permission BOOLEAN NOT NULL,
    charge_when_under BOOLEAN NOT NULL,
    charge_back_to REAL,
    stripe_customer_id TEXT,
    stripe_payment_method_id TEXT,
    stripe_checkout_session_id TEXT
);

-- Copy the data, converting charge_back_to to REAL
INSERT INTO users_new 
SELECT 
    id,
    email,
    password_hash,
    phone_number,
    nickname,
    time_to_live,
    verified,
    credits,
    notify,
    info,
    preferred_number,
    debug_logging_permission,
    charge_when_under,
    CAST(charge_back_to AS REAL),
    stripe_customer_id,
    stripe_payment_method_id,
    stripe_checkout_session_id
FROM users;

-- Drop the old table
DROP TABLE users;

-- Rename the new table to the original name
ALTER TABLE users_new RENAME TO users;


