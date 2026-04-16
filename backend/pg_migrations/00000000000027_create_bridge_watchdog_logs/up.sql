CREATE TABLE bridge_watchdog_logs (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    bridge_type TEXT NOT NULL,
    event_type TEXT NOT NULL,
    message TEXT NOT NULL,
    metadata TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_bwl_user_bridge ON bridge_watchdog_logs (user_id, bridge_type);
CREATE INDEX idx_bwl_created_at ON bridge_watchdog_logs (created_at);
CREATE INDEX idx_bwl_event_type ON bridge_watchdog_logs (event_type);
