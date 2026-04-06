-- Support multiple email connections per user
-- Add connection_id to processed_emails for per-account UID scoping
ALTER TABLE processed_emails ADD COLUMN imap_connection_id INTEGER REFERENCES imap_connection(id) ON DELETE CASCADE;

-- Prevent duplicate email addresses per user
ALTER TABLE imap_connection ADD CONSTRAINT unique_user_email UNIQUE (user_id, description);
