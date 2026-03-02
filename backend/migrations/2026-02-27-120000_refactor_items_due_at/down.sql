ALTER TABLE items ADD COLUMN monitor BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE items RENAME COLUMN due_at TO next_check_at;
DROP INDEX IF EXISTS idx_items_due_at;
CREATE INDEX idx_items_next_check ON items(next_check_at);
