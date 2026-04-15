ALTER TABLE ont_messages
ADD COLUMN sender_key TEXT;

CREATE INDEX idx_ont_messages_sender_key
ON ont_messages (user_id, platform, sender_key, created_at DESC)
WHERE sender_key IS NOT NULL;
