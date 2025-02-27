-- Your SQL goes here
ALTER TABLE users ADD COLUMN charge_when_under BOOLEAN DEFAULT false NOT NULL;
ALTER TABLE users ADD COLUMN charge_back_to INTEGER;
