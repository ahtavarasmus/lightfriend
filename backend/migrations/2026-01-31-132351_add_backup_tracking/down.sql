-- SQLite doesn't support DROP COLUMN directly in older versions,
-- but modern SQLite (3.35.0+) does support it
ALTER TABLE users DROP COLUMN last_backup_at;
ALTER TABLE users DROP COLUMN backup_session_active;
