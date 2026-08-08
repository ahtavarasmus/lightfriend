DROP TABLE IF EXISTS billing_usage_intents;

DROP INDEX IF EXISTS billing_webhook_claim_idx;

ALTER TABLE billing_webhook_events
    DROP COLUMN IF EXISTS last_error,
    DROP COLUMN IF EXISTS processed_at,
    DROP COLUMN IF EXISTS lease_until,
    DROP COLUMN IF EXISTS attempts,
    DROP COLUMN IF EXISTS status;

ALTER TABLE billing_usage_events
    DROP COLUMN IF EXISTS invoice_visible,
    DROP COLUMN IF EXISTS provider_status,
    DROP COLUMN IF EXISTS provider_reconciled_at;
