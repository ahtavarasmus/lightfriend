-- This file should undo anything in `up.sql`
-- Create new table with username field
CREATE TABLE users_new (
    id INTEGER PRIMARY KEY NOT NULL,
    username TEXT,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    nickname TEXT,
    time_to_live INTEGER,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    iq INTEGER NOT NULL DEFAULT 0,
    notify_credits BOOLEAN NOT NULL DEFAULT TRUE,
    locality TEXT NOT NULL DEFAULT ''
);

-- Copy data and set username to NULL
INSERT INTO users_new (
    id,
    username,
    email,
    password_hash,
    phone_number,
    nickname,
    time_to_live,
    verified,
    iq,
    notify_credits,
    locality
)
SELECT 
    id,
    NULL as username,
    email,
    password_hash,
    phone_number,
    nickname,
    time_to_live,
    verified,
    iq,
    notify_credits,
    locality
FROM users;

-- Drop the old table
DROP TABLE users;

-- Rename the new table
ALTER TABLE users_new RENAME TO users;

