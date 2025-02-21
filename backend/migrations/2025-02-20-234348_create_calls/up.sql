-- Your SQL goes here
CREATE TABLE calls (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    conversation_id INTEGER NOT NULL,
    status TEXT NOT NULL,
    analysis TEXT,
    call_duration_secs INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (conversation_id) REFERENCES conversations(id)
);
