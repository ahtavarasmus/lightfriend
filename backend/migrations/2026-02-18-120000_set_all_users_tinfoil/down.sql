-- Revert to openai as default provider
UPDATE user_settings SET llm_provider = 'openai';
