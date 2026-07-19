ALTER TABLE light_tool_runs
    ADD COLUMN account_user_id INT4 REFERENCES users(id) ON DELETE CASCADE;

CREATE INDEX light_tool_runs_account_user_idx
    ON light_tool_runs (account_user_id)
    WHERE account_user_id IS NOT NULL;
