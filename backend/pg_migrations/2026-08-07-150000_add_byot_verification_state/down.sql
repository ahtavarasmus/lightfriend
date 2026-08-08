UPDATE users
SET own_twilio_enabled = TRUE
FROM byot_verifications
WHERE users.id = byot_verifications.user_id
  AND byot_verifications.error_code = 'verification_required';

DROP TABLE IF EXISTS byot_verifications;
