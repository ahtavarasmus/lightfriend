ALTER TABLE users
    DROP COLUMN IF EXISTS refresh_token_hash,
    DROP COLUMN IF EXISTS refresh_token_compromised,
    DROP COLUMN IF EXISTS magic_token_expires_at;
