-- Add llm_provider setting to user_settings table
-- Values: "openai" (default) or "tinfoil"
ALTER TABLE user_settings ADD COLUMN llm_provider TEXT DEFAULT 'openai';
