-- Your SQL goes here
ALTER TABLE users ADD COLUMN locality TEXT NOT NULL DEFAULT 'usa';
UPDATE users SET locality = 'fin' WHERE phone_number LIKE '+358%';

