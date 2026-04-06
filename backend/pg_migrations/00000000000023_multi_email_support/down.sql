ALTER TABLE imap_connection DROP CONSTRAINT IF EXISTS unique_user_email;
ALTER TABLE processed_emails DROP COLUMN IF EXISTS imap_connection_id;
