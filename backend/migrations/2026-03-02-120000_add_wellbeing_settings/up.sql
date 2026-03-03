ALTER TABLE user_settings ADD COLUMN dumbphone_mode_on INTEGER NOT NULL DEFAULT 0;
ALTER TABLE user_settings ADD COLUMN notification_calmer_on INTEGER NOT NULL DEFAULT 0;
ALTER TABLE user_settings ADD COLUMN notification_calmer_schedule TEXT;
ALTER TABLE user_settings ADD COLUMN wellbeing_signup_timestamp INTEGER;
