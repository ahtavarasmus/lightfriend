-- Your SQL goes here
CREATE TABLE calendar_notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    event_id VARCHAR NOT NULL,
    notification_time INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Create an index for faster lookups
CREATE INDEX idx_calendar_notifications_user_event ON calendar_notifications(user_id, event_id);
CREATE INDEX idx_calendar_notifications_time ON calendar_notifications(notification_time);

