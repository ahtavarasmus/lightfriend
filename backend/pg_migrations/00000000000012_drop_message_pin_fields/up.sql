ALTER TABLE ont_messages DROP COLUMN pinned;
ALTER TABLE ont_messages DROP COLUMN status;
ALTER TABLE ont_messages DROP COLUMN review_after;
DROP INDEX IF EXISTS idx_ont_messages_user_pinned;
