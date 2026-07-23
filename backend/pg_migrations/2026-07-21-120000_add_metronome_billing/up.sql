CREATE TABLE billing_accounts (
    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    metronome_customer_id TEXT UNIQUE,
    metronome_contract_id TEXT UNIQUE,
    overage_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    overage_consent_at INT4,
    overage_consent_version TEXT,
    payment_ready BOOLEAN NOT NULL DEFAULT FALSE,
    usage_entitled BOOLEAN NOT NULL DEFAULT TRUE,
    provisioning_status TEXT NOT NULL DEFAULT 'pending'
        CHECK (provisioning_status IN ('pending', 'provisioned', 'failed')),
    provisioning_error TEXT,
    legacy_credit_migrated BOOLEAN NOT NULL DEFAULT FALSE,
    created_at INT4 NOT NULL,
    updated_at INT4 NOT NULL
);

CREATE TABLE billing_usage_events (
    transaction_id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    cost_microusd BIGINT NOT NULL CHECK (cost_microusd > 0),
    occurred_at INT4 NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'sending', 'sent', 'failed')),
    attempts INT4 NOT NULL DEFAULT 0,
    next_attempt_at INT4 NOT NULL,
    last_error TEXT,
    created_at INT4 NOT NULL,
    sent_at INT4
);

CREATE TABLE billing_webhook_events (
    event_id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    received_at INT4 NOT NULL
);

CREATE INDEX billing_accounts_provisioning_idx
    ON billing_accounts (provisioning_status, updated_at);

CREATE INDEX billing_usage_events_pending_idx
    ON billing_usage_events (status, next_attempt_at, created_at)
    WHERE status IN ('pending', 'failed', 'sending');
