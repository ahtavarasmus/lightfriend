-- Add migration flag to users table
-- DEFAULT TRUE so old VPS continues working normally (all users handled locally)
-- After copying DB to new AWS, run: UPDATE users SET migrated_to_new_server = FALSE;
ALTER TABLE users ADD COLUMN migrated_to_new_server BOOLEAN NOT NULL DEFAULT TRUE;
