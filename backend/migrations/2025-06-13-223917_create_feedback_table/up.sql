-- Your SQL goes here
-- Create ideas table
CREATE TABLE ideas (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    creator_id TEXT NOT NULL, -- Anonymous identifier for the creator
    text TEXT NOT NULL,
    created_at INTEGER NOT NULL -- Unix timestamp
);

-- Create upvotes table
CREATE TABLE idea_upvotes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    idea_id INTEGER NOT NULL,
    voter_id TEXT NOT NULL, -- Anonymous identifier for the voter
    created_at INTEGER NOT NULL, -- Unix timestamp
    FOREIGN KEY (idea_id) REFERENCES ideas(id) ON DELETE CASCADE
);

-- Create email subscriptions table
CREATE TABLE idea_email_subscriptions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    idea_id INTEGER NOT NULL,
    email TEXT NOT NULL,
    created_at INTEGER NOT NULL, -- Unix timestamp
    FOREIGN KEY (idea_id) REFERENCES ideas(id) ON DELETE CASCADE
);

-- Create indexes
CREATE INDEX idx_ideas_creator_id ON ideas(creator_id);
CREATE INDEX idx_idea_upvotes_idea_id ON idea_upvotes(idea_id);
CREATE INDEX idx_idea_upvotes_voter_id ON idea_upvotes(voter_id);
CREATE UNIQUE INDEX idx_idea_email_subscriptions_idea_email ON idea_email_subscriptions(idea_id, email);
