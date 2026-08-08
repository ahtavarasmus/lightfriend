ALTER TABLE billing_usage_events
    ADD COLUMN provider_reconciled_at INT4,
    ADD COLUMN provider_status TEXT NOT NULL DEFAULT 'unverified'
        CHECK (provider_status IN ('unverified', 'accepted', 'matched', 'unmatched', 'missing')),
    ADD COLUMN invoice_visible BOOLEAN NOT NULL DEFAULT FALSE;

-- Older code persisted raw provider responses. Retain an actionable category
-- without carrying provider/customer payloads forward.
UPDATE billing_usage_events
SET last_error = 'legacy_provider_error_redacted'
WHERE last_error IS NOT NULL;

UPDATE billing_accounts
SET provisioning_error = 'legacy_provider_error_redacted'
WHERE provisioning_error IS NOT NULL;

ALTER TABLE billing_webhook_events
    ADD COLUMN status TEXT NOT NULL DEFAULT 'processed'
        CHECK (status IN ('processing', 'processed', 'failed')),
    ADD COLUMN attempts INT4 NOT NULL DEFAULT 1,
    ADD COLUMN lease_until INT4,
    ADD COLUMN processed_at INT4,
    ADD COLUMN last_error TEXT;

UPDATE billing_webhook_events
SET processed_at = received_at
WHERE processed_at IS NULL;

CREATE INDEX billing_webhook_claim_idx
    ON billing_webhook_events (status, lease_until)
    WHERE status <> 'processed';

CREATE TABLE billing_usage_intents (
    transaction_id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'finalized', 'abandoned')),
    created_at INT4 NOT NULL,
    finalized_at INT4,
    last_error TEXT
);

CREATE INDEX billing_usage_intents_open_idx
    ON billing_usage_intents (status, created_at)
    WHERE status = 'open';
