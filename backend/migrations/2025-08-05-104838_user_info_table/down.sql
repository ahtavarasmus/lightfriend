-- This file should undo anything in `up.sql`
ALTER TABLE user_settings ADD COLUMN info TEXT;
ALTER TABLE user_settings ADD COLUMN timezone TEXT;

UPDATE user_settings
SET info = ui.info, timezone = ui.timezone
FROM user_info AS ui
WHERE user_settings.user_id = ui.user_id;

DROP TABLE user_info;
