-- Your SQL goes here
-- SQLite doesn't support ALTER COLUMN, so we need to recreate the table
CREATE TABLE users_new (
    id INTEGER PRIMARY KEY NOT NULL,
    username TEXT NULL,  -- Make it explicitly nullable
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

-- Copy the data
INSERT INTO users_new SELECT * FROM users;

-- Drop the old table
DROP TABLE users;

-- Rename the new table
ALTER TABLE users_new RENAME TO users;

