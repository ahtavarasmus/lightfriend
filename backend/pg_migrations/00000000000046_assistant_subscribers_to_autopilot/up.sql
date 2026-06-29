UPDATE users
SET plan_type = 'autopilot'
WHERE plan_type = 'assistant'
  AND sub_tier = 'tier 2';
