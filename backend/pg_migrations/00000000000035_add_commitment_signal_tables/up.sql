-- Per-user sender rules driving commitment detection.
-- rule_type: 'mute' (skip detection entirely) or 'always_track' (auto-create
-- ont_event without SMS prompt).
CREATE TABLE IF NOT EXISTS commitment_sender_rules (
    id SERIAL PRIMARY KEY,
    user_id INT4 NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    platform TEXT NOT NULL,
    sender_key TEXT NOT NULL,
    rule_type TEXT NOT NULL,
    source TEXT NOT NULL,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at INT4 NOT NULL,
    deactivated_at INT4
);

CREATE INDEX IF NOT EXISTS idx_commitment_sender_rules_lookup
    ON commitment_sender_rules (user_id, platform, sender_key, active);

-- Per-user content similarity memory for label-similarity detection.
-- label_type: 'track' (reply 1 or 2) or 'wrong' (reply 4). 'mute' (3) does
-- not seed embeddings because it's a sender preference, not a content signal.
-- embedding stores raw little-endian f32 bytes.
CREATE TABLE IF NOT EXISTS commitment_label_embeddings (
    id SERIAL PRIMARY KEY,
    user_id INT4 NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    label_type TEXT NOT NULL,
    embedding BYTEA NOT NULL,
    source_message_id INT8,
    created_at INT4 NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_commitment_label_embeddings_user_label
    ON commitment_label_embeddings (user_id, label_type);

-- Live SMS prompts asking the user to label a detected commitment.
-- user_label: '1' track, '2' always-track, '3' mute, '4' not-a-commitment.
-- resolved_at is non-null once user replied OR prompt expired without reply.
-- resulting_event_id is the ont_event created on reply 1 or 2.
-- due_at / remind_at preserve the detector-extracted schedule so reply
-- 1 / 2 can re-create the event with the original deadline (otherwise the
-- SMS-prompt branch silently strips deadline metadata).
CREATE TABLE IF NOT EXISTS commitment_prompts (
    id SERIAL PRIMARY KEY,
    user_id INT4 NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ont_message_id INT8 NOT NULL,
    platform TEXT NOT NULL,
    sender_key TEXT NOT NULL,
    sender_display_name TEXT NOT NULL,
    commitment_description TEXT NOT NULL,
    due_at INT4,
    remind_at INT4,
    sent_at INT4 NOT NULL,
    sms_message_sid TEXT,
    user_label TEXT,
    labeled_at INT4,
    resulting_event_id INT4,
    resolved_at INT4
);

CREATE INDEX IF NOT EXISTS idx_commitment_prompts_user_unresolved
    ON commitment_prompts (user_id, resolved_at);

-- Enforces "at most one unresolved prompt per sender per user" at the
-- database level, so concurrent message arrivals can't both pass the
-- find_unresolved_for_sender check and double-spam the user.
CREATE UNIQUE INDEX IF NOT EXISTS idx_commitment_prompts_unresolved_sender_unique
    ON commitment_prompts (user_id, platform, sender_key)
    WHERE resolved_at IS NULL;
