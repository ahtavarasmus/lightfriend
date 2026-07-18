-- Durable cleanup for Matrix/Tuwunel data left behind by disconnected bridges.
-- These rows never own ontology content; they only track homeserver cleanup.
CREATE TABLE IF NOT EXISTS bridge_cleanup_jobs (
    id SERIAL PRIMARY KEY,
    user_id INT4 NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    bridge_type TEXT NOT NULL,
    trigger_kind TEXT NOT NULL,
    expected_bridge_id INT4,
    expected_bridge_created_at INT4,
    management_room_id TEXT,
    status TEXT NOT NULL,
    attempt_count INT4 NOT NULL DEFAULT 0,
    not_before INT4 NOT NULL,
    last_error TEXT,
    created_at INT4 NOT NULL,
    updated_at INT4 NOT NULL,
    completed_at INT4
);

CREATE INDEX IF NOT EXISTS idx_bridge_cleanup_jobs_due
    ON bridge_cleanup_jobs (status, not_before, updated_at);

CREATE UNIQUE INDEX IF NOT EXISTS idx_bridge_cleanup_jobs_connection
    ON bridge_cleanup_jobs (user_id, bridge_type, expected_bridge_id)
    WHERE expected_bridge_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_bridge_cleanup_jobs_open_orphan
    ON bridge_cleanup_jobs (user_id, bridge_type)
    WHERE expected_bridge_id IS NULL
      AND status IN ('audit_pending', 'audit_ready', 'retrying');

CREATE TABLE IF NOT EXISTS bridge_cleanup_rooms (
    id SERIAL PRIMARY KEY,
    job_id INT4 NOT NULL REFERENCES bridge_cleanup_jobs(id) ON DELETE CASCADE,
    room_id TEXT NOT NULL,
    source TEXT NOT NULL,
    status TEXT NOT NULL,
    attempt_count INT4 NOT NULL DEFAULT 0,
    delete_id TEXT,
    last_error TEXT,
    discovered_at INT4 NOT NULL,
    updated_at INT4 NOT NULL,
    completed_at INT4,
    UNIQUE (job_id, room_id)
);

CREATE INDEX IF NOT EXISTS idx_bridge_cleanup_rooms_job_status
    ON bridge_cleanup_rooms (job_id, status, updated_at);
