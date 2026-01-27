-- Re-add source_lookback_hours column
ALTER TABLE tasks ADD COLUMN source_lookback_hours INTEGER DEFAULT 24;
