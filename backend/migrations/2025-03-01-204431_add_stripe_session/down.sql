-- This file should undo anything in `up.sql`
ALTER TABLE users DROP COLUMN stripe_checkout_session_id;
