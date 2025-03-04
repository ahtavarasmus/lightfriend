-- Your SQL goes here
ALTER TABLE usage_logs ADD COLUMN recharge_threshold_timestamp INTEGER;

ALTER TABLE usage_logs ADD COLUMN zero_credits_timestamp INTEGER;

