ALTER TABLE billing_accounts
    ADD COLUMN legacy_overage_preference_migrated BOOLEAN NOT NULL DEFAULT FALSE;
