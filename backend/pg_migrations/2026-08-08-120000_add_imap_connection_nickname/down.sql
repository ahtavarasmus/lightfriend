DROP INDEX IF EXISTS imap_connection_user_nickname_unique;

ALTER TABLE imap_connection
DROP COLUMN nickname;
