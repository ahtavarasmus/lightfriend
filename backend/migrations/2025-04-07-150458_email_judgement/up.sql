-- Your SQL goes here
CREATE TABLE email_judgments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    email_timestamp INTEGER NOT NULL,
    processed_at INTEGER NOT NULL,
    should_notify BOOLEAN NOT NULL,
    score INTEGER NOT NULL,
    reason TEXT NOT NULL
);


