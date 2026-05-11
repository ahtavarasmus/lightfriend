ALTER TABLE ont_messages ADD COLUMN IF NOT EXISTS commitment_prompt TEXT;
ALTER TABLE ont_messages ADD COLUMN IF NOT EXISTS commitment_result TEXT;
