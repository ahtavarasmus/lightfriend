-- Add stripe_customer_id column to users table
ALTER TABLE users ADD COLUMN stripe_customer_id TEXT;
