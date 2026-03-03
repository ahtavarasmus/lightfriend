-- Recreate tables for Google Calendar, Calendar Notifications, and Uber

CREATE TABLE google_calendar (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    encrypted_access_token TEXT NOT NULL,
    encrypted_refresh_token TEXT NOT NULL,
    status TEXT NOT NULL,
    last_update INTEGER NOT NULL,
    created_on INTEGER NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    expires_in INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE calendar_notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    event_id TEXT NOT NULL,
    notification_time INTEGER NOT NULL
);

CREATE TABLE uber (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id),
    encrypted_access_token TEXT NOT NULL,
    encrypted_refresh_token TEXT NOT NULL,
    status TEXT NOT NULL,
    last_update INTEGER NOT NULL,
    created_on INTEGER NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    expires_in INTEGER NOT NULL DEFAULT 0
);
