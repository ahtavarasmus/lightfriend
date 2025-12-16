-- Set plan_type = 'monitor' for existing tier 2 subscribers who are outside US/CA
-- These users were subscribed before plan_type was tracked, so we need to backfill
UPDATE users
SET plan_type = 'monitor'
WHERE sub_tier = 'tier 2'
  AND plan_type IS NULL
  AND (phone_number_country IS NULL OR phone_number_country NOT IN ('US', 'CA'));
