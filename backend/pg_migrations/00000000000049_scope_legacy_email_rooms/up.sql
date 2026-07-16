-- Backfill legacy `email_<uid>` room IDs only when an existing processed-email
-- marker proves that exactly one IMAP connection handled that UID. Cron-era
-- NULL markers remain legacy because assigning those rows to a mailbox would
-- be ambiguous for multi-account users.
WITH proven_account AS (
    SELECT
        message.id AS message_id,
        MIN(processed.imap_connection_id) AS imap_connection_id,
        SUBSTRING(message.room_id FROM 7) AS email_uid
    FROM ont_messages AS message
    INNER JOIN processed_emails AS processed
        ON processed.user_id = message.user_id
       AND processed.email_uid = SUBSTRING(message.room_id FROM 7)
       AND processed.imap_connection_id IS NOT NULL
    WHERE message.platform = 'email'
      AND message.room_id ~ '^email_[0-9]+$'
    GROUP BY message.id, message.room_id
    HAVING COUNT(DISTINCT processed.imap_connection_id) = 1
)
UPDATE ont_messages AS message
SET room_id = FORMAT(
    'email_%s_%s',
    proven.imap_connection_id,
    proven.email_uid
)
FROM proven_account AS proven
WHERE message.id = proven.message_id
  AND NOT EXISTS (
      SELECT 1
      FROM ont_messages AS scoped
      WHERE scoped.user_id = message.user_id
        AND scoped.room_id = FORMAT(
            'email_%s_%s',
            proven.imap_connection_id,
            proven.email_uid
        )
  );
