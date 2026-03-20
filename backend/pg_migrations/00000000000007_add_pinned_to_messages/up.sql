ALTER TABLE ont_messages ADD COLUMN pinned BOOLEAN NOT NULL DEFAULT FALSE;
CREATE INDEX idx_ont_messages_user_pinned ON ont_messages(user_id, pinned) WHERE pinned = TRUE;
