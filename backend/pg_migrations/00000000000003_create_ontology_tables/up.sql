-- Ontology v1: Person + Channel model
CREATE TABLE IF NOT EXISTS ont_persons (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ont_persons_user_id ON ont_persons(user_id);

CREATE TABLE IF NOT EXISTS ont_person_edits (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    person_id INTEGER NOT NULL REFERENCES ont_persons(id) ON DELETE CASCADE,
    property_name TEXT NOT NULL,
    value TEXT NOT NULL,
    edited_at INTEGER NOT NULL,
    UNIQUE(user_id, person_id, property_name)
);

CREATE TABLE IF NOT EXISTS ont_channels (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    person_id INTEGER NOT NULL REFERENCES ont_persons(id) ON DELETE CASCADE,
    platform TEXT NOT NULL,
    handle TEXT,
    room_id TEXT,
    notification_mode TEXT NOT NULL DEFAULT 'default',
    notification_type TEXT NOT NULL DEFAULT 'sms',
    notify_on_call INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ont_channels_user_id ON ont_channels(user_id);
CREATE INDEX IF NOT EXISTS idx_ont_channels_room_id ON ont_channels(room_id);
CREATE INDEX IF NOT EXISTS idx_ont_channels_person_id ON ont_channels(person_id);

CREATE TABLE IF NOT EXISTS ont_changelog (
    id BIGSERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id INTEGER NOT NULL,
    change_type TEXT NOT NULL,
    changed_fields TEXT,
    source TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ont_changelog_user_id ON ont_changelog(user_id);

-- Ontology v2: Links between entities
CREATE TABLE IF NOT EXISTS ont_links (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    source_type TEXT NOT NULL,
    source_id INTEGER NOT NULL,
    target_type TEXT NOT NULL,
    target_id INTEGER NOT NULL,
    link_type TEXT NOT NULL,
    metadata TEXT,
    created_at INTEGER NOT NULL,
    UNIQUE(user_id, source_type, source_id, target_type, target_id, link_type)
);
CREATE INDEX IF NOT EXISTS idx_ont_links_user_id ON ont_links(user_id);
CREATE INDEX IF NOT EXISTS idx_ont_links_source ON ont_links(source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_ont_links_target ON ont_links(target_type, target_id);
