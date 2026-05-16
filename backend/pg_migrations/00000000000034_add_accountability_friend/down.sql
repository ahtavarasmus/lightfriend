ALTER TABLE ont_events DROP COLUMN IF EXISTS friend_notified_at;

ALTER TABLE users DROP COLUMN IF EXISTS accountability_enabled;
ALTER TABLE users DROP COLUMN IF EXISTS accountability_friend_name;
ALTER TABLE users DROP COLUMN IF EXISTS accountability_friend_phone;
