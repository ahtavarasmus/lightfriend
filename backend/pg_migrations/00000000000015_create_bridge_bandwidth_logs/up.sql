CREATE TABLE bridge_bandwidth_logs (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    bridge_type TEXT NOT NULL,
    direction TEXT NOT NULL,
    bytes_estimate INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_bridge_bandwidth_user_id ON bridge_bandwidth_logs(user_id);
CREATE INDEX idx_bridge_bandwidth_created_at ON bridge_bandwidth_logs(created_at);
