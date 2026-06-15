DROP INDEX IF EXISTS idx_message_status_log_fallback_message_sid;

ALTER TABLE message_status_log DROP COLUMN IF EXISTS fallback_error;
ALTER TABLE message_status_log DROP COLUMN IF EXISTS fallback_message_sid;
ALTER TABLE message_status_log DROP COLUMN IF EXISTS fallback_attempted_at;
ALTER TABLE message_status_log DROP COLUMN IF EXISTS fallback_provider;
ALTER TABLE message_status_log DROP COLUMN IF EXISTS encrypted_body;
