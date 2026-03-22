CREATE TABLE IF NOT EXISTS ont_messages (
    id BIGSERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    room_id TEXT NOT NULL,
    platform TEXT NOT NULL,
    sender_name TEXT NOT NULL,
    content TEXT NOT NULL,
    person_id INTEGER,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ont_messages_user ON ont_messages(user_id);
CREATE INDEX IF NOT EXISTS idx_ont_messages_user_room ON ont_messages(user_id, room_id);
CREATE INDEX IF NOT EXISTS idx_ont_messages_user_platform_created ON ont_messages(user_id, platform, created_at);
CREATE INDEX IF NOT EXISTS idx_ont_messages_created ON ont_messages(created_at);
