ALTER TABLE ont_messages ADD COLUMN urgency TEXT;
ALTER TABLE ont_messages ADD COLUMN category TEXT;
ALTER TABLE ont_messages ADD COLUMN summary TEXT;
ALTER TABLE ont_messages ADD COLUMN digest_delivered_at INTEGER;

ALTER TABLE user_settings ADD COLUMN digest_enabled BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE user_settings ADD COLUMN digest_time TEXT;
