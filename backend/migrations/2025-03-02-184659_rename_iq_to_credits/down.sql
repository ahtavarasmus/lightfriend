-- This file should undo anything in `up.sql`
-- First, create a temporary table with the old schema
CREATE TABLE users_new (
    id INTEGER PRIMARY KEY NOT NULL,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    nickname TEXT,
    time_to_live INTEGER,
    verified BOOLEAN NOT NULL,
    iq INTEGER NOT NULL DEFAULT 0,
    notify_credits BOOLEAN NOT NULL,
    locality TEXT NOT NULL,
    info TEXT,
    preferred_number TEXT,
    iq_cost_per_euro INTEGER NOT NULL DEFAULT 300,
    debug_logging_permission BOOLEAN NOT NULL,
    charge_when_under BOOLEAN NOT NULL,
    charge_back_to INTEGER,
    stripe_customer_id TEXT,
    stripe_payment_method_id TEXT,
    stripe_checkout_session_id TEXT
);

-- Copy the data from the new table to the old table, converting credits back to iq
INSERT INTO users_new 
SELECT 
    id,
    email,
    password_hash,
    phone_number,
    nickname,
    time_to_live,
    verified,
    credits AS INTEGER as iq,
    notify_credits,
    locality,
    info,
    preferred_number,
    300 as iq_cost_per_euro,
    debug_logging_permission,
    charge_when_under,
    charge_back_to,
    stripe_customer_id,
    stripe_payment_method_id,
    stripe_checkout_session_id
FROM users;

-- Drop the new table
DROP TABLE users;

-- Rename the old table to the original name
ALTER TABLE users_new RENAME TO users;
