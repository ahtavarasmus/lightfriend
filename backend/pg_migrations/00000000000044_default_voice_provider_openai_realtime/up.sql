ALTER TABLE user_settings
ALTER COLUMN voice_provider SET DEFAULT 'openai_realtime';

UPDATE user_settings
SET voice_provider = 'openai_realtime'
WHERE voice_provider = 'tinfoil';
