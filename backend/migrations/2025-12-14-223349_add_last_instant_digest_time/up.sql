-- Add last_instant_digest_time to track when user last fetched on-demand digest
ALTER TABLE user_settings ADD COLUMN last_instant_digest_time INTEGER;
