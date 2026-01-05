-- Add permanence and recurrence fields to tasks table
ALTER TABLE tasks ADD COLUMN is_permanent INTEGER DEFAULT 0;
ALTER TABLE tasks ADD COLUMN recurrence_rule TEXT;       -- "daily", "weekly:1,3,5", "monthly:15"
ALTER TABLE tasks ADD COLUMN recurrence_time TEXT;       -- "09:00" (HH:MM in user timezone)
