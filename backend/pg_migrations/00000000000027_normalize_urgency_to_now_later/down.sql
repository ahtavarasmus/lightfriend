-- Reverse: there is no faithful inverse since the original 4-tier signal
-- (critical/high vs medium/low) is lost in the up-migration. The best we
-- can do is map "now" → "high" and "later" → "low" so the column type
-- and rough semantics survive a rollback.
UPDATE ont_messages SET urgency = 'high' WHERE urgency = 'now';
UPDATE ont_messages SET urgency = 'low'  WHERE urgency = 'later';
