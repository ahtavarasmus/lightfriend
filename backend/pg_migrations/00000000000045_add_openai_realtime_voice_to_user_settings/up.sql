ALTER TABLE user_settings
ADD COLUMN IF NOT EXISTS openai_realtime_voice TEXT NOT NULL DEFAULT 'marin';
