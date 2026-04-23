-- Collapse the legacy 4-tier urgency enum (critical/high/medium/low) into
-- the 2-tier model (now/later) used by the classifier and digest scheduler.
-- The classifier itself only emits "now" or "later" going forward; this
-- migration normalizes any rows that were written before the cutover so
-- they remain visible to the new digest queries.
UPDATE ont_messages SET urgency = 'now'   WHERE urgency IN ('critical', 'high');
UPDATE ont_messages SET urgency = 'later' WHERE urgency IN ('medium', 'low');
