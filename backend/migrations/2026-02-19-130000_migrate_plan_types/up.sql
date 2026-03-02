-- Migrate existing plan_type values to new tier names
-- digest -> autopilot, monitor -> autopilot
-- NULL with active sub (tier 2) -> autopilot (US/CA users had NULL plan_type)
UPDATE users SET plan_type = 'autopilot' WHERE plan_type = 'digest';
UPDATE users SET plan_type = 'autopilot' WHERE plan_type = 'monitor';
UPDATE users SET plan_type = 'autopilot' WHERE plan_type IS NULL AND sub_tier = 'tier 2';
