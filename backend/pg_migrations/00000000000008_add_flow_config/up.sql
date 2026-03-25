-- Add flow_config column: stores the full evaluation tree as JSON.
-- Replaces logic_type/logic_prompt/logic_fetch/action_type/action_config.
ALTER TABLE ont_rules ADD COLUMN flow_config TEXT;
