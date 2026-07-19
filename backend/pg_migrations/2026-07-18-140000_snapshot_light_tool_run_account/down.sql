DROP INDEX IF EXISTS light_tool_runs_account_user_idx;

ALTER TABLE light_tool_runs
    DROP COLUMN IF EXISTS account_user_id;
