DROP TABLE waitlist;

-- SQLite doesn't support DROP COLUMN, so we need to recreate the table
-- For simplicity, we'll just leave the magic_token column (it's nullable and won't cause issues)
-- In production, you'd need to recreate the users table without the column
