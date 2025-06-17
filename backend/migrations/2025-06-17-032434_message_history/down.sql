-- This file should undo anything in `up.sql`
DROP INDEX IF EXISTS idx_message_history_created_at;
DROP INDEX IF EXISTS idx_message_history_user_conversation;
DROP TABLE IF EXISTS message_history;
