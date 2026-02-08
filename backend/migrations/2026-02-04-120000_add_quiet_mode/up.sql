-- Add quiet_mode_until column to user_settings
-- NULL = notifications active (not quiet)
-- 0 = indefinite quiet mode
-- >0 = quiet until that unix timestamp
ALTER TABLE user_settings ADD COLUMN quiet_mode_until INTEGER;
