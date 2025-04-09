-- Your SQL goes here
CREATE TABLE processed_emails (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    email_uid VARCHAR(255) NOT NULL,
    processed_at INTEGER NOT NULL,
    UNIQUE (user_id, email_uid),
    FOREIGN KEY (user_id) REFERENCES users(id)
);
