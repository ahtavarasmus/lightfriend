-- One-shot watches: when the AI sends a message on the user's behalf with
-- notify_on_reply = true, we arm a row here. The first matching inbound
-- message within the TTL forwards a notification to the user and deletes
-- the row. No matching message within the TTL = silent expiry.
--
-- Polymorphic across bridge (Matrix: WhatsApp/Telegram/Signal) and email.
-- Bridge rows key the match on room_id. Email rows key on
-- (imap_connection_id, contact_identifier).
CREATE TABLE IF NOT EXISTS pending_reply_watches (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    platform TEXT NOT NULL,
    room_id TEXT,
    imap_connection_id INTEGER,
    contact_identifier TEXT NOT NULL,
    contact_display_name TEXT NOT NULL,
    created_at INT4 NOT NULL,
    expires_at INT4 NOT NULL
);

CREATE INDEX IF NOT EXISTS pending_reply_watches_bridge_idx
    ON pending_reply_watches (user_id, room_id)
    WHERE platform = 'bridge';

CREATE INDEX IF NOT EXISTS pending_reply_watches_email_idx
    ON pending_reply_watches (user_id, imap_connection_id, contact_identifier)
    WHERE platform = 'email';

CREATE INDEX IF NOT EXISTS pending_reply_watches_expires_idx
    ON pending_reply_watches (expires_at);
