-- Your SQL goes here
CREATE TABLE unipile_connection (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    account_type TEXT NOT NULL,
    account_id TEXT NOT NULL,
    status TEXT NOT NULL,
    last_update INTEGER NOT NULL,
    created_on INTEGER NOT NULL,
    description TEXT NOT NULL,
    connected_account_id TEXT,
    provider TEXT
);
DROP TABLE IF EXISTS calendar_connection;
