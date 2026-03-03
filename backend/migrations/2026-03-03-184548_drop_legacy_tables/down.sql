-- Recreate dropped tables for rollback safety

CREATE TABLE subaccounts (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL DEFAULT '-1',
    subaccount_sid TEXT NOT NULL UNIQUE,
    auth_token TEXT NOT NULL,
    country TEXT,
    number TEXT,
    cost_this_month FLOAT DEFAULT 0.0,
    created_at INTEGER,
    status TEXT,
    tinfoil_key TEXT,
    messaging_service_sid TEXT,
    subaccount_type TEXT NOT NULL DEFAULT 'us_ca',
    country_code TEXT
);

CREATE TABLE conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    user_id INTEGER NOT NULL,
    conversation_sid TEXT NOT NULL,
    service_sid TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    twilio_number TEXT NOT NULL DEFAULT '',
    user_number TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    trigger TEXT NOT NULL,
    condition TEXT,
    action TEXT NOT NULL,
    notification_type TEXT DEFAULT 'sms',
    status TEXT DEFAULT 'active',
    created_at INTEGER NOT NULL,
    completed_at INTEGER,
    is_permanent INTEGER,
    recurrence_rule TEXT,
    recurrence_time TEXT,
    sources TEXT,
    end_time INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE priority_senders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    sender TEXT NOT NULL,
    service_type TEXT NOT NULL DEFAULT 'imap',
    noti_type TEXT,
    noti_mode TEXT NOT NULL DEFAULT 'all',
    FOREIGN KEY(user_id) REFERENCES users(id)
);

CREATE TABLE keywords (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    keyword TEXT NOT NULL,
    service_type TEXT NOT NULL DEFAULT 'imap',
    FOREIGN KEY(user_id) REFERENCES users(id)
);

CREATE TABLE email_judgments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    email_timestamp INTEGER NOT NULL,
    processed_at INTEGER NOT NULL,
    should_notify BOOLEAN NOT NULL,
    score INTEGER NOT NULL,
    reason TEXT NOT NULL
);
