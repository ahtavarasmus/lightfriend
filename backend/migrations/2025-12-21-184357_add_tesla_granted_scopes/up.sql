-- Add granted_scopes column to track which scopes the user granted to Tesla OAuth
ALTER TABLE tesla ADD COLUMN granted_scopes TEXT;
