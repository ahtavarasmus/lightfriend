-- Your SQL goes here
-- Create a new table with the desired schema
CREATE TABLE users_new (
    id INTEGER PRIMARY KEY NOT NULL,
    username TEXT NOT NULL,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    nickname TEXT,
    time_to_live INTEGER,
    verified BOOLEAN NOT NULL,
    iq INTEGER NOT NULL,
    notify_credits BOOLEAN NOT NULL,
    locality TEXT NOT NULL
);

-- Copy the data
INSERT INTO users_new(id, username, email, password_hash, phone_number, nickname, time_to_live, verified, iq, notify_credits, locality)
SELECT id, username, username, password_hash, phone_number, nickname, time_to_live, verified, iq, notify_credits, locality
FROM users;

-- Drop the old table
DROP TABLE users;

-- Rename the new table to the original name
ALTER TABLE users_new RENAME TO users;

-- Add the unique index
CREATE UNIQUE INDEX users_email_unique_idx ON users(email);

