-- Your SQL goes here
CREATE TABLE critical_categories (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    category_name TEXT NOT NULL,
    definition TEXT,
    active BOOLEAN NOT NULL
);
