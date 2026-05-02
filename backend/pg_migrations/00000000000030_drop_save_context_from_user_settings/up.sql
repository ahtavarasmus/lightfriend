-- Conversation history is now bounded by time (last 48h) and a fixed
-- message cap, not a per-user limit. The save_context column is no
-- longer read or written.
ALTER TABLE user_settings DROP COLUMN save_context;
