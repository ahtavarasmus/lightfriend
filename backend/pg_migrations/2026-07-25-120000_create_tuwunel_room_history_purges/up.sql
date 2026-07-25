-- Durable room-level purge ledger for Matrix portal rooms discovered directly
-- from connected bridge sessions. This covers Tuwunel-only rooms which have no
-- corresponding ontology message boundary.
CREATE TABLE IF NOT EXISTS tuwunel_room_history_purges (
    id SERIAL PRIMARY KEY,
    user_id INT4 NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    service TEXT NOT NULL,
    room_id TEXT NOT NULL,
    cutoff_ts INT4 NOT NULL,
    submitted_cutoff_ts INT4,
    purge_id TEXT,
    status TEXT NOT NULL,
    attempt_count INT4 NOT NULL DEFAULT 0,
    last_error TEXT,
    last_discovered_at INT4 NOT NULL,
    last_attempted_at INT4,
    completed_at INT4,
    updated_at INT4 NOT NULL,
    UNIQUE (user_id, service, room_id)
);

CREATE INDEX IF NOT EXISTS idx_tuwunel_room_history_purges_status
    ON tuwunel_room_history_purges (status, updated_at);

CREATE INDEX IF NOT EXISTS idx_tuwunel_room_history_purges_discovery
    ON tuwunel_room_history_purges (user_id, service, last_discovered_at);

CREATE TABLE IF NOT EXISTS tuwunel_portal_census_scans (
    user_id INT4 NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    service TEXT NOT NULL,
    status TEXT NOT NULL,
    room_count INT4 NOT NULL DEFAULT 0,
    room_cursor TEXT,
    last_error TEXT,
    last_scanned_at INT4 NOT NULL,
    PRIMARY KEY (user_id, service)
);

CREATE INDEX IF NOT EXISTS idx_tuwunel_portal_census_scans_time
    ON tuwunel_portal_census_scans (last_scanned_at);
