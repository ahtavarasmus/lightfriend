-- Your SQL goes here
-- This migration creates a new proactive_settings table
-- and moves imap_proactive and imap_general_checks from the users table

-- Create new proactive_settings table
CREATE TABLE proactive_settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    imap_proactive BOOLEAN NOT NULL DEFAULT false,
    imap_general_checks TEXT,
    proactive_calendar BOOLEAN NOT NULL DEFAULT false,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Create index for faster lookup
CREATE INDEX proactive_settings_user_id ON proactive_settings (user_id);

-- Copy existing settings from users table to the new proactive_settings table
INSERT INTO proactive_settings (
    user_id,
    imap_proactive,
    imap_general_checks,
    proactive_calendar,
    created_at,
    updated_at
)
SELECT
    id,
    COALESCE(imap_proactive, false),
    imap_general_checks,
    false,  -- Default value for new proactive_calendar field
    unixepoch(),
    unixepoch()
FROM users;

-- Remove the columns from users table
ALTER TABLE users DROP COLUMN imap_proactive;
ALTER TABLE users DROP COLUMN imap_general_checks;
