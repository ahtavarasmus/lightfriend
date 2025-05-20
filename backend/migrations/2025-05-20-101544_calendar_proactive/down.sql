-- This file should undo anything in `up.sql`
-- This migration reverts the creation of the proactive_settings table
-- and moves data back to the users table

-- Add columns back to users table
ALTER TABLE users ADD COLUMN imap_proactive BOOLEAN DEFAULT false;
ALTER TABLE users ADD COLUMN imap_general_checks TEXT;

-- Copy data back to users table
UPDATE users
SET 
    imap_proactive = (
        SELECT imap_proactive 
        FROM proactive_settings 
        WHERE proactive_settings.user_id = users.id
    ),
    imap_general_checks = (
        SELECT imap_general_checks 
        FROM proactive_settings 
        WHERE proactive_settings.user_id = users.id
    );

-- Drop the proactive_settings table
DROP TABLE proactive_settings;
