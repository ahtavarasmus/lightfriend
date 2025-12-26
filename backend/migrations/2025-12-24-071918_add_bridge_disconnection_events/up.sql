CREATE TABLE bridge_disconnection_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    bridge_type TEXT NOT NULL,
    detected_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
