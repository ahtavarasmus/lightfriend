-- This file should undo anything in `up.sql`
-- Revert the changes
ALTER TABLE users RENAME TO users_old;

CREATE TABLE users (
    id INTEGER PRIMARY KEY NOT NULL,
    username TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    email TEXT NOT NULL,
    phone_number TEXT,
    nickname TEXT
);

INSERT INTO users (id, username, password_hash, email, phone_number, nickname)
SELECT id, username, password_hash, email, phone_number, nickname
FROM users_old;

DROP TABLE users_old;

