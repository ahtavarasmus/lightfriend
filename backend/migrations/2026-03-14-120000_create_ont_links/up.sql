-- Ontology v2: Links between entities

CREATE TABLE ont_links (
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
CREATE INDEX idx_ont_links_user_id ON ont_links(user_id);
CREATE INDEX idx_ont_links_source ON ont_links(source_type, source_id);
CREATE INDEX idx_ont_links_target ON ont_links(target_type, target_id);
