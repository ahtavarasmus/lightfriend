-- This file should undo anything in `up.sql`
alter table message_history drop column tool_calls_json;
