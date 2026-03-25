ALTER TABLE users
    DROP COLUMN IF EXISTS magic_token_expires_at,
    DROP COLUMN IF EXISTS refresh_token_compromised,
    DROP COLUMN IF EXISTS refresh_token_hash;
