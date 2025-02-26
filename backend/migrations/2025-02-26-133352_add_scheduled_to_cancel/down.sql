-- This file should undo anything in `up.sql`
ALTER TABLE subscriptions 
DROP COLUMN is_scheduled_to_cancel;
