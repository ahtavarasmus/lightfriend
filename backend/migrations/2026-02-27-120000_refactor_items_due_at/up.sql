-- Rename next_check_at to due_at and drop monitor
ALTER TABLE items RENAME COLUMN next_check_at TO due_at;
ALTER TABLE items DROP COLUMN monitor;

-- Recreate index on due_at (old one was on next_check_at)
DROP INDEX IF EXISTS idx_items_next_check;
CREATE INDEX idx_items_due_at ON items(due_at);
