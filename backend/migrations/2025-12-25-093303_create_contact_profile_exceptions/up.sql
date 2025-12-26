-- Create contact profile exceptions table for per-platform notification overrides
CREATE TABLE contact_profile_exceptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id INTEGER NOT NULL,
    platform TEXT NOT NULL,
    notification_mode TEXT NOT NULL,
    notification_type TEXT NOT NULL DEFAULT 'sms',
    notify_on_call INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (profile_id) REFERENCES contact_profiles(id) ON DELETE CASCADE,
    UNIQUE(profile_id, platform)
);
