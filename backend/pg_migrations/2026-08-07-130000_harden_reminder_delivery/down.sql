DROP TABLE IF EXISTS scheduler_health;
DROP INDEX IF EXISTS ont_events_reminder_claim_idx;
DROP INDEX IF EXISTS ont_events_reminder_delivery_key_idx;
ALTER TABLE ont_events
    DROP COLUMN IF EXISTS reminder_timezone,
    DROP COLUMN IF EXISTS reminder_delivery_key,
    DROP COLUMN IF EXISTS reminder_delivered_at,
    DROP COLUMN IF EXISTS reminder_last_error,
    DROP COLUMN IF EXISTS reminder_lease_until,
    DROP COLUMN IF EXISTS reminder_next_attempt_at,
    DROP COLUMN IF EXISTS reminder_attempts;
ALTER TABLE user_info DROP COLUMN IF EXISTS timezone_updated_at;
