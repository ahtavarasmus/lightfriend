-- Your SQL goes here
ALTER TABLE users ADD COLUMN debug_logging_permission BOOLEAN NOT NULL DEFAULT false;
