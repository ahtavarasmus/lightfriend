-- Your SQL goes here
CREATE TABLE imap_connection (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    method TEXT NOT NULL,
    encrypted_password TEXT NOT NULL,
    status TEXT NOT NULL,
    last_update INTEGER NOT NULL,
    created_on INTEGER NOT NULL,
    description TEXT NOT NULL,
    expires_in INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
