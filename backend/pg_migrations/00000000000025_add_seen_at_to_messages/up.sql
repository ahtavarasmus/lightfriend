ALTER TABLE ont_messages ADD COLUMN seen_at INTEGER;
UPDATE ont_messages SET seen_at = resolved_at WHERE resolved_at IS NOT NULL;
