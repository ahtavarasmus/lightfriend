-- Your SQL goes here
CREATE TABLE user_info (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    location TEXT,
    dictionary TEXT,
    info TEXT,
    timezone TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

INSERT INTO user_info (user_id, info, timezone)
SELECT user_id, info, timezone FROM user_settings;

ALTER TABLE user_settings DROP COLUMN info;
ALTER TABLE user_settings DROP COLUMN timezone;
