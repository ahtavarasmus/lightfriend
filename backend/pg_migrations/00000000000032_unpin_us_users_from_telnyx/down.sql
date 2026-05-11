-- Re-pin every US user back to Telnyx (mirrors migration 31).
-- Note: this overrides any per-user pin currently set for +1 numbers,
-- same as migration 31's up.sql does.
UPDATE users
SET preferred_sms_provider = 'telnyx'
WHERE phone_number LIKE '+1%';
