-- Add sources and source_lookback_hours columns to tasks table
ALTER TABLE tasks ADD COLUMN sources TEXT;
ALTER TABLE tasks ADD COLUMN source_lookback_hours INTEGER DEFAULT 24;
