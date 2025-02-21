-- This file should undo anything in `up.sql`

ALTER TABLE conversations DROP COLUMN twilio_number;
