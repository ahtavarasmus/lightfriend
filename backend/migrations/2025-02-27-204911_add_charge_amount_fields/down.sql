-- This file should undo anything in `up.sql`
ALTER TABLE users DROP COLUMN charge_when_under;
ALTER TABLE users DROP COLUMN charge_back_to;
