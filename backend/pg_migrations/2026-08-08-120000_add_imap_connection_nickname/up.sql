ALTER TABLE imap_connection
ADD COLUMN nickname TEXT;

CREATE UNIQUE INDEX imap_connection_user_nickname_unique
ON imap_connection (user_id, nickname)
WHERE nickname IS NOT NULL;
