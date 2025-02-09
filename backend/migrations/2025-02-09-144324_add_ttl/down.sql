-- This file should undo anything in `up.sql`
-- Remove time_to_live column
CREATE TABLE users_new (
    id INTEGER PRIMARY KEY NOT NULL,
    username TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    nickname TEXT
);

-- Copy data excluding time_to_live
INSERT INTO users_new (id, username, password_hash, phone_number, nickname)
SELECT id, username, password_hash, phone_number, nickname
FROM users;

-- Drop the old table
DROP TABLE users;

-- Rename the new table
ALTER TABLE users_new RENAME TO users;

