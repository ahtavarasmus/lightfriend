DROP INDEX IF EXISTS idx_ont_messages_matrix_event_id;

ALTER TABLE ont_messages
DROP COLUMN matrix_event_id;
