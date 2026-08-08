ALTER TABLE user_info
    ADD COLUMN timezone_updated_at INT4;

UPDATE user_info
SET timezone_updated_at = EXTRACT(EPOCH FROM NOW())::INT4
WHERE timezone IS NOT NULL AND timezone_updated_at IS NULL;

ALTER TABLE ont_events
    ADD COLUMN reminder_attempts INT4 NOT NULL DEFAULT 0,
    ADD COLUMN reminder_next_attempt_at INT4,
    ADD COLUMN reminder_lease_until INT4,
    ADD COLUMN reminder_last_error TEXT,
    ADD COLUMN reminder_delivered_at INT4,
    ADD COLUMN reminder_delivery_key TEXT,
    ADD COLUMN reminder_timezone TEXT;

CREATE UNIQUE INDEX ont_events_reminder_delivery_key_idx
    ON ont_events (reminder_delivery_key)
    WHERE reminder_delivery_key IS NOT NULL;

CREATE INDEX ont_events_reminder_claim_idx
    ON ont_events (status, reminder_next_attempt_at, reminder_lease_until, remind_at);

CREATE TABLE scheduler_health (
    job_name TEXT PRIMARY KEY,
    last_started_at INT4 NOT NULL,
    last_completed_at INT4,
    last_error TEXT,
    updated_at INT4 NOT NULL
);
