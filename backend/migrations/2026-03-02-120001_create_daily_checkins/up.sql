CREATE TABLE daily_checkins (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    checkin_date TEXT NOT NULL,
    mood INTEGER NOT NULL,
    energy INTEGER NOT NULL,
    sleep_quality INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX idx_checkins_user_date ON daily_checkins(user_id, checkin_date);
