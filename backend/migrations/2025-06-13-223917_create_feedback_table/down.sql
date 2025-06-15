-- This file should undo anything in `up.sql`
-- Drop indexes first
DROP INDEX IF EXISTS idx_ideas_creator_id;
DROP INDEX IF EXISTS idx_idea_upvotes_idea_id;
DROP INDEX IF EXISTS idx_idea_upvotes_voter_id;
DROP INDEX IF EXISTS idx_idea_email_subscriptions_idea_email;

-- Drop tables in reverse order of creation
DROP TABLE IF EXISTS idea_email_subscriptions;
DROP TABLE IF EXISTS idea_upvotes;
DROP TABLE IF EXISTS ideas;
