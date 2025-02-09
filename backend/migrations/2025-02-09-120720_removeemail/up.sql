-- Create temporary table without email field
CREATE TABLE users_new (
    id INTEGER PRIMARY KEY NOT NULL,
    username TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    nickname TEXT
);

-- Copy data from old table to new table
INSERT INTO users_new (id, username, password_hash, phone_number, nickname)
SELECT id, username, password_hash, phone_number, nickname
FROM users;

-- Drop the old table
DROP TABLE users;

-- Rename the new table to the original name
ALTER TABLE users_new RENAME TO users;
