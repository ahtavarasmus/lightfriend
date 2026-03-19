-- Ontology v3: Rules (Automation -> Logic -> Action)
CREATE TABLE IF NOT EXISTS ont_rules (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,

    -- TRIGGER: when to fire
    trigger_type TEXT NOT NULL,      -- "ontology_change" | "schedule"
    trigger_config TEXT NOT NULL,    -- JSON

    -- LOGIC: how to evaluate
    logic_type TEXT NOT NULL,        -- "llm" | "passthrough"
    logic_prompt TEXT,               -- LLM instruction (null for passthrough)
    logic_fetch TEXT,                -- comma-separated data sources to pre-fetch

    -- ACTION: what to do
    action_type TEXT NOT NULL,       -- "notify" | "tool_call"
    action_config TEXT NOT NULL,     -- JSON

    -- Lifecycle
    status TEXT NOT NULL DEFAULT 'active',
    next_fire_at INTEGER,            -- for schedule rules: next trigger time (UTC)
    expires_at INTEGER,
    last_triggered_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ont_rules_user_id ON ont_rules(user_id);
CREATE INDEX IF NOT EXISTS idx_ont_rules_status ON ont_rules(status);
CREATE INDEX IF NOT EXISTS idx_ont_rules_next_fire ON ont_rules(next_fire_at) WHERE trigger_type = 'schedule' AND status = 'active';
CREATE INDEX IF NOT EXISTS idx_ont_rules_trigger_type ON ont_rules(trigger_type, status);
