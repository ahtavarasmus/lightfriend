-- Remove source_lookback_hours from tasks table
-- SQLite 3.35+ supports ALTER TABLE DROP COLUMN
-- The column is no longer needed - lookback is calculated dynamically
-- based on time since last completed task of same action type

ALTER TABLE tasks DROP COLUMN source_lookback_hours;
