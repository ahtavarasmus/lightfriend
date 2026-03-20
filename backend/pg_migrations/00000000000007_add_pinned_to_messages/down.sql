DROP INDEX IF EXISTS idx_ont_messages_user_pinned;
ALTER TABLE ont_messages DROP COLUMN pinned;
