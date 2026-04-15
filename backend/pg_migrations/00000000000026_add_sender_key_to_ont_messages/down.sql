DROP INDEX IF EXISTS idx_ont_messages_sender_key;

ALTER TABLE ont_messages
DROP COLUMN sender_key;
