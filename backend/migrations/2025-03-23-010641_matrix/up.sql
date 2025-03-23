-- Your SQL goes here
alter table users add column matrix_username TEXT;
alter table users add column encrypted_matrix_access_token TEXT;

CREATE TABLE bridges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    bridge_type TEXT NOT NULL,
    status TEXT NOT NULL,
    room_id TEXT,
    data TEXT,
    created_at INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
