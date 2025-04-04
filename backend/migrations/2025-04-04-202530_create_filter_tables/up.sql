-- Your SQL goes here
CREATE TABLE waiting_checks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    waiting_type TEXT NOT NULL,
    due_date INTEGER NOT NULL, -- UTC epoch timestamp
    content TEXT NOT NULL,
    remove_when_found BOOLEAN NOT NULL DEFAULT true,
    FOREIGN KEY(user_id) REFERENCES users(id)
);

CREATE TABLE priority_senders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    sender TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id)
);

CREATE TABLE keywords (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    keyword TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id)
);

CREATE TABLE importance_priorities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    importance_type TEXT NOT NULL,
    threshold INTEGER NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id)
);
