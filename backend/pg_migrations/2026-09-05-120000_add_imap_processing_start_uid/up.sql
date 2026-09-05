-- The UID that existed when an inbox was connected (or reconnected).
-- Messages at or below this boundary are historical and must never enter
-- proactive processing. NULL is retained only until an existing connection
-- is first observed by a worker, which initializes it without importing mail.
ALTER TABLE imap_connection
ADD COLUMN processing_start_uid BIGINT;

-- Preserve continuity for existing connections that already have processing
-- history. Connections without any history are initialized against the live
-- mailbox by the worker so deployment cannot backfill their inboxes.
UPDATE imap_connection AS connection
SET processing_start_uid = processed.max_uid
FROM (
    SELECT
        imap_connection_id,
        MAX(CAST(email_uid AS BIGINT)) AS max_uid
    FROM processed_emails
    WHERE imap_connection_id IS NOT NULL
      AND email_uid ~ '^[0-9]+$'
    GROUP BY imap_connection_id
) AS processed
WHERE connection.id = processed.imap_connection_id;
