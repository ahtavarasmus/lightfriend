CREATE TABLE wellbeing_points (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    points INTEGER NOT NULL DEFAULT 0,
    current_streak INTEGER NOT NULL DEFAULT 0,
    longest_streak INTEGER NOT NULL DEFAULT 0,
    last_activity_date TEXT,
    created_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX idx_wellbeing_user ON wellbeing_points(user_id);

CREATE TABLE wellbeing_point_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    event_type TEXT NOT NULL,
    points_earned INTEGER NOT NULL,
    event_date TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
