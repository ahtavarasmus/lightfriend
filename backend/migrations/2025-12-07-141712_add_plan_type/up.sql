-- Add plan_type column to users table
-- Values: 'monitor', 'digest', or NULL for US/CA users (who don't have these plans)
ALTER TABLE users ADD COLUMN plan_type TEXT;
