-- One-time retention catch-up for ontology message rows.
--
-- The application scheduler already purges ont_messages older than 30 days
-- daily, but this migration forces the same retention boundary during deploy
-- so old rows are removed even if the scheduler has not run recently.
DELETE FROM ont_messages
WHERE created_at < (EXTRACT(EPOCH FROM NOW())::INT4 - (30 * 24 * 60 * 60));

ANALYZE ont_messages;
