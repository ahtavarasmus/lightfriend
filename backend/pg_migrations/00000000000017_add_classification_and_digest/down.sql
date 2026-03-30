ALTER TABLE ont_messages DROP COLUMN IF EXISTS urgency;
ALTER TABLE ont_messages DROP COLUMN IF EXISTS category;
ALTER TABLE ont_messages DROP COLUMN IF EXISTS summary;
ALTER TABLE ont_messages DROP COLUMN IF EXISTS digest_delivered_at;

ALTER TABLE user_settings DROP COLUMN IF EXISTS digest_enabled;
ALTER TABLE user_settings DROP COLUMN IF EXISTS digest_time;
