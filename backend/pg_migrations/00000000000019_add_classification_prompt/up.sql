ALTER TABLE ont_messages ADD COLUMN IF NOT EXISTS classification_prompt TEXT;
ALTER TABLE ont_messages ADD COLUMN IF NOT EXISTS classification_result TEXT;
