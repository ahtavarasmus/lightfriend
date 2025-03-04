-- This file should undo anything in `up.sql`
ALTER TABLE usage_logs DROP COLUMN conversation_id;
ALTER TABLE usage_logs DROP COLUMN status;
