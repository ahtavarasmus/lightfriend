-- Normalize existing mailbox descriptions before adding global uniqueness.
UPDATE imap_connection
SET description = lower(trim(description))
WHERE description IS NOT NULL;

-- Enforce that one mailbox can belong to only one user at a time.
ALTER TABLE imap_connection
ADD CONSTRAINT unique_email_mailbox_global UNIQUE (description);
