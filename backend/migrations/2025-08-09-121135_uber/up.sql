-- Your SQL goes here
CREATE TABLE IF NOT EXISTS uber (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL UNIQUE,
    encrypted_access_token TEXT NOT NULL,
    encrypted_refresh_token TEXT NOT NULL,
    status TEXT NOT NULL,
    last_update INTEGER NOT NULL,
    created_on INTEGER NOT NULL,
    description TEXT NOT NULL,
    expires_in INTEGER NOT NULL
);
