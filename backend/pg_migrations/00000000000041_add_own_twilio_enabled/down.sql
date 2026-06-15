UPDATE users
SET plan_type = 'byot'
WHERE own_twilio_enabled = TRUE
  AND plan_type = 'autopilot';

ALTER TABLE users
    DROP COLUMN IF EXISTS own_twilio_enabled;
