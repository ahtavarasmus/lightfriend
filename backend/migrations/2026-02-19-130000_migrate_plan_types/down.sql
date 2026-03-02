-- Cannot perfectly reverse this migration since we lose the distinction
-- between monitor and digest users. Best effort: set all autopilot back to monitor.
UPDATE users SET plan_type = 'monitor' WHERE plan_type = 'autopilot';
