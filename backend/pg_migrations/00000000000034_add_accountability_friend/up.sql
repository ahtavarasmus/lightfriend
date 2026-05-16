ALTER TABLE users ADD COLUMN IF NOT EXISTS accountability_friend_phone TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS accountability_friend_name TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS accountability_enabled BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE ont_events ADD COLUMN IF NOT EXISTS friend_notified_at INT4;
