-- Add price tracking columns to message_status_log
ALTER TABLE message_status_log ADD COLUMN price REAL;
ALTER TABLE message_status_log ADD COLUMN price_unit TEXT;
