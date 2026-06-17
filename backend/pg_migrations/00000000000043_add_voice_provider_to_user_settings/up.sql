ALTER TABLE user_settings
ADD COLUMN IF NOT EXISTS voice_provider TEXT NOT NULL DEFAULT 'tinfoil';
