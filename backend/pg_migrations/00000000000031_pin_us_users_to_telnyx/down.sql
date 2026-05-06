-- Clear all telnyx pins. Users fall back to country-based routing.
-- Note: this won't restore user id=1 to 'sinch' (set by migration 29);
-- if you need that back, re-run migration 29's UPDATE manually.
UPDATE users
SET preferred_sms_provider = NULL
WHERE preferred_sms_provider = 'telnyx';
