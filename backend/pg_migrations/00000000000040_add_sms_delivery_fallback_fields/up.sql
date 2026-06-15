ALTER TABLE message_status_log ADD COLUMN encrypted_body TEXT;
ALTER TABLE message_status_log ADD COLUMN fallback_provider TEXT;
ALTER TABLE message_status_log ADD COLUMN fallback_attempted_at INTEGER;
ALTER TABLE message_status_log ADD COLUMN fallback_message_sid TEXT;
ALTER TABLE message_status_log ADD COLUMN fallback_error TEXT;

CREATE INDEX idx_message_status_log_fallback_message_sid
    ON message_status_log(fallback_message_sid);
