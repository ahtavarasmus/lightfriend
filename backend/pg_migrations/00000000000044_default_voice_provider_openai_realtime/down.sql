ALTER TABLE user_settings
ALTER COLUMN voice_provider SET DEFAULT 'tinfoil';

-- Existing rows are intentionally left as-is. At this point we cannot
-- distinguish old defaulted Tinfoil rows from explicit user choices.
