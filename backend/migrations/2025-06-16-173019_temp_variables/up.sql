-- Your SQL goes here
CREATE TABLE temp_variables (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    confirm_send_event_type TEXT NOT NULL,
    confirm_send_event_recipient TEXT,
    confirm_send_event_subject TEXT,
    confirm_send_event_content TEXT,
    confirm_send_event_start_time TEXT,
    confirm_send_event_duration TEXT,
    confirm_send_event_id TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
