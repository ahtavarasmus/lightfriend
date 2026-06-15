ALTER TABLE users
    ADD COLUMN own_twilio_enabled BOOLEAN NOT NULL DEFAULT FALSE;

UPDATE users
SET own_twilio_enabled = TRUE,
    plan_type = 'autopilot'
WHERE plan_type = 'byot';
