ALTER TABLE bridges
    ADD COLUMN last_native_activity_at INTEGER,
    ADD COLUMN native_activity_reminded_at INTEGER;

-- Existing linked accounts have no trustworthy historical native-app signal.
-- Start their inactivity window at deploy time instead of immediately warning
-- long-lived users based on the unrelated bridge creation timestamp.
UPDATE bridges
SET last_native_activity_at = EXTRACT(EPOCH FROM NOW())::INTEGER
WHERE bridge_type = 'whatsapp';
