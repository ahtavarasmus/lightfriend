-- Your SQL goes here
CREATE TABLE usage_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    activity_type TEXT NOT NULL,  -- 'sms' or 'call'
    iq_used INTEGER NOT NULL,
    iq_cost_per_euro INTEGER NOT NULL, 
    created_at INTEGER NOT NULL,   -- Unix timestamp
    success BOOLEAN NOT NULL, 
    summary TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
