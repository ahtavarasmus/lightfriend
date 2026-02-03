-- Remove migration flag from users table
-- Note: SQLite doesn't support DROP COLUMN directly, but Diesel handles this
ALTER TABLE users DROP COLUMN migrated_to_new_server;
