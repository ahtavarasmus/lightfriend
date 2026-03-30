ALTER TABLE ont_messages DROP COLUMN urgency;
ALTER TABLE ont_messages DROP COLUMN category;
ALTER TABLE ont_messages DROP COLUMN summary;
ALTER TABLE ont_messages DROP COLUMN digest_delivered_at;

ALTER TABLE user_settings DROP COLUMN digest_enabled;
ALTER TABLE user_settings DROP COLUMN digest_time;
