CREATE TABLE byot_verifications (
    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    phone_number TEXT NOT NULL,
    phone_sid TEXT,
    status TEXT NOT NULL
        CHECK (status IN ('configuring', 'verified', 'error', 'drifted')),
    attempt_id TEXT NOT NULL,
    configured_at INT4,
    verified_at INT4,
    last_checked_at INT4 NOT NULL,
    error_code TEXT,
    updated_at INT4 NOT NULL
);

CREATE INDEX byot_verifications_status_idx
    ON byot_verifications (status, last_checked_at);

-- Existing enabled rows predate read-back verification. Fail closed and
-- surface a retryable setup state instead of treating legacy enablement as
-- proof that both current callbacks and capabilities are valid.
INSERT INTO byot_verifications (
    user_id,
    phone_number,
    status,
    attempt_id,
    last_checked_at,
    error_code,
    updated_at
)
SELECT
    id,
    COALESCE(preferred_number, ''),
    'error',
    md5(random()::text || clock_timestamp()::text || id::text),
    EXTRACT(EPOCH FROM NOW())::INT4,
    'verification_required',
    EXTRACT(EPOCH FROM NOW())::INT4
FROM users
WHERE own_twilio_enabled = TRUE;

UPDATE users
SET own_twilio_enabled = FALSE
WHERE own_twilio_enabled = TRUE;
