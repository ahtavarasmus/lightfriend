CREATE TABLE temporary_alert_suppressions (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('topic', 'quiet')),
    scope TEXT NOT NULL CHECK (scope IN ('all', 'critical', 'digest')),
    match_text TEXT,
    timezone TEXT NOT NULL,
    created_at INT4 NOT NULL,
    expires_at INT4 NOT NULL,
    ended_at INT4,
    CHECK (expires_at > created_at),
    CHECK ((kind = 'topic' AND match_text IS NOT NULL) OR kind = 'quiet')
);

CREATE INDEX temporary_alert_suppressions_active_idx
    ON temporary_alert_suppressions (user_id, expires_at)
    WHERE ended_at IS NULL;
