-- Your SQL goes here
-- First, ensure all existing records have a phone number (using email as fallback if needed)
UPDATE users SET phone_number = email WHERE phone_number IS NULL;

-- Change phone_number to be NOT NULL and email to be nullable
ALTER TABLE users RENAME TO users_old;

CREATE TABLE users (
    id INTEGER PRIMARY KEY NOT NULL,
    username TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    email TEXT,
    phone_number TEXT NOT NULL,
    nickname TEXT
);

INSERT INTO users (id, username, password_hash, email, phone_number, nickname)
SELECT id, username, password_hash, email, phone_number, nickname
FROM users_old;

DROP TABLE users_old;

