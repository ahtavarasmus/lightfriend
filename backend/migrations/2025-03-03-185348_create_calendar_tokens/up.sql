-- Your SQL goes here
CREATE TABLE calendar_connection (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    provider TEXT NOT NULL,
    encrypted_access_token TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
